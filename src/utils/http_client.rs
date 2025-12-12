/// 全局共享的 HTTP 客户端
///
/// 目的：
/// 1. 复用 TCP 连接和连接池（显著减少内存占用和网络开销）
/// 2. 避免每个 API 调用都创建新客户端（reqwest::Client 初始化成本高）
/// 3. 自动处理连接池管理和 Keep-Alive
///
/// reqwest 官方推荐：共享单个 Client 实例而不是为每个请求创建新实例
use once_cell::sync::Lazy;
use reqwest::Client;
use std::time::Duration;

/// 全局 HTTP 客户端实例（使用懒初始化）
pub static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        // 连接池配置
        .pool_max_idle_per_host(10) // 每个主机最多保留 10 个空闲连接
        .pool_idle_timeout(Duration::from_secs(300)) // 连接空闲 5 分钟后关闭
        // 超时配置
        .timeout(Duration::from_secs(30)) // 整体请求超时 30 秒
        .connect_timeout(Duration::from_secs(10)) // 连接建立超时 10 秒
        // 重定向配置
        .redirect(reqwest::redirect::Policy::limited(5)) // 最多跟随 5 个重定向
        // 用户代理
        .user_agent("NanoMail/0.1.0 (Windows; U; Rust) Gecko")
        .build()
        .expect("构建全局 HTTP 客户端失败")
});

/// 获取全局 HTTP 客户端
pub fn get_client() -> &'static Client {
    &HTTP_CLIENT
}
