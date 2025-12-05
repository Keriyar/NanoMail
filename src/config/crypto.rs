/// Token 加密/解密模块
///
/// 使用 AES-256-GCM 对敏感数据（如 OAuth2 Token）进行加密存储
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::utils::machine_id;

/// 加密前缀（用于识别加密数据）
const ENCRYPTED_PREFIX: &str = "encrypted:";

/// AES-GCM Nonce 长度（12 字节）
const NONCE_SIZE: usize = 12;

/// 加密明文 Token
///
/// 使用 AES-256-GCM 模式加密数据，密钥从机器 GUID 派生
///
/// # 数据格式
/// 返回格式：`"encrypted:" + Base64(nonce[12 bytes] + ciphertext)`
///
/// # Arguments
/// * `plain` - 待加密的明文字符串
///
/// # Returns
/// 加密后的 Base64 字符串，带 `encrypted:` 前缀
///
/// # Errors
/// - 密钥派生失败
/// - 加密失败
///
/// # Example
/// ```no_run
/// let encrypted = encrypt_token("my_secret_token")?;
/// assert!(encrypted.starts_with("encrypted:"));
/// ```
pub fn encrypt_token(plain: &str) -> Result<String> {
    // 1. 获取加密密钥（从机器指纹派生）
    let key_bytes = machine_id::derive_encryption_key()
        .context("无法派生加密密钥")?;

    // 2. 创建 AES-256-GCM 密码器
    let cipher = Aes256Gcm::new(&key_bytes.into());

    // 3. 生成随机 nonce（12 字节）
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // 4. 加密数据
    let ciphertext = cipher
        .encrypt(&nonce, plain.as_bytes())
        .map_err(|e| anyhow::anyhow!("AES-GCM 加密失败: {}", e))?;

    // 5. 组合：nonce + ciphertext
    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce);
    combined.extend_from_slice(&ciphertext);

    // 6. Base64 编码
    let encoded = BASE64.encode(&combined);

    // 7. 添加前缀
    Ok(format!("{}{}", ENCRYPTED_PREFIX, encoded))
}

/// 解密加密的 Token
///
/// 解析 `encrypted:` 前缀的 Base64 数据并解密
///
/// # Arguments
/// * `encrypted` - 加密后的字符串（必须以 `encrypted:` 开头）
///
/// # Returns
/// 解密后的明文字符串
///
/// # Errors
/// - 格式错误（缺少前缀）
/// - Base64 解码失败
/// - 数据长度不足
/// - 密钥派生失败
/// - 解密失败（密钥错误或数据损坏）
///
/// # Example
/// ```no_run
/// let plain = decrypt_token("encrypted:SGVs...")?;
/// println!("解密成功: {}", plain);
/// ```
pub fn decrypt_token(encrypted: &str) -> Result<String> {
    // 1. 检查前缀
    if !encrypted.starts_with(ENCRYPTED_PREFIX) {
        anyhow::bail!("加密数据格式错误：缺少 'encrypted:' 前缀");
    }

    // 2. 去除前缀并 Base64 解码
    let base64_data = &encrypted[ENCRYPTED_PREFIX.len()..];
    let combined = BASE64
        .decode(base64_data)
        .context("Base64 解码失败")?;

    // 3. 检查数据长度（至少包含 nonce）
    if combined.len() < NONCE_SIZE {
        anyhow::bail!(
            "加密数据长度不足（需要至少 {} 字节，实际 {} 字节）",
            NONCE_SIZE,
            combined.len()
        );
    }

    // 4. 分离 nonce 和 ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // 5. 获取加密密钥
    let key_bytes = machine_id::derive_encryption_key()
        .context("无法派生解密密钥")?;

    // 6. 创建密码器并解密
    let cipher = Aes256Gcm::new(&key_bytes.into());
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("AES-GCM 解密失败（可能密钥错误或数据损坏）: {}", e))?;

    // 7. 转换为 UTF-8 字符串
    let result = String::from_utf8(plaintext)
        .context("解密后的数据不是有效的 UTF-8 字符串")?;

    Ok(result)
}

/// 检查字符串是否为加密格式
///
/// # Example
/// ```
/// assert!(is_encrypted("encrypted:abc..."));
/// assert!(!is_encrypted("plain_text"));
/// ```
pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENCRYPTED_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 需要在 Windows 环境运行（依赖机器 GUID）
    fn test_encrypt_decrypt_roundtrip() {
        let plain = "test_access_token_12345";

        // 加密
        let encrypted = encrypt_token(plain).unwrap();
        println!("加密结果: {}...{}", &encrypted[..20], &encrypted[encrypted.len()-10..]);

        // 验证格式
        assert!(encrypted.starts_with(ENCRYPTED_PREFIX));
        assert!(encrypted.len() > ENCRYPTED_PREFIX.len() + NONCE_SIZE);

        // 解密
        let decrypted = decrypt_token(&encrypted).unwrap();

        // 验证往返一致性
        assert_eq!(plain, decrypted);
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_encrypt_different_nonce() {
        let plain = "same_token";

        // 两次加密应产生不同结果（因为 nonce 随机）
        let encrypted1 = encrypt_token(plain).unwrap();
        let encrypted2 = encrypt_token(plain).unwrap();

        assert_ne!(encrypted1, encrypted2);

        // 但都能正确解密
        assert_eq!(decrypt_token(&encrypted1).unwrap(), plain);
        assert_eq!(decrypt_token(&encrypted2).unwrap(), plain);
    }

    #[test]
    fn test_is_encrypted() {
        assert!(is_encrypted("encrypted:SGVsbG8="));
        assert!(!is_encrypted("plain_text"));
        assert!(!is_encrypted(""));
    }

    #[test]
    fn test_decrypt_invalid_format() {
        // 缺少前缀
        let result = decrypt_token("SGVsbG8gV29ybGQ=");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("缺少 'encrypted:' 前缀"));
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        // 无效的 Base64
        let result = decrypt_token("encrypted:!!!invalid@@@");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Base64"));
    }

    #[test]
    fn test_decrypt_too_short() {
        // 数据长度不足（少于 12 字节 nonce）
        let short_data = BASE64.encode(b"short");
        let result = decrypt_token(&format!("encrypted:{}", short_data));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("长度不足"));
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_decrypt_corrupted_data() {
        // 加密一个有效 token
        let plain = "valid_token";
        let mut encrypted = encrypt_token(plain).unwrap();

        // 损坏密文（修改最后一个字符）
        encrypted.pop();
        encrypted.push('X');

        // 解密应失败
        let result = decrypt_token(&encrypted);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("解密失败"));
    }

    #[test]
    #[ignore] // 需要在 Windows 环境运行
    fn test_encrypt_unicode() {
        // 测试 Unicode 字符
        let plain = "测试Token🔒";
        let encrypted = encrypt_token(plain).unwrap();
        let decrypted = decrypt_token(&encrypted).unwrap();
        assert_eq!(plain, decrypted);
    }
}
