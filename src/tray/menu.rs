// 托盘右键菜单模块

use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};

pub struct MenuIds {
    pub open_gmail: MenuId,
    pub about: MenuId,
    pub quit: MenuId,
}

pub fn create_menu_with_ids() -> Result<(Menu, MenuIds)> {
    let menu = Menu::new();

    let open_gmail = MenuItem::new("打开 Gmail", true, None);
    let about = MenuItem::new("关于 NanoMail", true, None);
    // 在托盘菜单中显示为“推出”——此项将真正结束程序
    let quit = MenuItem::new("退出", true, None);

    menu.append_items(&[
        &open_gmail,
        &PredefinedMenuItem::separator(),
        &about,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let ids = MenuIds {
        open_gmail: open_gmail.id().clone(),
        about: about.id().clone(),
        quit: quit.id().clone(),
    };

    Ok((menu, ids))
}
