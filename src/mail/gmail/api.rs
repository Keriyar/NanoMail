/// Gmail API 调用模块
///
/// 负责调用 Gmail API 获取邮件信息、未读数量以及用户信息（头像、昵称）
use anyhow::{Context, Result};
use serde::Deserialize;

use crate::mail::gmail::types::GmailAccount;
use crate::utils::http_client;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// 在同步前检测网络可用性并在失败时按指数退避重试
async fn ensure_network_available() -> Result<bool> {
    const CHECK_URL: &str = "https://www.google.com/generate_204";
    const MAX_ATTEMPTS: usize = 4;
    const PER_REQUEST_TIMEOUT_SECS: u64 = 3;

    let client = http_client::get_client();
    let mut attempt = 0usize;
    let mut delay_secs = 1u64;
    let mut had_failure = false;

    loop {
        attempt += 1;
        tracing::debug!("网络检测: 第 {} 次，尝试连接 {}", attempt, CHECK_URL);

        match timeout(
            Duration::from_secs(PER_REQUEST_TIMEOUT_SECS),
            client.get(CHECK_URL).send(),
        )
        .await
        {
            Ok(Ok(resp)) => {
                // 204 表示连接成功且无内容
                if resp.status().is_success() {
                    tracing::debug!("网络检测成功 (HTTP {})", resp.status());
                    return Ok(had_failure);
                } else {
                    tracing::warn!("网络检测返回非成功状态: {}", resp.status());
                    had_failure = true;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("网络检测请求失败: {}", e);
                had_failure = true;
            }
            Err(_) => {
                tracing::warn!("网络检测超时 ({}s)", PER_REQUEST_TIMEOUT_SECS);
                had_failure = true;
            }
        }

        if attempt >= MAX_ATTEMPTS {
            tracing::error!("网络不可用：连续 {} 次检测失败", MAX_ATTEMPTS);
            return Err(anyhow::anyhow!("网络不可用"));
        }

        tracing::info!("网络检测失败，{} 秒后重试（指数退避）...", delay_secs);
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        delay_secs = std::cmp::min(delay_secs * 2, 30);
    }
}

/// Google UserInfo 响应 (OIDC 标准)
/// 替代了原本分散的 ProfileResponse 和 People API
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    /// 用户完整姓名
    pub name: Option<String>,

    /// 用户头像 URL
    pub picture: Option<String>,

    /// 邮箱地址
    pub email: String,
}

/// Gmail 标签信息（用于获取精确未读数）
#[derive(Debug, Deserialize)]
struct LabelInfo {
    /// 标签中的未读消息数
    #[serde(rename = "messagesUnread")]
    messages_unread: Option<u32>,
}

/// Gmail API 客户端
pub struct GmailApiClient {
    access_token: String,
}

impl GmailApiClient {
    /// 创建新的 Gmail API 客户端
    ///
    /// # Arguments
    /// * `access_token` - 已解密的 Access Token（明文）
    pub fn new(access_token: String) -> Self {
        Self { access_token }
    }

    /// 获取未读邮件数量
    ///
    /// 使用 Gmail Labels API 获取 INBOX 标签的 messagesUnread 字段
    /// 这比 messages.list 的 resultSizeEstimate 更精确
    ///
    /// # Returns
    /// 返回未读邮件数量
    pub async fn get_unread_count(&self) -> Result<u32> {
        tracing::debug!("正在获取未读邮件数量...");

        // 使用 Labels API 获取 INBOX 标签信息（包含精确的未读数）
        let url = "https://gmail.googleapis.com/gmail/v1/users/me/labels/INBOX";

        let response = http_client::get_client()
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .context("请求 INBOX 标签信息失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            if status == 401 {
                anyhow::bail!("Token 已过期，需要刷新");
            }

            anyhow::bail!("Gmail Labels API 返回错误 {}: {}", status, error_text);
        }

        // 获取原始响应体用于调试
        let response_text = response.text().await.context("读取响应体失败")?;
        tracing::info!("[DEBUG-UNREAD] Gmail Labels API 原始响应: {}", response_text);

        let label_info: LabelInfo =
            serde_json::from_str(&response_text).context("解析标签信息响应失败")?;

        let unread_count = label_info.messages_unread.unwrap_or(0);

        tracing::info!(
            "[DEBUG-UNREAD] messagesUnread = {:?}, 最终 unread_count = {}",
            label_info.messages_unread,
            unread_count
        );

