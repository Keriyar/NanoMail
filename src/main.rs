#![windows_subsystem = "windows"] // 临时禁用以查看日志

// 导入 Slint 生成的代码
slint::include_modules!();

use anyhow::Result;
use slint::Model;
use std::sync::{Arc, mpsc};

mod config;
mod mail;
mod sync;
mod tray;
mod ui;
mod utils;

fn main() -> Result<()> {
    // 1. 初始化日志
    init_logger()?;

    // 2. 创建 Tokio 运行时（用于 async OAuth2）
    let rt = tokio::runtime::Runtime::new()?;
    let rt_handle = rt.handle().clone();

    // 3. 创建通信通道
    let (tray_tx, tray_rx) = mpsc::channel::<tray::TrayCommand>();

    // 4. 创建 Slint UI
    let main_window = MainWindow::new()?;

    // 5. 加载已保存的账户
    let saved_accounts = match config::storage::load_accounts() {
        Ok(accounts) if !accounts.is_empty() => {
            tracing::info!("✅ 从文件加载 {} 个账户", accounts.len());
            accounts
        }
        Ok(_) => {
            tracing::info!("📭 无已保存账户");
            vec![]
        }
        Err(e) => {
            tracing::warn!("⚠️ 加载账户失败: {}, 使用空列表", e);
            vec![]
        }
    };

    // 转换为 Slint 类型
    let slint_accounts: Vec<Account> = saved_accounts.into_iter().map(|acc| acc.into()).collect();

    let account_model = slint::VecModel::from(slint_accounts);
    main_window.set_accounts(std::rc::Rc::new(account_model).into());

    // 6. 设置初始应用状态为 Normal（绿色 N）
    main_window.set_app_status("normal".into());
    tracing::debug!("应用状态初始化: Normal (绿色 N)");
    tracing::info!("app_status set -> normal (初始化)");

    // 6.1 从配置加载并初始化主题
    if let Ok(cfg) = config::load() {
        let is_dark = cfg.app.theme == "dark";
        Theme::get(&main_window).set_is_dark(is_dark);
        tracing::info!("主题初始化: {}", if is_dark { "dark" } else { "light" });
    }

    // 7. 创建系统托盘
    let _tray_handle = tray::create_tray_icon(tray_tx.clone())?;

    // 8. 绑定 Slint 回调（传入 Tokio 运行时）
    bind_callbacks(&main_window, rt_handle.clone())?;

    // 9. 启动同步引擎
    let sync_engine = Arc::new(sync::SyncEngine::new(rt_handle.clone()));
    let window_weak_for_sync = main_window.as_weak();

    sync_engine.start(move |email, res| {
        match res {
            Ok(sync_info) => {
                tracing::info!(
                    "[DEBUG-UNREAD] 回调收到: email={}, unread_count={}",
                    email, sync_info.unread_count
                );

                // 更新UI（必须在事件循环中）
                let weak = window_weak_for_sync.clone();
                let sync_info_cloned = sync_info.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        update_account_sync_info(&window, sync_info_cloned.clone());

                        // 优先检查网络问题：若同步过程中曾检测到网络问题，显示红色
                        if sync_info_cloned.network_issue {
                            window.set_app_status("error".into());
                            tracing::info!("app_status set -> error (network_issue)");
                            tracing::error!(
                                "账户 {} 同步过程中检测到网络问题",
                                sync_info_cloned.email
                            );
                            // 网络和 Token 均正常 -> 绿色
                            window.set_app_status("normal".into());
                        }
                    }
                })
                .ok();
            }
            Err(err_msg) => {
                tracing::error!("同步账户失败: {} -> {}", email, err_msg);

                // 构造带错误信息的 AccountSyncInfo 以更新 UI（标为 has_error）
                let info = mail::gmail::AccountSyncInfo {
                    email: email.clone(),
                    unread_count: 0,
                    avatar_url: String::new(),
                    display_name: email.clone(),
                    error_message: Some(err_msg.clone()),
                    network_issue: true,
                };

                let weak = window_weak_for_sync.clone();
                let err_clone = err_msg.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        update_account_sync_info(&window, info);

                        // 网络不可用 -> 红色；Token或其他错误 -> 也是红色（用户要求）
                        window.set_app_status("error".into());
                        tracing::info!("app_status set -> error (callback Err: {})", err_clone);
                    }
                })
                .ok();
            }
        }
    });

    // 10. 启动托盘事件监听线程（传入 SyncEngine 引用与退出信号以便优雅退出）
    let window_weak = main_window.as_weak();
    let tray_sync = sync_engine.clone();
    // 创建退出信号通道，主线程将在 UI 事件循环返回后等待此信号
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let shutdown_tx_clone = shutdown_tx.clone();
    std::thread::spawn(move || {
        handle_tray_commands(tray_rx, window_weak, tray_sync, shutdown_tx_clone);
    });

    // 11. 窗口初始显示（默认在启动时打开主界面）
    tracing::info!("NanoMail v0.1.0 启动，显示主界面于右下角");
    tray::show_window_near_tray(&main_window);

    // 12. 运行 Slint 全局事件循环（保持运行，即使窗口被隐藏）
    // 使用 run_event_loop_until_quit() 确保即使窗口隐藏也能继续处理事件
    let _ = slint::run_event_loop_until_quit();

    // 当我们在托盘点击“推出”时，托盘线程会调用 slint::quit_event_loop(),
    // 此时全局事件循环返回，我们在这里等待托盘线程发送的退出信号以完成清理。
    tracing::debug!("事件循环已退出，等待托盘线程的退出信号以完成优雅关机...");
    let _ = shutdown_rx.recv();

    tracing::info!("收到推出信号，开始优雅关机...");
    sync_engine.request_stop();
    std::thread::sleep(std::time::Duration::from_millis(200));

    Ok(())
}

