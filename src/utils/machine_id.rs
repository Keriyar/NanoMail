/// 机器指纹与加密密钥派生模块
///
/// 从 Windows 注册表读取机器 GUID，使用 Argon2 派生加密密钥

use anyhow::{Context, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use winreg::enums::*;
use winreg::RegKey;

/// 固定盐值（编译时确定，用于密钥派生的一致性）
///
/// 注意：这个盐值对所有用户相同，真正的唯一性来自机器 GUID
const FIXED_SALT: &[u8] = b"NanoMail.v1.2025";

/// 从 Windows 注册表获取机器 GUID
///
/// 读取路径：`HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Cryptography\MachineGuid`
///
/// # Errors
/// - 无法打开注册表键（权限不足）
/// - MachineGuid 值不存在
fn get_machine_guid() -> Result<String> {
    tracing::debug!("正在从注册表读取机器 GUID");

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let crypto_key = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Cryptography")
        .context("无法打开注册表键：HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography")?;

    let guid: String = crypto_key
        .get_value("MachineGuid")
        .context("无法读取 MachineGuid 值（可能需要管理员权限）")?;

    tracing::debug!("机器 GUID 读取成功: {}...{}", &guid[..8], &guid[guid.len()-4..]);

    Ok(guid)
}

/// 从机器 GUID 派生 256-bit 加密密钥
///
/// 使用 Argon2id 算法从机器 GUID 派生密钥，确保：
/// 1. 密钥与硬件绑定（基于 MachineGuid）
/// 2. 相同机器上派生结果一致（固定盐值）
/// 3. 密钥强度高（Argon2 抗暴力破解）
///
/// # Returns
/// 返回 32 字节（256-bit）的加密密钥
///
/// # Errors
/// - 无法读取机器 GUID
/// - Argon2 哈希失败
///
/// # Example
/// ```no_run
/// let key = derive_encryption_key()?;
/// assert_eq!(key.len(), 32);
/// ```
pub fn derive_encryption_key() -> Result<[u8; 32]> {
    // 1. 获取机器 GUID
    let guid = get_machine_guid()?;

    // 2. 将固定盐值转换为 SaltString（Argon2 要求）
    let salt = SaltString::encode_b64(FIXED_SALT)
        .map_err(|e| anyhow::anyhow!("盐值编码失败: {}", e))?;

    // 3. 使用 Argon2id（平衡内存和 CPU 消耗）
    let argon2 = Argon2::default();

    // 4. 对 GUID 进行哈希
    let password_hash = argon2
        .hash_password(guid.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Argon2 哈希失败: {}", e))?;

    // 5. 提取哈希值（PHC 格式）
    let hash_bytes = password_hash
        .hash
        .ok_or_else(|| anyhow::anyhow!("哈希值为空"))?;

    // 6. 取前 32 字节作为密钥
    let mut key = [0u8; 32];
    let hash_slice = hash_bytes.as_bytes();

    if hash_slice.len() < 32 {
        anyhow::bail!("哈希长度不足 32 字节（实际: {}）", hash_slice.len());
    }

    key.copy_from_slice(&hash_slice[..32]);

    tracing::debug!("加密密钥派生成功（256-bit）");

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_get_machine_guid() {
        let guid = get_machine_guid().unwrap();
        assert!(!guid.is_empty());
        assert!(guid.len() >= 32); // GUID 格式通常是 32 个字符（无连字符）
        println!("机器 GUID: {}", guid);
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_derive_encryption_key() {
        let key1 = derive_encryption_key().unwrap();
        let key2 = derive_encryption_key().unwrap();

        // 相同机器上派生结果应该一致
        assert_eq!(key1, key2);

        // 密钥长度正确
        assert_eq!(key1.len(), 32);

        println!("密钥派生成功: {:?}...{:?}", &key1[..4], &key1[28..]);
    }

    #[test]
    fn test_fixed_salt_consistency() {
        // 确保固定盐值不会意外修改
        assert_eq!(FIXED_SALT, b"NanoMail.v1.2025");
        assert_eq!(FIXED_SALT.len(), 16);
    }
}