        Ok(unread_count)
    }

    /// 获取用户信息（包含头像、名字、邮箱）
    ///
    /// 使用 Google OAuth2 UserInfo 端点，一次性获取所有资料。
    /// 相比 Gmail Profile API + People API，这种方式更标准且不容易出现权限问题。
    ///
    /// # Returns
    /// 返回 GoogleUserInfo 结构体
    pub async fn get_user_info(&self) -> Result<GoogleUserInfo> {
        tracing::debug!("正在获取用户资料(头像/邮箱)...");

        // Google 标准 OIDC 用户信息端点
        // 需要 scope: "https://www.googleapis.com/auth/userinfo.profile"
        let url = "https://www.googleapis.com/oauth2/v3/userinfo";

        let response = http_client::get_client()
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .context("请求用户信息失败")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            if status == 403 || status == 404 {
                tracing::warn!(
                    "获取用户信息失败，可能是 Scope 缺失 (userinfo.profile): {}",
                    error_text
                );
            }

            anyhow::bail!("UserInfo API 返回错误 {}: {}", status, error_text);
        }

        let info: GoogleUserInfo = response.json().await.context("解析用户信息响应失败")?;

        tracing::debug!(
            "✅ 获取到用户信息: {} (头像是否存在: {})",
            info.email,
            info.picture.is_some()
        );

        Ok(info)
    }
}

/// 下载头像并缓存到配置目录下的 `avatars/`，返回本地 file:// URI（如果成功）
async fn download_avatar_to_cache(url: &str, email: &str) -> Option<String> {
    // 解析扩展名（优先从 Content-Type）
    let client = reqwest::Client::new();

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("下载头像失败（请求失败）: {}: {}", url, e);
            return None;
        }
    };

    if !resp.status().is_success() {
        tracing::warn!("下载头像失败（HTTP {}）: {}", resp.status(), url);
        return None;
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let ext = if content_type.starts_with("image/png") {
        "png"
    } else if content_type.starts_with("image/jpeg") {
        "jpg"
    } else if content_type.starts_with("image/webp") {
        "webp"
    } else if content_type.starts_with("image/svg") || content_type.contains("svg") {
        "svg"
    } else {
        // fallback: try parse from url
        if let Some(pos) = url.rfind('.') {
            let candidate = &url[pos + 1..];
            if candidate.len() <= 5 {
                candidate
            } else {
                "img"
            }
        } else {
            "img"
        }
    };

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("读取头像响应体失败: {}", e);
            return None;
        }
    };

    // 构建缓存路径
    let mut cache_dir = match dirs::config_dir() {
        Some(d) => d.join("NanoMail").join("avatars"),
        None => {
            tracing::warn!("无法获取配置目录，跳过头像缓存");
            return None;
        }
    };

    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        tracing::warn!("创建头像缓存目录失败: {}", e);
        return None;
    }

    // 文件名使用邮箱的 base64 或安全化
    let safe_name = email.replace('@', "_").replace('.', "_");
    cache_dir.push(format!("{}.{}", safe_name, ext));

    let path_buf: PathBuf = cache_dir.clone();

    if let Err(e) = std::fs::write(&path_buf, &bytes) {
        tracing::warn!("写入头像缓存失败: {}", e);
        return None;
    }

    // 返回本地绝对路径（Slint 在不同平台对 file:// 支持不一，使用本地路径更稳健）
    Some(path_buf.display().to_string())
}

/// 账户同步信息（包含未读数、头像和错误状态）
#[derive(Debug, Clone)]
pub struct AccountSyncInfo {
    pub email: String,
    pub unread_count: u32,
    pub avatar_url: String,
    pub display_name: String,
    pub error_message: Option<String>, // 新增：错误消息（如果同步失败）
    pub network_issue: bool,           // 新增：同步过程中是否曾检测到网络问题（即临时失败）
}

