use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// 新增模块
pub mod crypto;
pub mod oauth_config;
pub mod storage;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub theme: String,
    pub sync_interval: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                version: "0.1.0".to_string(),
                theme: "light".to_string(),
                sync_interval: 300,
            },
        }
    }
}

/// 获取配置文件路径
pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("NanoMail");

    std::fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("config.toml"))
}

/// 加载配置
pub fn load() -> Result<Config> {
    let path = config_path()?;

    if !path.exists() {
        let config = Config::default();
        save(&config)?;
        return Ok(config);
    }

    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// 保存配置
pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}
