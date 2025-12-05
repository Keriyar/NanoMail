/// 邮件同步引擎
///
/// 负责定期同步所有账户的邮件信息（未读数、头像等）
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::config::storage;
use crate::mail::gmail::{self, AccountSyncInfo};

/// 同步间隔（5分钟）
const SYNC_INTERVAL_SECS: u64 = 300;

/// 同步引擎
pub struct SyncEngine {
    /// 是否正在运行
    running: Arc<RwLock<bool>>,

    /// Tokio 运行时句柄
    rt_handle: tokio::runtime::Handle,
}

impl SyncEngine {
    /// 创建新的同步引擎
    ///
    /// # Arguments
    /// * `rt_handle` - Tokio 运行时句柄
    pub fn new(rt_handle: tokio::runtime::Handle) -> Self {
        Self {
            running: Arc::new(RwLock::new(false)),
            rt_handle,
        }
    }

    /// 启动同步引擎
    ///
    /// 会在后台线程中定期同步所有账户
    ///
    /// # Arguments
    /// * `sync_callback` - 同步完成后的回调函数，接收账户邮箱和同步信息
    pub fn start<F>(&self, sync_callback: F)
    where
        F: Fn(String, AccountSyncInfo) + Send + 'static,
    {
        let running = self.running.clone();
        let handle = self.rt_handle.clone();

        // 检查是否已经在运行
        if *running.blocking_read() {
            tracing::warn!("同步引擎已在运行");
            return;
        }

        // 标记为运行中
        *running.blocking_write() = true;

        tracing::info!("🚀 启动同步引擎（间隔: {} 秒）", SYNC_INTERVAL_SECS);

        // 在 Tokio 运行时内部以异步任务启动同步循环（避免跨线程 block_on 导致 runtime 在关闭时出错）
        handle.spawn(async move {
            let mut timer = interval(Duration::from_secs(SYNC_INTERVAL_SECS));

            // 首次同步延迟10秒（等待UI初始化）
            tracing::debug!("等待 10 秒后开始首次同步...");
            tokio::time::sleep(Duration::from_secs(10)).await;

            loop {
                // 检查运行标志
                if !*running.read().await {
                    tracing::info!("同步循环检测到停止标志，退出任务");
                    break;
                }

                timer.tick().await;

                tracing::info!("⏰ 开始定期同步...");

                // 加载所有账户
                let accounts = match storage::load_accounts() {
                    Ok(accounts) => accounts,
                    Err(e) => {
                        tracing::error!("加载账户失败: {}", e);
                        continue;
                    }
                };

                if accounts.is_empty() {
                    tracing::debug!("没有账户需要同步");
                    continue;
                }

                tracing::info!("正在同步 {} 个账户...", accounts.len());

                // 并行同步所有账户
                for account in accounts {
                    let email = account.email.clone();

                    match gmail::sync_account_info(&account).await {
                        Ok((sync_info, updated_account)) => {
                            tracing::info!(
                                "✅ {} - 未读 {} 封",
                                sync_info.email,
                                sync_info.unread_count
                            );

                            // 如果 Token 被刷新，保存更新后的账户
                            if let Some(updated) = updated_account {
                                if let Err(e) = storage::save_account(&updated) {
                                    tracing::error!("❌ 保存刷新后的账户失败: {}", e);
                                }
                            }

                            // 调用回调函数更新UI
                            sync_callback(email, sync_info);
                        }
                        Err(e) => {
                            tracing::error!("❌ 同步账户 {} 失败: {}", email, e);

                            // TODO: 如果是Token过期错误，尝试刷新Token
                        }
                    }
                }

                tracing::info!("✅ 本轮同步完成");
            }
        });
    }

    /// 立即执行一次同步
    ///
    /// 不等待定时器，立即同步所有账户
    ///
    /// # Arguments
    /// * `sync_callback` - 同步完成后的回调函数
    pub async fn sync_now<F>(&self, sync_callback: F) -> Result<()>
    where
        F: Fn(String, AccountSyncInfo) + Send,
    {
        tracing::info!("🔄 立即同步所有账户...");

        // 加载所有账户
        let accounts = storage::load_accounts()?;

        if accounts.is_empty() {
            tracing::info!("📭 没有账户需要同步");
            return Ok(());
        }

        tracing::info!("正在同步 {} 个账户...", accounts.len());

        // 并行同步所有账户
        for account in accounts {
            let email = account.email.clone();

            match gmail::sync_account_info(&account).await {
                Ok((sync_info, updated_account)) => {
                    tracing::info!(
                        "✅ {} - 未读 {} 封",
                        sync_info.email,
                        sync_info.unread_count
                    );

                    // 如果 Token 被刷新，保存更新后的账户
                    if let Some(updated) = updated_account {
                        if let Err(e) = storage::save_account(&updated) {
                            tracing::error!("❌ 保存刷新后的账户失败: {}", e);
                        }
                    }

                    // 调用回调函数更新UI
                    sync_callback(email, sync_info);
                }
                Err(e) => {
                    tracing::error!("❌ 同步账户 {} 失败: {}", email, e);
                }
            }
        }

        tracing::info!("✅ 立即同步完成");

        Ok(())
    }

    /// 停止同步引擎
    pub async fn stop(&self) {
        *self.running.write().await = false;
        tracing::info!("🛑 同步引擎已停止");
    }

    /// 同步请求停止（同步接口，适用于在非 async 环境调用）
    pub fn request_stop(&self) {
        *self.running.blocking_write() = false;
        tracing::info!("🛑 已请求停止同步引擎（同步接口）");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_engine_creation() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = SyncEngine::new(rt.handle().clone());

        assert!(!*engine.running.blocking_read());
    }

    #[test]
    fn test_sync_interval() {
        assert_eq!(SYNC_INTERVAL_SECS, 300); // 5分钟
    }
}
