// 托盘图标资源加载模块

use anyhow::Result;
use image::GenericImageView;
use tray_icon::Icon;

use super::events::TrayIconState; // 保留以兼容现有接口

/// 编译时嵌入托盘图标文件（避免运行时依赖外部文件）
const ICON_BYTES: &[u8] = include_bytes!("../../assets/icons/NanoMail.ico");

/// 加载托盘图标(忽略状态,始终使用 NanoMail.ico)
pub fn load_icon(_state: TrayIconState) -> Result<Icon> {
    load_icon_from_memory(ICON_BYTES)
}

/// 从内存加载图标（支持 PNG/ICO 等格式）
fn load_icon_from_memory(img_bytes: &[u8]) -> Result<Icon> {
    tracing::debug!("从嵌入资源加载托盘图标（{} bytes）", img_bytes.len());

    // 使用 image crate 从内存解码（自动检测格式）
    let img = image::load_from_memory(img_bytes)
        .map_err(|e| anyhow::anyhow!("图标解码失败: {}", e))?;

    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let icon = Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|e| anyhow::anyhow!("图标创建失败: {:?}", e))?;

    tracing::info!("✓ 成功加载托盘图标（{}x{}）", width, height);
    Ok(icon)
}
