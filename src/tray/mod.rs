// 系统托盘模块

use anyhow::Result;
use screen_size::get_primary_screen_size;
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
    let is_visible = window.window().is_visible();
    tracing::info!("toggle_window: 当前窗口可见性 = {}", is_visible);

    if is_visible {
        tracing::info!("toggle_window: 隐藏窗口");
        window.hide().ok();
    } else {
        tracing::info!("toggle_window: 显示窗口");
        show_window_near_tray(window);
    }
}

/// 在托盘附近显示窗口（尽量放置在右下角，留出任务栏空间）
pub fn show_window_near_tray<T: ComponentHandle>(window: &T) {
    tracing::info!("show_window_near_tray: 开始显示窗口");

    // 尝试动态获取主显示器分辨率，回退到默认值
    let (screen_width, screen_height) = match get_primary_screen_size() {
        Ok((w, h)) => (w as i32, h as i32),
        Err(e) => {
            tracing::warn!("无法获取屏幕尺寸: {}, 使用默认值 1920x1080", e);
            (1920, 1080)
        }
    };

    // 设计的窗口尺寸
    let window_width = 380i32;
    let window_height = 400i32;

    // 在右下角上方显示（留出任务栏和边距）
    let x = screen_width - window_width - 97;
    let y = screen_height - window_height - 50;

    tracing::info!("show_window_near_tray: 设置窗口位置 x={}, y={}", x, y);
    window
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));

    tracing::info!("show_window_near_tray: 调用 window.show()");
    if let Err(e) = window.show() {
        tracing::error!("show_window_near_tray: 显示窗口失败: {:?}", e);
    } else {
        tracing::info!("show_window_near_tray: 窗口已显示");
    }
}
