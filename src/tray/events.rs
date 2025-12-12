// 托盘事件处理模块

use std::sync::mpsc;
use tray_icon::{TrayIconEvent, menu::MenuEvent};

/// 托盘 → Slint 窗口的命令
#[derive(Debug, Clone)]
pub enum TrayCommand {
    ToggleWindow,
    ShowWindow,
    HideWindow,
    OpenGmail,
    ShowAbout,
    Exit,
}

/// Slint 窗口 → 托盘的命令（用于更新图标状态）
#[derive(Debug, Clone)]
pub enum WindowCommand {
    UpdateIcon(TrayIconState),
}

#[derive(Debug, Clone, Copy)]
pub enum TrayIconState {
    Normal,
    Unread,
    Error,
}

/// 运行托盘事件循环
pub fn run_event_loop(menu_ids: super::menu::MenuIds, tx: mpsc::Sender<TrayCommand>) {
    let menu_channel = tray_icon::menu::MenuEvent::receiver();
    let tray_channel = tray_icon::TrayIconEvent::receiver();

    loop {
        // 检查菜单事件
        if let Ok(event) = menu_channel.try_recv() {
            tracing::debug!("托盘菜单事件: {:?}", event);
            handle_menu_event(event, &menu_ids, &tx);
        }

        // 检查托盘图标事件
        if let Ok(event) = tray_channel.try_recv() {
            tracing::debug!("托盘图标事件: {:?}", event);
            handle_tray_event(event, &tx);
        }

        // 降低 CPU 占用
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn handle_menu_event(
    event: MenuEvent,
    menu_ids: &super::menu::MenuIds,
    tx: &mpsc::Sender<TrayCommand>,
) {
    // 记录完整的事件信息
    tracing::info!("收到托盘菜单事件: {:?}", event);

    let menu_id = event.id;

    // 直接比较菜单 ID，不再依赖字符串匹配
    if menu_id == menu_ids.open_gmail {
        tracing::info!("菜单事件: 打开 Gmail");
        if let Err(e) = tx.send(TrayCommand::OpenGmail) {
            tracing::error!("发送 OpenGmail 命令失败: {:?}", e);
        }
    } else if menu_id == menu_ids.about {
        tracing::info!("菜单事件: 关于");
        if let Err(e) = tx.send(TrayCommand::ShowAbout) {
            tracing::error!("发送 ShowAbout 命令失败: {:?}", e);
        }
    } else if menu_id == menu_ids.quit {
        tracing::info!("菜单事件: 退出");
        if let Err(e) = tx.send(TrayCommand::Exit) {
            tracing::error!("发送 Exit 命令失败: {:?}", e);
        }
    } else {
        tracing::warn!("未识别的菜单 ID: {:?}", menu_id);
    }
}

fn handle_tray_event(event: TrayIconEvent, tx: &mpsc::Sender<TrayCommand>) {
    tracing::debug!("handle_tray_event: {:?}", event);
    if let TrayIconEvent::Click {
        button: tray_icon::MouseButton::Left,
        ..
    } = event
    {
        tracing::debug!("托盘左键点击 -> ToggleWindow");
        tx.send(TrayCommand::ToggleWindow).ok();
    }
}
