/// 账户文件存储模块
///
/// 负责将 Gmail 账户信息持久化到 TOML 文件
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mail::gmail::types::GmailAccount;

/// 账户存储文件版本号
const STORAGE_VERSION: &str = "1.0";

/// 账户存储容器
#[derive(Debug, Serialize, Deserialize)]
struct AccountsStorage {
    /// 文件格式版本
    version: String,

    /// Gmail 账户列表
    accounts: Vec<AccountEntry>,
}

/// 账户条目（包含类型标识）
#[derive(Debug, Serialize, Deserialize)]
struct AccountEntry {
    /// 账户类型（gmail, netease, 等）
    #[serde(rename = "type")]
    account_type: String,

    /// Gmail 账户数据
    #[serde(flatten)]
    gmail: GmailAccount,
}

impl Default for AccountsStorage {
    fn default() -> Self {
        Self {
            version: STORAGE_VERSION.to_string(),
            accounts: Vec::new(),
        }
    }
}

/// 获取账户文件路径
///
/// 返回：`%APPDATA%\NanoMail\accounts.toml`
pub fn accounts_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("NanoMail");

    // 确保目录存在
    std::fs::create_dir_all(&config_dir)
        .context("创建配置目录失败")?;

    Ok(config_dir.join("accounts.toml"))
}

/// 加载所有账户
///
/// # Returns
/// 返回所有已保存的 Gmail 账户列表，文件不存在时返回空列表
///
/// # Errors
/// - 文件格式错误
/// - 反序列化失败
pub fn load_accounts() -> Result<Vec<GmailAccount>> {
    let path = accounts_path()?;

    // 文件不存在时返回空列表
    if !path.exists() {
        tracing::debug!("账户文件不存在，返回空列表");
        return Ok(Vec::new());
    }

    // 读取文件
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("读取账户文件失败: {}", path.display()))?;

    // 解析 TOML
    let storage: AccountsStorage = toml::from_str(&content)
        .context("解析账户文件失败（文件可能损坏）")?;

    // 验证版本
    if storage.version != STORAGE_VERSION {
        tracing::warn!(
            "账户文件版本不匹配（期望: {}, 实际: {}），尝试兼容加载",
            STORAGE_VERSION,
            storage.version
        );
    }

    // 提取 Gmail 账户
    let accounts: Vec<GmailAccount> = storage
        .accounts
        .into_iter()
        .map(|entry| entry.gmail)
        .collect();

    tracing::debug!("成功加载 {} 个账户", accounts.len());

    Ok(accounts)
}

/// 保存所有账户
///
/// 覆盖式保存，替换整个账户列表
///
/// # Arguments
/// * `accounts` - 要保存的账户列表
///
/// # Errors
/// - 序列化失败
/// - 文件写入失败
pub fn save_accounts(accounts: &[GmailAccount]) -> Result<()> {
    let path = accounts_path()?;

    // 转换为存储格式
    let entries: Vec<AccountEntry> = accounts
        .iter()
        .map(|gmail| AccountEntry {
            account_type: "gmail".to_string(),
            gmail: gmail.clone(),
        })
        .collect();

    let storage = AccountsStorage {
        version: STORAGE_VERSION.to_string(),
        accounts: entries,
    };

    // 序列化为 TOML
    let content = toml::to_string_pretty(&storage)
        .context("序列化账户数据失败")?;

    // 写入文件
    std::fs::write(&path, content)
        .with_context(|| format!("写入账户文件失败: {}", path.display()))?;

    tracing::debug!("成功保存 {} 个账户到: {}", accounts.len(), path.display());

    Ok(())
}

/// 保存单个账户（追加或更新）
///
/// 如果账户已存在（邮箱相同），则更新；否则追加
///
/// # Arguments
/// * `account` - 要保存的账户
///
/// # Errors
/// - 加载或保存失败
pub fn save_account(account: &GmailAccount) -> Result<()> {
    let mut accounts = load_accounts()?;

    // 查找是否已存在
    if let Some(existing) = accounts.iter_mut().find(|a| a.email == account.email) {
        tracing::debug!("更新已存在的账户: {}", account.email);
        *existing = account.clone();
    } else {
        tracing::debug!("添加新账户: {}", account.email);
        accounts.push(account.clone());
    }

    save_accounts(&accounts)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_account(email: &str) -> GmailAccount {
        GmailAccount::new(
            email.to_string(),
            format!("{} User", email),
            "test_access_token".to_string(),
            "test_refresh_token".to_string(),
            3600,
        ).expect("创建测试账户失败")
    }

    #[test]
    #[ignore] // 需要文件系统权限
    fn test_accounts_path() {
        let path = accounts_path().unwrap();
        println!("账户文件路径: {}", path.display());
        assert!(path.to_string_lossy().contains("NanoMail"));
        assert!(path.to_string_lossy().ends_with("accounts.toml"));
    }

    #[test]
    #[ignore] // 需要 Windows 环境和文件系统权限
    fn test_save_and_load_single_account() {
        let account = create_test_account("test1@gmail.com");

        // 保存
        save_account(&account).unwrap();

        // 加载
        let loaded = load_accounts().unwrap();
        assert!(!loaded.is_empty());

        let found = loaded.iter().find(|a| a.email == "test1@gmail.com");
        assert!(found.is_some());

        let found = found.unwrap();
        assert_eq!(found.email, "test1@gmail.com");
        assert_eq!(found.display_name, "test1@gmail.com User");
    }

    #[test]
    #[ignore] // 需要 Windows 环境和文件系统权限
    fn test_save_multiple_accounts() {
        let accounts = vec![
            create_test_account("user1@gmail.com"),
            create_test_account("user2@gmail.com"),
            create_test_account("user3@gmail.com"),
        ];

        // 保存多个
        save_accounts(&accounts).unwrap();

        // 加载验证
        let loaded = load_accounts().unwrap();
        assert_eq!(loaded.len(), 3);
    }

    #[test]
    #[ignore] // 需要 Windows 环境和文件系统权限
    fn test_update_existing_account() {
        let mut account = create_test_account("update@gmail.com");

        // 第一次保存
        save_account(&account).unwrap();

        // 修改并再次保存
        account.display_name = "Updated Name".to_string();
        save_account(&account).unwrap();

        // 验证更新
        let loaded = load_accounts().unwrap();
        let found = loaded.iter().find(|a| a.email == "update@gmail.com").unwrap();
        assert_eq!(found.display_name, "Updated Name");

        // 验证没有重复
        let count = loaded.iter().filter(|a| a.email == "update@gmail.com").count();
        assert_eq!(count, 1);
    }

    #[test]
    #[ignore] // 需要 Windows 环境和文件系统权限
    fn test_empty_file() {
        // 删除文件
        let path = accounts_path().unwrap();
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }

        // 加载应返回空列表
        let loaded = load_accounts().unwrap();
        assert!(loaded.is_empty());
    }
}
