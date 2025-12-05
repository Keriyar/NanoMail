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
            handle_menu_event(event, &menu_ids, &tx);
        }

        // 检查托盘图标事件
        if let Ok(event) = tray_channel.try_recv() {
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
    let menu_id = event.id;

    if menu_id == menu_ids.open_gmail {
        tx.send(TrayCommand::OpenGmail).ok();
    } else if menu_id == menu_ids.about {
        tx.send(TrayCommand::ShowAbout).ok();
    } else if menu_id == menu_ids.quit {
        tx.send(TrayCommand::Exit).ok();
    }
}

fn handle_tray_event(event: TrayIconEvent, tx: &mpsc::Sender<TrayCommand>) {
    if let TrayIconEvent::Click {
        button: tray_icon::MouseButton::Left,
        ..
    } = event
    {
        tx.send(TrayCommand::ToggleWindow).ok();
    }
}
