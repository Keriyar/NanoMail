/// 头像处理模块
///
/// 负责下载头像并生成缩略图，减少内存占用
use image::imageops::FilterType;
use image::GenericImageView;
use std::path::PathBuf;

use super::http_client;

/// 缩略图尺寸（与 UI 中头像显示尺寸匹配）
const THUMBNAIL_SIZE: u32 = 48;

/// 下载头像并生成缩略图，返回本地缓存路径
///
/// # Arguments
/// * `url` - 头像 URL
/// * `email` - 用户邮箱（用于生成文件名）
///
/// # Returns
/// 成功返回本地缓存路径，失败返回 None
pub async fn download_and_resize_avatar(url: &str, email: &str) -> Option<String> {
    tracing::debug!("下载头像: {} -> {}", email, url);

    // 1. 下载图片
    let resp = match http_client::get_client().get(url).send().await {
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

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("读取头像响应体失败: {}", e);
            return None;
        }
    };

    // 2. 解码图片
    let img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(e) => {
            tracing::warn!("解码头像失败: {}", e);
            return None;
        }
    };

    // 3. 生成缩略图（48x48）
    let thumbnail = img.resize_exact(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);
    tracing::debug!(
        "头像缩略图生成: {}x{} -> {}x{}",
        img.width(),
        img.height(),
        THUMBNAIL_SIZE,
        THUMBNAIL_SIZE
    );

    // 4. 构建缓存路径
    let cache_dir = match dirs::config_dir() {
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

    // 文件名使用邮箱安全化 + 固定 PNG 格式（缩略图统一格式）
    let safe_name = email.replace('@', "_").replace('.', "_");
    let path: PathBuf = cache_dir.join(format!("{}_thumb.png", safe_name));

    // 5. 保存缩略图（PNG 格式，质量好且支持透明）
    if let Err(e) = thumbnail.save(&path) {
        tracing::warn!("保存头像缩略图失败: {}", e);
        return None;
    }

    let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    tracing::info!(
        "✓ 头像缩略图已缓存: {} ({} bytes)",
        path.display(),
        file_size
    );

    Some(path.display().to_string())
}

/// 获取已缓存的头像路径（如果存在）
pub fn get_cached_avatar_path(email: &str) -> Option<String> {
    let cache_dir = dirs::config_dir()?.join("NanoMail").join("avatars");
    let safe_name = email.replace('@', "_").replace('.', "_");
    let path = cache_dir.join(format!("{}_thumb.png", safe_name));

    if path.exists() {
        Some(path.display().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_size() {
        assert_eq!(THUMBNAIL_SIZE, 48);
    }

    #[test]
    fn test_get_cached_avatar_path_not_exists() {
        let result = get_cached_avatar_path("nonexistent@test.com");
        // 可能存在也可能不存在，只测试不会 panic
        let _ = result;
    }
}
