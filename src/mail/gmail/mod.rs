/// Gmail 模块 - OAuth2 认证与 API 调用
pub mod api;
pub mod oauth;
pub mod token;
pub mod types;

// 重新导出常用类型和函数
pub use api::{sync_account_info, AccountSyncInfo};
pub use oauth::authenticate;
pub use types::GmailAccount;

// TokenManager 暂时不导出（阶段4使用）
#[allow(unused_imports)]
pub use token::TokenManager;