/// 处理托盘命令（在独立线程中运行）
fn handle_tray_commands(
    rx: mpsc::Receiver<tray::TrayCommand>,
    window_weak: slint::Weak<MainWindow>,
    sync_engine: std::sync::Arc<sync::SyncEngine>,
    shutdown_tx: mpsc::Sender<()>,
) {
    while let Ok(cmd) = rx.recv() {
        let weak = window_weak.clone();

        // 对于可能影响运行时或需要先停止后台任务的命令，优先处理
        match cmd {
            tray::TrayCommand::Exit => {
                tracing::info!("========================================");
                tracing::info!("托盘收到退出命令，开始优雅关机流程");
                tracing::info!("========================================");

                // 请求同步引擎停止（同步接口）
                sync_engine.request_stop();

                // 在主线程执行 UI 隐藏并退出事件循环
                let quit_result = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        tracing::info!("退出流程: 隐藏窗口");
                        window.hide().ok();
                    }
                    tracing::info!("退出流程: 调用 quit_event_loop()");
                    match slint::quit_event_loop() {
                        Ok(_) => tracing::info!("退出流程: quit_event_loop() 成功"),
                        Err(e) => tracing::error!("退出流程: quit_event_loop() 失败: {:?}", e),
                    }
                });

                match quit_result {
                    Ok(_) => tracing::info!("退出流程: invoke_from_event_loop 成功"),
                    Err(e) => {
                        tracing::error!("退出流程: invoke_from_event_loop 失败: {:?}", e);
                        // 如果 invoke_from_event_loop 失败，强制退出
                        tracing::error!("退出流程: 尝试强制退出...");
                        std::process::exit(0);
                    }
                }

                // 发送退出信号给主线程以完成优雅退出（主线程在 run_event_loop() 返回后会等待此信号）
                if let Err(e) = shutdown_tx.send(()) {
                    tracing::error!("退出流程: 发送退出信号失败: {:?}", e);
                }

                tracing::info!("退出流程: 托盘线程退出循环");
                // 退出循环
                break;
            }
            _ => {}
        }

        // 确保 UI 更新在主线程执行
        let sync_engine_clone = sync_engine.clone();
        let result = slint::invoke_from_event_loop(move || {
            if let Some(window) = weak.upgrade() {
                match cmd {
                    tray::TrayCommand::ToggleWindow => {
                        tracing::info!("处理托盘命令: ToggleWindow");
                        // 如果窗口将要显示，触发立即同步
                        if !window.window().is_visible() {
                            sync_engine_clone.trigger_sync();
                        }
                        tray::toggle_window(&window);
                    }
                    tray::TrayCommand::ShowWindow => {
                        tracing::info!("处理托盘命令: ShowWindow");
                        sync_engine_clone.trigger_sync();
                        tray::show_window_near_tray(&window);
                    }
                    tray::TrayCommand::HideWindow => {
                        tracing::info!("处理托盘命令: HideWindow");
                        window.hide().ok();
                    }
                    tray::TrayCommand::OpenGmail => {
                        tracing::info!("处理托盘命令: OpenGmail");
                        open_gmail();
                    }
                    tray::TrayCommand::ShowAbout => {
                        tracing::info!("处理托盘命令: ShowAbout");
                        show_about_dialog();
                    }
                    _ => {}
                }
            } else {
                tracing::warn!("窗口引用已失效，无法处理托盘命令");
            }
        });

        if let Err(e) = result {
            tracing::error!("invoke_from_event_loop 失败: {:?}", e);
        }
    }
}

