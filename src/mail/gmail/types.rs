/// Gmail 账户数据结构
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::crypto;

/// Gmail 账户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailAccount {
    /// 邮箱地址
    pub email: String,

    /// 显示名称
    pub display_name: String,

    /// 访问令牌（加密存储）
    ///
    /// 格式：`"encrypted:BASE64..."`
    #[serde(
        serialize_with = "serialize_token",
        deserialize_with = "deserialize_token"
    )]
    pub access_token: String,

    /// 刷新令牌（加密存储）
    ///
    /// 格式：`"encrypted:BASE64..."`
    #[serde(
        serialize_with = "serialize_token",
        deserialize_with = "deserialize_token"
    )]
    pub refresh_token: String,

    /// Token 过期时间（UTC）
    pub expires_at: DateTime<Utc>,

    /// 账户是否激活
    #[serde(default = "default_true")]
    pub is_active: bool,
}

/// 默认值：true
fn default_true() -> bool {
    true
}

/// 序列化 Token（加密）
///
/// 如果 Token 未加密（明文），则先加密再序列化
fn serialize_token<S>(token: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::Error;

    // 如果已加密，直接序列化
    if crypto::is_encrypted(token) {
        return serializer.serialize_str(token);
    }

    // 否则先加密
    let encrypted = crypto::encrypt_token(token).map_err(S::Error::custom)?;
    serializer.serialize_str(&encrypted)
}

/// 反序列化 Token（保持加密状态）
///
/// 从文件读取时保持加密状态，不立即解密（按需解密）
fn deserialize_token<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // 验证格式
    if !crypto::is_encrypted(&s) {
        return Err(serde::de::Error::custom(
            "Token 格式错误：应为加密格式（encrypted:...）",
        ));
    }

    Ok(s)
}

impl GmailAccount {
    /// 创建新账户（Token 为明文，会自动加密）
    ///
    /// Token 在创建时会立即加密，确保内存中不存储明文
    pub fn new(
        email: String,
        display_name: String,
        access_token: String,
        refresh_token: String,
        expires_in_seconds: i64,
    ) -> Result<Self> {
        // 在创建时立即加密 Token，保护内存安全
        let encrypted_access_token =
            crypto::encrypt_token(&access_token).context("加密 Access Token 失败")?;
        let encrypted_refresh_token =
            crypto::encrypt_token(&refresh_token).context("加密 Refresh Token 失败")?;

        Ok(Self {
            email,
            display_name,
            access_token: encrypted_access_token,
            refresh_token: encrypted_refresh_token,
            expires_at: Utc::now() + chrono::Duration::seconds(expires_in_seconds),
            is_active: true,
        })
    }

    /// 解密访问令牌
    pub fn decrypt_access_token(&self) -> Result<String> {
        crypto::decrypt_token(&self.access_token)
    }

    /// 解密刷新令牌
    pub fn decrypt_refresh_token(&self) -> Result<String> {
        crypto::decrypt_token(&self.refresh_token)
    }

    /// 检查 Token 是否即将过期
    ///
    /// # Arguments
    /// * `threshold_minutes` - 提前多少分钟算作"即将过期"
    pub fn is_token_expiring(&self, threshold_minutes: i64) -> bool {
        let threshold = Utc::now() + chrono::Duration::minutes(threshold_minutes);
        self.expires_at <= threshold
    }

    /// 更新访问令牌（自动加密）
    pub fn update_access_token(
        &mut self,
        new_token: String,
        expires_in_seconds: i64,
    ) -> Result<()> {
        self.access_token = crypto::encrypt_token(&new_token)?;
        self.expires_at = Utc::now() + chrono::Duration::seconds(expires_in_seconds);
        Ok(())
    }
}

/// 转换为 Slint UI 的 Account 类型
use slint::Image;

impl From<GmailAccount> for crate::Account {
    fn from(gmail: GmailAccount) -> Self {
        // 尝试加载占位符图片（项目资源），失败则使用 Image::default()
        let placeholder = match Image::load_from_path(std::path::Path::new(
            "assets/icons/placeholder-avatar.svg",
        )) {
            Ok(img) => img,
            Err(_) => Image::default(),
        };

        Self {
            email: gmail.email.into(),
            display_name: gmail.display_name.into(),
            avatar_image: placeholder,
            unread_count: 0, // TODO: 阶段4 实现未读数获取
            is_loading: false,
            has_error: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_create_account() {
        let account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "plain_access_token".to_string(),
            "plain_refresh_token".to_string(),
            3600,
        )
        .expect("创建账户失败");

        assert_eq!(account.email, "test@gmail.com");
        assert_eq!(account.display_name, "Test User");
        assert!(account.is_active);
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_serialize_with_encryption() {
        let account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "plain_access_token".to_string(),
            "plain_refresh_token".to_string(),
            3600,
        )
        .expect("创建账户失败");

        // 序列化为 TOML
        let toml = toml::to_string(&account).unwrap();
        println!("序列化结果:\n{}", toml);

        // 验证 Token 已加密
        assert!(toml.contains("encrypted:"));
        assert!(!toml.contains("plain_access_token"));
        assert!(!toml.contains("plain_refresh_token"));
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_decrypt_tokens() {
        let account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "plain_access_token".to_string(),
            "plain_refresh_token".to_string(),
            3600,
        )
        .expect("创建账户失败");

        // Token 现在已经在创建时加密了，直接解密
        let decrypted_access = account.decrypt_access_token().unwrap();
        let decrypted_refresh = account.decrypt_refresh_token().unwrap();

        assert_eq!(decrypted_access, "plain_access_token");
        assert_eq!(decrypted_refresh, "plain_refresh_token");
    }

    #[test]
    fn test_is_token_expiring() {
        let mut account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "token".to_string(),
            "refresh".to_string(),
            3600, // 1 小时后过期
        )
        .expect("创建账户失败");

        // 未过期（提前 10 分钟检查）
        assert!(!account.is_token_expiring(10));

        // 即将过期（提前 120 分钟检查）
        assert!(account.is_token_expiring(120));

        // 设置为已过期
        account.expires_at = Utc::now() - chrono::Duration::minutes(10);
        assert!(account.is_token_expiring(0));
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_update_access_token() {
        let mut account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "old_token".to_string(),
            "refresh".to_string(),
            3600,
        )
        .expect("创建账户失败");

        let old_expires = account.expires_at;

        // 等待 1 秒确保时间戳变化
        std::thread::sleep(std::time::Duration::from_secs(1));

        // 更新 Token
        account
            .update_access_token("new_token".to_string(), 7200)
            .unwrap();

        // 验证
        assert!(crypto::is_encrypted(&account.access_token));
        assert_ne!(account.expires_at, old_expires);

        let decrypted = account.decrypt_access_token().unwrap();
        assert_eq!(decrypted, "new_token");
    }

    #[test]
    fn test_convert_to_slint_account() {
        let gmail_account = GmailAccount::new(
            "test@gmail.com".to_string(),
            "Test User".to_string(),
            "token".to_string(),
            "refresh".to_string(),
            3600,
        )
        .expect("创建账户失败");

        let slint_account: crate::Account = gmail_account.into();

        assert_eq!(slint_account.email.as_str(), "test@gmail.com");
        assert_eq!(slint_account.display_name.as_str(), "Test User");
        assert_eq!(slint_account.unread_count, 0);
        assert!(!slint_account.is_loading);
        assert!(!slint_account.has_error);
    }
}
