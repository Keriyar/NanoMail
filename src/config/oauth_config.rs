/// OAuth2 配置读取模块
///
/// 支持从环境变量、配置文件或默认值读取 OAuth2 客户端凭据
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// OAuth2 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// Google OAuth2 客户端 ID
    pub client_id: String,

    /// Google OAuth2 客户端密钥
    pub client_secret: String,

    /// 重定向 URI（本地服务器地址）
    pub redirect_uri: String,

    /// 请求的 API 权限范围
    pub scopes: Vec<String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: "YOUR_CLIENT_ID.apps.googleusercontent.com".to_string(),
            client_secret: "YOUR_CLIENT_SECRET".to_string(),
            redirect_uri: "http://localhost:8080".to_string(),
            // 修改这里：添加 userinfo.email, userinfo.profile 和 openid
            scopes: vec![
                "https://www.googleapis.com/auth/gmail.readonly".to_string(), // 读取邮件状态
                "https://www.googleapis.com/auth/userinfo.email".to_string(), // 获取邮箱地址
                "https://www.googleapis.com/auth/userinfo.profile".to_string(), // 获取头像和名字
                "openid".to_string(),                                         // OIDC 身份认证标准
            ],
        }
    }
}

impl OAuthConfig {
    /// 加载 OAuth2 配置
    ///
    /// 优先级（从高到低）：
    /// 1. 环境变量：`GMAIL_CLIENT_ID`, `GMAIL_CLIENT_SECRET`
    /// 2. 配置文件：`%APPDATA%\NanoMail\config.toml` 的 `[oauth]` 段
    /// 3. 默认占位符（用于开发/测试）
    ///
    /// # Returns
    /// 返回加载的配置，即使使用默认值也不会报错
    ///
    /// # Example
    /// ```no_run
    /// let config = OAuthConfig::load()?;
    /// println!("Client ID: {}", config.client_id);
    /// ```
    pub fn load() -> Result<Self> {
        // 优先级 1：环境变量
        if let (Ok(client_id), Ok(client_secret)) = (
            std::env::var("GMAIL_CLIENT_ID"),
            std::env::var("GMAIL_CLIENT_SECRET"),
        ) {
            tracing::info!("✅ 从环境变量加载 OAuth2 配置");

            // 使用默认配置为基础，确保 scopes 与默认一致（包含 userinfo.profile / openid）
            let mut cfg = Self::default();
            cfg.client_id = client_id;
            cfg.client_secret = client_secret;

            cfg.redirect_uri =
                std::env::var("OAUTH_REDIRECT_URI").unwrap_or_else(|_| cfg.redirect_uri.clone());

            return Ok(cfg);
        }

        // 优先级 2：配置文件
        if let Ok(config) = Self::load_from_file() {
            tracing::info!("✅ 从配置文件加载 OAuth2 配置");
            return Ok(config);
        }

        // 优先级 3：默认占位符
        tracing::warn!("⚠️ 未找到 OAuth2 配置，使用默认占位符");
        tracing::warn!(
            "请设置环境变量或创建配置文件：{}",
            Self::config_file_path()?.display()
        );

        Ok(Self::default())
    }

    /// 从配置文件加载
    fn load_from_file() -> Result<Self> {
        let path = Self::config_file_path()?;

        if !path.exists() {
            anyhow::bail!("配置文件不存在: {}", path.display());
        }

        let content = std::fs::read_to_string(&path)?;

        // 解析完整配置文件
        let config_toml: toml::Value = toml::from_str(&content)?;

        // 提取 [oauth] 段
        let oauth_section = config_toml
            .get("oauth")
            .ok_or_else(|| anyhow::anyhow!("配置文件缺少 [oauth] 段"))?;

        let oauth_config: Self = oauth_section.clone().try_into()?;

        Ok(oauth_config)
    }

    /// 获取配置文件路径
    ///
    /// 返回：`%APPDATA%\NanoMail\config.toml`
    fn config_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
            .join("NanoMail");

        Ok(config_dir.join("config.toml"))
    }

    /// 验证配置是否为默认占位符
    ///
    /// 用于检查用户是否已正确配置 OAuth2 凭据
    pub fn is_placeholder(&self) -> bool {
        self.client_id.contains("YOUR_CLIENT_ID")
            || self.client_secret.contains("YOUR_CLIENT_SECRET")
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OAuthConfig::default();
        assert!(config.is_placeholder());
        assert_eq!(config.redirect_uri, "http://localhost:8080");
        assert_eq!(config.scopes.len(), 4);
        assert!(config.scopes.iter().any(|s| s == "openid"));
    }

    #[test]
    fn test_is_placeholder() {
        let mut config = OAuthConfig::default();
        assert!(config.is_placeholder());

        config.client_id = "real-client-id.apps.googleusercontent.com".to_string();
        config.client_secret = "real-secret".to_string();
        assert!(!config.is_placeholder());
    }

    #[test]
    #[ignore] // 需要手动设置环境变量测试
    fn test_load_from_env() {
        unsafe {
            std::env::set_var("GMAIL_CLIENT_ID", "test-id");
            std::env::set_var("GMAIL_CLIENT_SECRET", "test-secret");
        }

        let config = OAuthConfig::load().unwrap();
        assert_eq!(config.client_id, "test-id");
        assert_eq!(config.client_secret, "test-secret");

        unsafe {
            std::env::remove_var("GMAIL_CLIENT_ID");
            std::env::remove_var("GMAIL_CLIENT_SECRET");
        }
    }

}