fn show_about_dialog() {
    tracing::info!("显示关于对话框");
    // MVP: 打开 GitHub 页面
    webbrowser::open("https://github.com/Keriyar/NanoMail").ok();
}

fn open_gmail() {
    let url = "https://mail.google.com/mail/u/0/#inbox";
    if let Err(e) = webbrowser::open(url) {
        tracing::error!("无法打开浏览器: {}", e);
    }
}

/// 绑定所有 Slint 回调
fn bind_callbacks(
    main_window: &MainWindow,
    rt_handle: tokio::runtime::Handle,
) -> Result<()> {
    // 主题切换
    main_window.on_theme_toggled({
        let weak = main_window.as_weak();
        move || {
            tracing::info!("[回调] 主题切换按钮被点击");
            if let Some(window) = weak.upgrade() {
                // 切换主题
                let current_is_dark = Theme::get(&window).get_is_dark();
                let new_is_dark = !current_is_dark;
                Theme::get(&window).set_is_dark(new_is_dark);
                tracing::info!("主题切换: {} -> {}", 
                    if current_is_dark { "dark" } else { "light" },
                    if new_is_dark { "dark" } else { "light" }
                );

                // 持久化主题偏好
                if let Ok(mut cfg) = config::load() {
                    cfg.app.theme = if new_is_dark { "dark".to_string() } else { "light".to_string() };
                    if let Err(e) = config::save(&cfg) {
                        tracing::error!("保存主题配置失败: {}", e);
                    }
                }
            }
        }
    });

    // 添加账户（集成 OAuth2）
    main_window.on_add_account_clicked({
        let window_weak = main_window.as_weak();

        move || {
            tracing::info!("[回调] 添加账户按钮被点击");

            let weak = window_weak.clone();
            let handle = rt_handle.clone();

            std::thread::spawn(move || {
                handle.block_on(async {
                    // 执行 OAuth2 认证
                    match mail::gmail::authenticate().await {
                        Ok(account) => {
                            tracing::info!("✅ OAuth2 成功: {}", account.email);

                            // 立即同步账户信息（获取未读数）
                            let (sync_info, updated_account) =
                                match mail::gmail::sync_account_info(&account).await {
                                    Ok((info, updated)) => (Some(info), updated),
                                    Err(e) => {
                                        tracing::error!("立即同步失败: {}", e);
                                        (None, None)
                                    }
                                };

                            // 使用更新后的账户（如果 Token 被刷新）
                            let final_account = updated_account.unwrap_or(account);

                            // 更新 UI（必须在事件循环中）
                            slint::invoke_from_event_loop(move || {
                                if let Some(window) = weak.upgrade() {
                                    update_accounts_ui(&window, final_account, sync_info);
                                }
                            })
                            .ok();
                        }
                        Err(e) => {
                            tracing::error!("❌ OAuth2 失败: {}", e);
                            // TODO: 显示错误对话框
                        }
                    }
                });
            });
        }
    });

    // 打开 Gmail
    main_window.on_open_gmail_clicked({
        move || {
            tracing::info!("[回调] 打开 Gmail 按钮被点击");
            open_gmail();
        }
    });

    // 反馈按钮
    main_window.on_feedback_clicked({
        move || {
            tracing::info!("[回调] 反馈按钮被点击");
            let url = "https://github.com/Keriyar/NanoMail";
            webbrowser::open(url).ok();
        }
    });

    // 窗口中的“隐藏到托盘”按钮（之前名为退出）
    main_window.on_minimize_clicked({
        let weak = main_window.as_weak();
        move || {
            tracing::info!("[回调] 隐藏到托盘按钮被点击，隐藏窗口");
            if let Some(window) = weak.upgrade() {
                window.hide().ok();
            }
        }
    });

    // 头像重试
    main_window.on_avatar_retry({
        move |index| {
            tracing::info!("[回调] 头像重试: 账户索引 {}", index);
            // TODO: 阶段4 实现头像重新加载
        }
    });

    Ok(())
}

