/// Gmail OAuth2 认证流程
///
/// 实现完整的 OAuth2 授权码流程（带 PKCE）
use anyhow::{Context, Result};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use std::time::Duration;
use tiny_http::{Header, Response, Server};
use tokio::sync::oneshot;
use url::Url;

use crate::config::{oauth_config::OAuthConfig, storage};
use crate::mail::gmail::types::GmailAccount;

/// OAuth2 回调超时时间（秒）
const CALLBACK_TIMEOUT_SECS: u64 = 60;

/// 本地服务器端口范围
const PORT_RANGE: std::ops::Range<u16> = 8080..8090;

/// OAuth2 成功页面 HTML
const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权成功 - NanoMail</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            background: white;
            padding: 40px;
            border-radius: 12px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.2);
            text-align: center;
            max-width: 400px;
        }
        h1 {
            color: #667eea;
            margin-bottom: 20px;
        }
        p {
            color: #666;
            line-height: 1.6;
        }
        .checkmark {
            font-size: 64px;
            color: #4caf50;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="checkmark">✓</div>
        <h1>授权成功</h1>
        <p>您的 Gmail 账户已成功连接到 NanoMail。</p>
        <p>现在可以关闭此页面并返回应用程序。</p>
    </div>
</body>
</html>"#;

/// OAuth2 错误页面 HTML
const ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>授权失败 - NanoMail</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
        }
        .container {
            background: white;
            padding: 40px;
            border-radius: 12px;
            box-shadow: 0 10px 40px rgba(0,0,0,0.2);
            text-align: center;
            max-width: 400px;
        }
        h1 {
            color: #f5576c;
            margin-bottom: 20px;
        }
        p {
            color: #666;
            line-height: 1.6;
        }
        .cross {
            font-size: 64px;
            color: #f44336;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="cross">✗</div>
        <h1>授权失败</h1>
        <p>Gmail 账户连接失败，请稍后重试。</p>
        <p>如果问题持续，请检查网络连接或联系支持。</p>
    </div>
</body>
</html>"#;

/// 执行 Gmail OAuth2 认证
///
/// 完整的八步流程：
/// 1. 生成授权 URL
/// 2. 启动本地服务器
/// 3. 打开浏览器
/// 4. 等待回调
/// 5. 验证 CSRF state
/// 6. 交换 Token
/// 7. 获取用户信息
/// 8. 加密保存
///
/// # Returns
/// 返回已保存的 Gmail 账户信息
///
/// # Errors
/// - OAuth2 配置无效（占位符）
/// - 无法启动本地服务器（端口被占用）
/// - 浏览器打开失败
/// - 用户拒绝授权
/// - Token 交换失败
/// - 网络错误
pub async fn authenticate() -> Result<GmailAccount> {
    tracing::info!("🔐 开始 Gmail OAuth2 认证流程");

    // 步骤 1：加载配置
    let config = OAuthConfig::load()?;

    // 验证配置
    if config.is_placeholder() {
        anyhow::bail!(
            "OAuth2 配置无效：请设置环境变量或创建配置文件\n\
             参考：docs/setup_oauth.md"
        );
    }

    // 步骤 2：生成授权 URL
    let (auth_url, csrf_state, pkce_verifier, port) = build_auth_url(&config)?;
    tracing::info!("✅ 授权 URL 生成成功");
    tracing::debug!("授权 URL: {}", auth_url);

    // 步骤 3：启动本地服务器
    let (code_tx, code_rx) = oneshot::channel();
    let server_handle = std::thread::spawn(move || start_local_server(port, code_tx));
    tracing::info!("✅ 本地服务器启动成功: http://localhost:{}", port);

    // 步骤 4：打开浏览器
    webbrowser::open(auth_url.as_str()).context("无法打开浏览器，请手动复制以下 URL：")?;
    tracing::info!("✅ 浏览器已打开，等待用户授权...");

    // 步骤 5：等待回调（带超时）
    let (received_code, received_state) =
        tokio::time::timeout(Duration::from_secs(CALLBACK_TIMEOUT_SECS), code_rx)
            .await
            .context("授权超时：用户未在规定时间内完成授权")?
            .context("本地服务器接收回调失败")?;

    tracing::info!("✅ 收到授权回调");

    // 等待服务器线程结束
    server_handle
        .join()
        .map_err(|_| anyhow::anyhow!("服务器线程 panic"))?
        .context("服务器关闭时出错")?;

    // 步骤 6：验证 CSRF state
    if received_state.secret() != csrf_state.secret() {
        anyhow::bail!(
            "CSRF 验证失败：state 不匹配\n期望: {}...\n实际: {}...",
            &csrf_state.secret()[..8],
            &received_state.secret()[..8]
        );
    }
    tracing::info!("✅ CSRF 验证通过");

    // 步骤 7：交换 Token
    tracing::debug!("开始交换 Token，使用 redirect_uri: {}", config.redirect_uri);
    let token_response = exchange_code_for_token(received_code, pkce_verifier, &config, port)
        .await
        .context("Token 交换失败")?;

    let access_token = token_response.access_token().secret().to_string();
    let refresh_token = token_response
        .refresh_token()
        .ok_or_else(|| anyhow::anyhow!("未收到 refresh_token"))?
        .secret()
        .to_string();

    let expires_in = token_response
        .expires_in()
        .unwrap_or(Duration::from_secs(3600))
        .as_secs() as i64;

    tracing::info!("✅ Token 交换成功");
    tracing::debug!(
        "Access Token: {}...{} (有效期: {} 秒)",
        &access_token[..5],
        &access_token[access_token.len() - 5..],
        expires_in
    );

    // 步骤 8：获取用户信息
    let (email, display_name) = fetch_user_info(&access_token)
        .await
        .context("获取用户信息失败")?;

    tracing::info!("✅ 用户信息获取成功: {}", email);

    // 步骤 9：创建账户（Token 在创建时自动加密）
    let account = GmailAccount::new(email, display_name, access_token, refresh_token, expires_in)
        .context("创建账户失败")?;

    storage::save_account(&account).context("保存账户失败")?;

    tracing::info!("✅ 账户已保存（Token 已加密）");
    tracing::info!("🎉 OAuth2 认证流程完成");

    Ok(account)
}

/// 生成授权 URL
///
/// 使用 PKCE (RFC 7636) 提升安全性
fn build_auth_url(config: &OAuthConfig) -> Result<(Url, CsrfToken, PkceCodeVerifier, u16)> {
    // 尝试端口范围
    let mut last_error = None;
    for port in PORT_RANGE {
        match try_build_auth_url(config, port) {
            Ok(result) => return Ok(result),
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("所有端口均被占用")))
}

fn try_build_auth_url(
    config: &OAuthConfig,
    port: u16,
) -> Result<(Url, CsrfToken, PkceCodeVerifier, u16)> {
    // 构建 OAuth2 客户端
    let client = BasicClient::new(
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    )
    .set_redirect_uri(RedirectUrl::new(format!("http://localhost:{}", port))?);

    // 生成 PKCE 挑战
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // 生成授权 URL
    let (auth_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(config.scopes.iter().map(|s| Scope::new(s.clone())))
        .set_pkce_challenge(pkce_challenge)
        .url();

    Ok((auth_url, csrf_state, pkce_verifier, port))
}

/// 启动本地 HTTP 服务器接收 OAuth2 回调
fn start_local_server(
    port: u16,
    code_tx: oneshot::Sender<(AuthorizationCode, CsrfToken)>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("无法启动本地服务器（端口可能被占用）: {}", e))?;

    tracing::debug!("本地服务器监听: {}", addr);

    for request in server.incoming_requests() {
        let url_str = format!("http://localhost:{}{}", port, request.url());
        tracing::debug!("收到请求: {}", url_str);

        let parsed_url = Url::parse(&url_str)?;

        // 解析 query 参数
        let params: std::collections::HashMap<_, _> =
            parsed_url.query_pairs().into_owned().collect();

        // 检查是否有错误
        if let Some(error) = params.get("error") {
            tracing::error!("用户拒绝授权: {}", error);

            // 返回错误页面
            let response = Response::from_string(ERROR_HTML).with_header(
                Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap(),
            );
            request.respond(response)?;

            return Err(anyhow::anyhow!("用户拒绝授权: {}", error));
        }

        // 提取 code 和 state
        let code = params
            .get("code")
            .ok_or_else(|| anyhow::anyhow!("回调缺少 code 参数"))?;

        let state = params
            .get("state")
            .ok_or_else(|| anyhow::anyhow!("回调缺少 state 参数"))?;

        tracing::debug!("Code: {}...", &code[..10]);
        tracing::debug!("State: {}...", &state[..10]);

        // 返回成功页面
        let response = Response::from_string(SUCCESS_HTML)
            .with_header(Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap());
        request.respond(response)?;

        // 发送结果
        code_tx
            .send((
                AuthorizationCode::new(code.clone()),
                CsrfToken::new(state.clone()),
            ))
            .ok();

        break;
    }

    Ok(())
}

/// 交换授权码为 Token
async fn exchange_code_for_token(
    code: AuthorizationCode,
    verifier: PkceCodeVerifier,
    config: &OAuthConfig,
    port: u16,
) -> Result<
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
> {
    // 使用实际的 redirect_uri（带端口号）
    let actual_redirect_uri = format!("http://localhost:{}", port);

    let client = BasicClient::new(
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    )
    .set_redirect_uri(RedirectUrl::new(actual_redirect_uri.clone())?);

    tracing::debug!("交换 Token：client_id={}...", &config.client_id[..20]);
    tracing::debug!("交换 Token：client_id={}...", &config.client_id[..20]);

    // 为了支持重试（不带 client_secret 的 PKCE-only），先把 code/verifier 的字符串保存下来，
    // 每次重试都重新构造对应对象（AuthorizationCode/ PkceCodeVerifier）
    let code_secret = code.secret().to_string();
    let verifier_secret = verifier.secret().to_string();

    // 首次尝试（带 client_secret）
    let first_code = AuthorizationCode::new(code_secret.clone());
    let first_verifier = PkceCodeVerifier::new(verifier_secret.clone());

    match client
        .exchange_code(first_code)
        .set_pkce_verifier(first_verifier)
        .request_async(oauth2::reqwest::async_http_client)
        .await
    {
        Ok(tok) => return Ok(tok),
        Err(e) => {
            tracing::error!("Token 交换详细错误: {:?}", e);

            let err_str = format!("{:?}", e);

            // 如果是 invalid_client/Unauthorized，尝试不带 client_secret 的 PKCE-only 重试（适配部分 native 客户端配置）
            if err_str.contains("invalid_client") || err_str.contains("Unauthorized") {
                tracing::warn!(
                    "首次交换返回 invalid_client/Unauthorized，尝试使用不带 client_secret 的公共客户端重试（PKCE-only）"
                );

                let client_public = BasicClient::new(
                    ClientId::new(config.client_id.clone()),
                    None,
                    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
                    Some(TokenUrl::new(
                        "https://oauth2.googleapis.com/token".to_string(),
                    )?),
                )
                .set_redirect_uri(RedirectUrl::new(actual_redirect_uri.clone())?);

                let retry_code = AuthorizationCode::new(code_secret);
                let retry_verifier = PkceCodeVerifier::new(verifier_secret);

                match client_public
                    .exchange_code(retry_code)
                    .set_pkce_verifier(retry_verifier)
                    .request_async(oauth2::reqwest::async_http_client)
                    .await
                {
                    Ok(tok2) => return Ok(tok2),
                    Err(e2) => {
                        tracing::error!("使用 PKCE-only 重试仍失败: {:?}", e2);
                        return Err(anyhow::anyhow!("Token 交换失败: {}", e2));
                    }
                }
            }

            return Err(anyhow::anyhow!("Token 交换请求失败: {}", e));
        }
    }
}

/// 获取用户信息
///
/// 调用 Gmail API 获取邮箱地址
async fn fetch_user_info(access_token: &str) -> Result<(String, String)> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://gmail.googleapis.com/gmail/v1/users/me/profile")
        .bearer_auth(access_token)
        .send()
        .await
        .context("请求用户信息失败")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Gmail API 返回错误: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        );
    }

    let json: serde_json::Value = response.json().await.context("解析响应 JSON 失败")?;

    let email = json["emailAddress"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("响应中缺少 emailAddress 字段"))?
        .to_string();

    // Gmail API 不返回 display name，使用邮箱前缀
    let display_name = email.split('@').next().unwrap_or(&email).to_string();

    Ok((email, display_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_range() {
        assert!(PORT_RANGE.contains(&8080));
        assert!(PORT_RANGE.contains(&8089));
        assert!(!PORT_RANGE.contains(&8090));
    }

    #[test]
    fn test_html_contains_charset() {
        assert!(SUCCESS_HTML.contains("utf-8"));
        assert!(ERROR_HTML.contains("utf-8"));
    }
}
