/// Windows 原生 Toast 通知模块
///
/// 使用 WinRT API 发送系统级通知，显示在 Windows 通知中心
use winrt_toast_reborn::{Toast, ToastManager};

/// 获取或创建 ToastManager
/// 使用 PowerShell 的 AUMID 作为临时方案
fn get_toast_manager() -> ToastManager {
    ToastManager::new(ToastManager::POWERSHELL_AUM_ID)
}

/// 显示新邮件系统通知
///
/// 通知会显示在 Windows 右下角，并进入通知中心
///
/// # Arguments
/// * `email` - 账户邮箱
/// * `new_count` - 新增的未读邮件数量
pub fn show_new_mail_notification(email: &str, new_count: u32) {
    let manager = get_toast_manager();
    
    // 构建通知内容
    let title = "📬 NanoMail - 新邮件";
    let body = if new_count == 1 {
        format!("{} 收到 1 封新邮件", email)
    } else {
        format!("{} 收到 {} 封新邮件", email, new_count)
    };
    
    // 创建 Toast 通知
    let mut toast = Toast::new();
    toast
        .text1(title)
        .text2(&body);
    
    // 发送通知
    match manager.show(&toast) {
        Ok(_) => {
            tracing::info!("✅ 已发送新邮件通知: {} (+{} 封)", email, new_count);
        }
        Err(e) => {
            tracing::error!("❌ 发送通知失败: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // 需要在 Windows 环境下运行
    fn test_show_notification() {
        show_new_mail_notification("test@gmail.com", 3);
    }
}