/// 将新账户添加到 UI 列表
fn update_accounts_ui(
    window: &MainWindow,
    gmail_account: mail::gmail::GmailAccount,
    sync_info: Option<mail::gmail::AccountSyncInfo>,
) {
    use slint::VecModel;
    use std::rc::Rc;

    // 转换为 Slint Account 类型
    let mut slint_account: Account = gmail_account.into();

    // 如果有同步信息，更新未读数和头像
    if let Some(info) = sync_info {
        slint_account.unread_count = info.unread_count as i32;

        // 将头像路径转换为 Slint Image（若路径为空或加载失败则使用默认 image）
        if !info.avatar_url.is_empty() {
            match slint::Image::load_from_path(std::path::Path::new(&info.avatar_url)) {
                Ok(img) => slint_account.avatar_image = img,
                Err(_) => slint_account.avatar_image = slint::Image::default(),
            }
        } else {
            slint_account.avatar_image = slint::Image::default();
        }
    }

    // 获取现有账户列表
    let accounts = window.get_accounts();
    let mut new_accounts = Vec::new();

    for i in 0..accounts.row_count() {
        if let Some(acc) = accounts.row_data(i) {
            new_accounts.push(acc);
        }
    }

    // 添加新账户
    new_accounts.push(slint_account);

    let account_count = new_accounts.len();

    // 更新 UI
    let model = VecModel::from(new_accounts);
    window.set_accounts(Rc::new(model).into());

    tracing::info!("UI 已更新：显示 {} 个账户", account_count);
}

/// 更新账户同步信息（未读数、头像和错误状态）
fn update_account_sync_info(window: &MainWindow, sync_info: mail::gmail::AccountSyncInfo) {
    use slint::VecModel;
    use std::rc::Rc;

    let accounts = window.get_accounts();
    let mut new_accounts = Vec::new();

    // 找到对应账户并更新
    for i in 0..accounts.row_count() {
        if let Some(mut acc) = accounts.row_data(i) {
            if acc.email.as_str() == sync_info.email {
                // 若同步成功，更新未读数；若失败则保持旧值（或者在 AccountSyncInfo 里处理逻辑）
                // 当前逻辑：sync_info 包含即时数据。如果失败，external sync_info.unread_count 默认为0
                // 但 callback 处理时手动构造了 unread_count=0 的 info
                // 这里我们要判断：如果 error_message 存在，则忽略 unread_count 的更新，仅更新错误状态
                if sync_info.error_message.is_none() {
                    tracing::info!(
                        "[DEBUG-UNREAD] UI更新前: 旧值={}, 新值={}",
                        acc.unread_count, sync_info.unread_count
                    );
                    acc.unread_count = sync_info.unread_count as i32;
                    tracing::info!(
                        "[DEBUG-UNREAD] UI更新后: acc.unread_count={}",
                        acc.unread_count
                    );
                }
                if !sync_info.avatar_url.is_empty() {
                    match slint::Image::load_from_path(std::path::Path::new(&sync_info.avatar_url))
                    {
                        Ok(img) => acc.avatar_image = img,
                        Err(_) => acc.avatar_image = slint::Image::default(),
                    }
                } else {
                    acc.avatar_image = slint::Image::default();
                }

                // 如果有错误，标记为 has_error 并显示错误消息
                if let Some(error_msg) = &sync_info.error_message {
                    acc.has_error = true;
                    tracing::error!("❌ 账户 {} 同步失败: {}", sync_info.email, error_msg);
                } else {
                    acc.has_error = false;
                }

                tracing::debug!(
                    "更新账户 {} 未读数: {} (错误: {})",
                    sync_info.email,
                    sync_info.unread_count,
                    sync_info.error_message.as_deref().unwrap_or("无")
                );
            }
            new_accounts.push(acc);
        }
    }

    // 更新 UI
    let model = VecModel::from(new_accounts);
    window.set_accounts(Rc::new(model).into());
}

/// 初始化日志系统
fn init_logger() -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nanomail=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
