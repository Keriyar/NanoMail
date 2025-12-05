/// Token 自动刷新管理模块
use anyhow::{Context, Result};
use oauth2::{
    AuthUrl, ClientId, ClientSecret, RefreshToken, TokenResponse, TokenUrl, basic::BasicClient,
};

use crate::config::{oauth_config::OAuthConfig, storage};
use crate::mail::gmail::types::GmailAccount;

/// Token 刷新阈值（提前多少分钟刷新）
const REFRESH_THRESHOLD_MINUTES: i64 = 5;

/// Token 管理器
///
/// 负责自动刷新过期的 Access Token
pub struct TokenManager {
    /// 关联的 Gmail 账户
    account: GmailAccount,

    /// OAuth2 配置
    oauth_config: OAuthConfig,
}

impl TokenManager {
    /// 创建 Token 管理器
    ///
    /// # Arguments
    /// * `account` - Gmail 账户（包含加密的 Token）
    ///
    /// # Errors
    /// - OAuth2 配置加载失败
    pub fn new(account: GmailAccount) -> Result<Self> {
        let oauth_config = OAuthConfig::load().context("加载 OAuth2 配置失败")?;

        Ok(Self {
            account,
            oauth_config,
        })
    }

    /// 获取有效的 Access Token
    ///
    /// 如果 Token 即将过期（默认提前 5 分钟），则自动刷新
    ///
    /// # Returns
    /// 返回解密后的 Access Token（明文）
    ///
    /// # Errors
    /// - Token 刷新失败
    /// - 解密失败
    pub async fn get_valid_token(&mut self) -> Result<String> {
        // 检查是否需要刷新
        if self.account.is_token_expiring(REFRESH_THRESHOLD_MINUTES) {
            tracing::info!(
                "Access Token 即将过期（{}），自动刷新",
                self.account.expires_at
            );
            self.refresh_access_token().await?;
        }

        // 解密并返回
        self.account.decrypt_access_token()
    }

    /// 强制刷新 Access Token
    ///
    /// 使用 Refresh Token 从 Google 获取新的 Access Token
    ///
    /// # Errors
    /// - Refresh Token 解密失败
    /// - 网络请求失败
    /// - OAuth2 配置无效
    /// - 保存账户失败
    async fn refresh_access_token(&mut self) -> Result<()> {
        tracing::debug!("开始刷新 Access Token");

        // 1. 解密 Refresh Token
        let refresh_token = self
            .account
            .decrypt_refresh_token()
            .context("解密 Refresh Token 失败")?;

        // 2. 构建 OAuth2 客户端
        let client = BasicClient::new(
            ClientId::new(self.oauth_config.client_id.clone()),
            Some(ClientSecret::new(self.oauth_config.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
            Some(TokenUrl::new(
                "https://oauth2.googleapis.com/token".to_string(),
            )?),
        );

        // 3. 使用 Refresh Token 交换新的 Access Token
        let token_response = client
            .exchange_refresh_token(&RefreshToken::new(refresh_token))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();

                // 提供更清晰的错误消息
                if error_msg.contains("invalid_grant") || error_msg.contains("401") {
                    tracing::error!("❌ Token 刷新失败 [授权被拒绝/已过期]: {}", error_msg);
                    tracing::error!(
                        "   💡 可能原因:\n   \
                         - Refresh Token 已过期或被撤销\n   \
                         - 用户撤销了应用授权\n   \
                         - 需要用户重新授权，请移除后重新添加账户"
                    );
                    anyhow::anyhow!(
                        "Refresh Token 交换失败（可能已过期或被撤销）：{}",
                        error_msg
                    )
                } else {
                    anyhow::anyhow!("Refresh Token 交换失败: {}", error_msg)
                }
            })?;

        let new_access_token = token_response.access_token().secret().to_string();
        let expires_in = token_response
            .expires_in()
            .unwrap_or(std::time::Duration::from_secs(3600))
            .as_secs() as i64;

        // 4. 更新账户（自动加密）
        self.account
            .update_access_token(new_access_token.clone(), expires_in)
            .context("更新 Access Token 失败")?;

        // 5. 持久化到文件
        storage::save_account(&self.account).context("保存账户失败")?;

        tracing::info!(
            "✅ Access Token 刷新成功（新的过期时间: {}）",
            self.account.expires_at
        );

        tracing::debug!(
            "新 Token: {}...{}",
            &new_access_token[..5],
            &new_access_token[new_access_token.len() - 5..]
        );

        Ok(())
    }

    /// 对外暴露的强制刷新方法
    ///
    /// 在某些情况下（例如调用 UserInfo 返回 401），需要立即尝试使用
    /// Refresh Token 交换新的 Access Token。该方法包装内部的刷新实现。
    pub async fn force_refresh(&mut self) -> Result<()> {
        self.refresh_access_token().await
    }

    /// 获取账户引用
    pub fn account(&self) -> &GmailAccount {
        &self.account
    }

    /// 获取可变账户引用
    pub fn account_mut(&mut self) -> &mut GmailAccount {
        &mut self.account
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_refresh_threshold() {
        assert_eq!(REFRESH_THRESHOLD_MINUTES, 5);
    }

    #[tokio::test]
    #[ignore] // 需要有效的 Refresh Token 和网络连接
    async fn test_token_refresh() {
        // 创建一个过期的账户
        let mut account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "old_access_token".to_string(),
            "valid_refresh_token".to_string(),
            -3600, // 已过期 1 小时
        )
        .expect("创建账户失败");

        // 设置为已过期
        account.expires_at = Utc::now() - chrono::Duration::hours(1);

        // 创建管理器
        let mut manager = TokenManager::new(account).unwrap();

        // 应该触发刷新
        assert!(manager.account.is_token_expiring(0));

        // 尝试获取有效 Token（会自动刷新）
        // 注意：此测试需要有效的 OAuth2 配置和 Refresh Token
        let result = manager.get_valid_token().await;

        if let Ok(token) = result {
            println!("刷新成功，新 Token: {}...", &token[..10]);
            assert!(!token.is_empty());
        } else {
            println!("刷新失败（预期：需要有效的 Refresh Token）");
        }
    }
}