/// 同步账户信息（获取未读数和头像）
///
/// # Arguments
/// * `account` - Gmail 账户（需要有效的 Token）
///
/// # Returns
/// 返回同步后的账户信息和更新后的账户（如果 Token 被刷新）
pub async fn sync_account_info(
    account: &GmailAccount,
) -> Result<(AccountSyncInfo, Option<GmailAccount>)> {
    tracing::info!("🔄 同步账户信息: {}", account.email);

    // 同步前执行网络检测与重连（若网络不可用则进行重试）。
    tracing::debug!("同步前执行网络检测...");
    let had_network_issue = match ensure_network_available().await {
        Ok(had) => had,
        Err(e) => {
            tracing::error!("网络检测最终失败，跳过同步 {}: {}", account.email, e);
            return Err(e).context("网络检测失败，取消本次同步");
        }
    };

    // 使用 TokenManager 获取有效的 Access Token（自动刷新过期的 Token）
    let mut token_manager = crate::mail::gmail::token::TokenManager::new(account.clone())
        .context("创建 TokenManager 失败")?;

    let access_token = token_manager
        .get_valid_token()
        .await
        .context("获取有效 Access Token 失败")?;

    // 检查 Token 是否被刷新（如果刷新了，需要返回更新后的账户）
    let updated_account = if token_manager.account().expires_at != account.expires_at {
        tracing::info!("✅ Token 已自动刷新，更新账户信息");
        Some(token_manager.account().clone())
    } else {
        None
    };

    // 创建 API 客户端
    let client = GmailApiClient::new(access_token);

    // 获取未读数（并行/先行请求可提升性能，但这里先获取未读数）
    let unread_count = client.get_unread_count().await.context("获取未读数失败")?;

    // 处理用户信息，失败时降级处理；如果是 401，尝试强制刷新 Token 并重试一次
    let info_result = client.get_user_info().await;

    let (email, avatar_url, display_name, error_message) = match info_result {
        Ok(info) => {
            // 尝试下载头像到本地缓存，若失败则使用远程 URL
            let avatar = if let Some(pic_url) = info.picture {
                match download_avatar_to_cache(&pic_url, &info.email).await {
                    Some(local_uri) => local_uri,
                    None => pic_url,
                }
            } else {
                String::new()
            };

            (
                info.email,
                avatar,
                info.name.unwrap_or_else(|| account.email.clone()),
                None,
            )
        }
        Err(e) => {
            let error_str = e.to_string();

            if error_str.contains("401") {
                tracing::error!("❌ 获取用户信息失败 [401 Unauthorized]: {}", error_str);
                tracing::error!("   💡 尝试使用 Refresh Token 刷新 Access Token 并重试");

                // 尝试刷新 Token 并重试一次
                match token_manager.force_refresh().await {
                    Ok(_) => {
                        tracing::info!("✅ 强制刷新 Token 成功，重试 UserInfo 请求");
                        match token_manager.get_valid_token().await {
                            Ok(new_token) => {
                                let new_client = GmailApiClient::new(new_token);
                                match new_client.get_user_info().await {
                                    Ok(info2) => {
                                        // 同样尝试缓存重试获取到的头像
                                        let avatar2 = if let Some(pic2) = info2.picture {
                                            match download_avatar_to_cache(&pic2, &info2.email)
                                                .await
                                            {
                                                Some(local_uri2) => local_uri2,
                                                None => pic2,
                                            }
                                        } else {
                                            String::new()
                                        };

                                        (
                                            info2.email,
                                            avatar2,
                                            info2.name.unwrap_or_else(|| account.email.clone()),
                                            None,
                                        )
                                    }
                                    Err(e2) => {
                                        tracing::error!("❌ 重试 UserInfo 仍失败: {}", e2);
                                        (
                                            account.email.clone(),
                                            String::new(),
                                            account.email.clone(),
                                            Some("Token 无效或已过期，请重新授权".to_string()),
                                        )
                                    }
                                }
                            }
                            Err(e3) => {
                                tracing::error!("无法获取刷新后的 Access Token: {}", e3);
                                (
                                    account.email.clone(),
                                    String::new(),
                                    account.email.clone(),
                                    Some("Token 无效或已过期，请重新授权".to_string()),
                                )
                            }
                        }
                    }
                    Err(refresh_err) => {
                        tracing::error!("强制刷新 Token 失败: {}", refresh_err);
                        tracing::error!(
                            "   💡 可能原因:\n   - Refresh Token 已过期或被撤销\n   - 用户撤销了应用授权\n   - 需要用户重新授权，请移除后重新添加账户"
                        );

                        (
                            account.email.clone(),
                            String::new(),
                            account.email.clone(),
                            Some("Token 无效或已过期，请重新授权".to_string()),
                        )
                    }
                }
            } else {
                tracing::warn!("⚠️ 获取用户信息失败 (使用本地缓存): {}", error_str);
                (
                    account.email.clone(),
                    String::new(),
                    account.email.clone(),
                    Some(format!("获取用户信息失败: {}", error_str)),
                )
            }
        }
    };

    tracing::info!(
        "[DEBUG-UNREAD] sync_account_info 完成: email={}, unread_count={}, error={:?}",
        email,
        unread_count,
        error_message
    );

    let sync_info = AccountSyncInfo {
        email: email.clone(),
        unread_count,
        avatar_url,
        display_name,
        error_message,
        network_issue: had_network_issue,
    };

    tracing::info!(
        "[DEBUG-UNREAD] 返回 AccountSyncInfo: email={}, unread_count={}",
        sync_info.email,
        sync_info.unread_count
    );

    Ok((sync_info, updated_account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_creation() {
        let client = GmailApiClient::new("test_token".to_string());
        assert_eq!(client.access_token, "test_token");
    }

    #[tokio::test]
    #[ignore] // 需要有效的 Access Token
    async fn test_get_unread_count() {
        let access_token =
            std::env::var("TEST_ACCESS_TOKEN").expect("请设置 TEST_ACCESS_TOKEN 环境变量");

        let client = GmailApiClient::new(access_token);
        let count = client.get_unread_count().await.unwrap();

        println!("未读邮件数: {}", count);
        assert!(count >= 0);
    }

    #[tokio::test]
    #[ignore] // 需要有效的 Access Token
    async fn test_get_user_info() {
        let access_token =
            std::env::var("TEST_ACCESS_TOKEN").expect("请设置 TEST_ACCESS_TOKEN 环境变量");

        let client = GmailApiClient::new(access_token);
        let info = client.get_user_info().await.unwrap();

        println!(
            "邮箱: {}, 名字: {:?}, 头像: {:?}",
            info.email, info.name, info.picture
        );
        assert!(!info.email.is_empty());
    }
}
