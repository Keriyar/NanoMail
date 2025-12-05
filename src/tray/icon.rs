// 托盘图标资源加载模块

use anyhow::Result;
use image::GenericImageView;
use tray_icon::Icon;

use super::events::TrayIconState; // 保留以兼容现有接口

/// 加载托盘图标(忽略状态,始终使用 NanoMail.ico)
pub fn load_icon(_state: TrayIconState) -> Result<Icon> {
    const ICON_PATH: &str = "assets/icons/NanoMail.ico";

    load_icon_from_file(ICON_PATH)
}

/// 从图像文件加载图标（支持 PNG/ICO 等格式）
fn load_icon_from_file(path: &str) -> Result<Icon> {
    tracing::debug!("加载托盘图标: {}", path);

    // 读取文件内容
    let img_bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("图标文件读取失败 [{}]: {}", path, e))?;

    // 使用 image crate 从内存解码（自动检测格式，忽略扩展名）
    let img = image::load_from_memory(&img_bytes)
        .map_err(|e| anyhow::anyhow!("图标解码失败: {}", e))?;

    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let icon = Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|e| anyhow::anyhow!("图标创建失败: {:?}", e))?;

    tracing::info!("✓ 成功加载托盘图标: {}", path);
    Ok(icon)
}
