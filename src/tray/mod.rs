// 系统托盘模块

use anyhow::Result;
use slint::ComponentHandle;
use std::sync::mpsc;
use tray_icon::{TrayIcon, TrayIconBuilder};

mod events;
mod icon;
mod menu;

pub use events::{TrayCommand, TrayIconState, WindowCommand};

/// 创建系统托盘图标
pub fn create_tray_icon(tx: mpsc::Sender<TrayCommand>) -> Result<TrayIcon> {
    // 1. 加载图标
    let icon = icon::load_icon(TrayIconState::Normal)?;

    // 2. 创建菜单
    let (menu, menu_ids) = menu::create_menu_with_ids()?;

    // 3. 构建托盘图标
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NanoMail - Gmail 通知客户端")
        .with_icon(icon)
        .build()?;

    tracing::info!("系统托盘图标已创建");

    // 4. 启动事件循环
    std::thread::spawn(move || {
        tracing::debug!("托盘事件循环已启动");
        events::run_event_loop(menu_ids, tx);
    });

    Ok(tray)
}

/// 切换窗口显示/隐藏
pub fn toggle_window<T: ComponentHandle>(window: &T) {
    if window.window().is_visible() {
        tracing::debug!("隐藏窗口");
        window.hide().ok();
    } else {
        tracing::debug!("显示窗口");
        show_window_near_tray(window);
    }
}

/// 在托盘附近显示窗口（MVP: 固定右下角）
pub fn show_window_near_tray<T: ComponentHandle>(window: &T) {
    // TODO: 从 Windows API 获取真实屏幕尺寸
    let screen_width = 1920;
    let screen_height = 1080;
    let window_width = 380;
    let window_height = 400;

    let x = screen_width - window_width - 20;
    let y = screen_height - window_height - 80;

    window
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));

    window.show().ok();
}
