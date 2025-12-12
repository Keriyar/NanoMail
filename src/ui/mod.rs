// UI 模块 - Rust-Slint 数据桥接

use slint::{Image, SharedString};

/// 编译时嵌入占位头像（避免运行时依赖外部文件）
const PLACEHOLDER_AVATAR_BYTES: &[u8] = include_bytes!("../../assets/icons/placeholder-avatar.svg");

/// Account 结构体（对应 Slint 的 Account struct）
#[derive(Clone, Debug)]
pub struct Account {
    pub email: String,
    pub display_name: String,
    pub avatar_url: String,
    pub unread_count: i32,
    pub is_loading: bool,
    pub has_error: bool,
}

impl Account {
    /// 创建测试账户
    pub fn mock() -> Self {
        Self {
            email: "crayonape@gmail.com".to_string(),
            display_name: "Crayon Ape".to_string(),
            avatar_url: String::new(), // 空字符串 = 使用默认头像
            unread_count: 22,
            is_loading: false,
            has_error: false,
        }
    }

    /// 创建多个测试账户
    pub fn mock_multiple(count: usize) -> Vec<Self> {
        (0..count)
            .map(|i| Self {
                email: format!("user{}@gmail.com", i + 1),
                display_name: format!("Test User {}", i + 1),
                avatar_url: String::new(),
                unread_count: ((i + 1) * 10) as i32,
                is_loading: false,
                has_error: i % 3 == 0, // 每3个账户有一个错误状态
            })
            .collect()
    }
}

/// 加载占位头像（从嵌入的资源）
fn load_placeholder_avatar() -> Image {
    match Image::load_from_svg_data(PLACEHOLDER_AVATAR_BYTES) {
        Ok(img) => img,
        Err(e) => {
            tracing::warn!("加载嵌入的占位头像失败: {}", e);
            Image::default()
        }
    }
}

/// 将 Rust Account 转换为 Slint Account
impl From<Account> for crate::Account {
    fn from(account: Account) -> Self {
        // 尝试将本地路径转换为 Slint Image；失败时使用嵌入的占位图
        let avatar_image: Image = if account.avatar_url.is_empty() {
            load_placeholder_avatar()
        } else {
            match Image::load_from_path(std::path::Path::new(&account.avatar_url)) {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("加载头像失败 [{}]: {}", account.avatar_url, e);
                    load_placeholder_avatar()
                }
            }
        };

        Self {
            email: SharedString::from(account.email),
            display_name: SharedString::from(account.display_name),
            avatar_image,
            unread_count: account.unread_count,
            is_loading: account.is_loading,
            has_error: account.has_error,
        }
    }
}
