use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use reqwest::Url;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

use crate::service::config::app_language::get_app_language_code;
use crate::service::i18n::LocaleId;
use crate::service::mcp::auth::{
    clear_stored_oauth_credentials, map_auth_error, prepare_remote_oauth_authorization,
    MCPRemoteOAuthSessionSnapshot, MCPRemoteOAuthStatus,
};
use crate::service::mcp::server::MCPServerType;
use crate::util::errors::{BitFunError, BitFunResult};

use super::{ActiveRemoteOAuthSession, MCPServerManager};

const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct OAuthCallbackPayload {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Clone, Copy)]
enum OAuthCallbackLocale {
    ZhCN,
    ZhTW,
    EnUS,
}

struct OAuthCallbackPageCopy {
    html_lang: &'static str,
    page_title: &'static str,
    brand_label: &'static str,
    badge_success: &'static str,
    badge_warning: &'static str,
    badge_error: &'static str,
    success_title: &'static str,
    success_message: &'static str,
    success_detail_title: &'static str,
    success_detail_body: &'static str,
    warning_title: &'static str,
    warning_message: &'static str,
    warning_detail_title: &'static str,
    error_title: &'static str,
    error_message: &'static str,
    error_detail_title: &'static str,
    close_hint: &'static str,
}

impl OAuthCallbackLocale {
    fn from_language_code(value: &str) -> Option<Self> {
        match LocaleId::from_str(value)? {
            LocaleId::ZhCN => Some(Self::ZhCN),
            LocaleId::ZhTW => Some(Self::ZhTW),
            LocaleId::EnUS => Some(Self::EnUS),
        }
    }

    fn from_accept_language(value: &str) -> Self {
        value
            .split(',')
            .filter_map(|part| part.split(';').next())
            .find_map(|part| Self::from_language_code(part.trim()))
            .unwrap_or(Self::ZhCN)
    }

    fn copy(self) -> OAuthCallbackPageCopy {
        match self {
            Self::ZhCN => OAuthCallbackPageCopy {
                html_lang: "zh-CN",
                page_title: "BitFun OAuth 回调",
                brand_label: "BitFun Desktop",
                badge_success: "已收到授权",
                badge_warning: "回调参数不完整",
                badge_error: "授权失败",
                success_title: "BitFun 已收到 OAuth 回调",
                success_message: "可以返回 BitFun。应用正在交换授权码并重新连接 MCP 服务器。",
                success_detail_title: "接下来会发生什么",
                success_detail_body:
                    "这个页面可以直接关闭。如果 BitFun 没有自动完成重连，请回到 MCP 设置页后重试 OAuth。",
                warning_title: "BitFun 收到的 OAuth 回调缺少必要参数",
                warning_message:
                    "OAuth 提供方已跳转回来，但缺少必须的参数。请返回 BitFun 重新发起登录流程。",
                warning_detail_title: "缺少的参数",
                error_title: "BitFun 未能完成 OAuth 授权",
                error_message:
                    "请返回 BitFun，并根据下面的提供方返回信息检查问题后重新发起 OAuth。",
                error_detail_title: "提供方返回",
                close_hint: "处理完成后，这个页面可以直接关闭。",
            },
            Self::ZhTW => OAuthCallbackPageCopy {
                html_lang: "zh-TW",
                page_title: "BitFun OAuth 回調",
                brand_label: "BitFun Desktop",
                badge_success: "已收到授權",
                badge_warning: "回調參數不完整",
                badge_error: "授權失敗",
                success_title: "BitFun 已收到 OAuth 回調",
                success_message: "可以返回 BitFun。應用正在交換授權碼並重新連接 MCP 服務器。",
                success_detail_title: "接下來會發生什麼",
                success_detail_body:
                    "這個頁面可以直接關閉。如果 BitFun 沒有自動完成重連，請回到 MCP 設置頁後重試 OAuth。",
                warning_title: "BitFun 收到的 OAuth 回調缺少必要參數",
                warning_message:
                    "OAuth 提供方已跳轉回來，但缺少必須的參數。請返回 BitFun 重新發起登錄流程。",
                warning_detail_title: "缺少的參數",
                error_title: "BitFun 未能完成 OAuth 授權",
                error_message:
                    "請返回 BitFun，並根據下面的提供方返回信息檢查問題後重新發起 OAuth。",
                error_detail_title: "提供方返回",
                close_hint: "處理完成後，這個頁面可以直接關閉。",
            },
            Self::EnUS => OAuthCallbackPageCopy {
                html_lang: "en-US",
                page_title: "BitFun OAuth Callback",
                brand_label: "BitFun Desktop",
                badge_success: "Authorization received",
                badge_warning: "Callback incomplete",
                badge_error: "Authorization failed",
                success_title: "BitFun received the OAuth callback",
                success_message:
                    "You can return to BitFun now. The app is exchanging the authorization code and reconnecting the MCP server.",
                success_detail_title: "What happens next",
                success_detail_body:
                    "This page can be closed now. If BitFun does not finish reconnecting automatically, return to MCP settings and retry OAuth.",
                warning_title: "BitFun received an OAuth callback with missing parameters",
                warning_message:
                    "The provider redirected back, but required OAuth parameters were missing. Return to BitFun and start the sign-in flow again.",
                warning_detail_title: "Missing parameters",
                error_title: "BitFun could not finish the OAuth authorization",
                error_message:
                    "Return to BitFun and review the provider response below before retrying OAuth.",
                error_detail_title: "Provider response",
                close_hint: "This page can be closed after you review the status.",
            },
        }
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn resolve_oauth_callback_locale(
    preferred_language: Option<&str>,
    accept_language: Option<&str>,
) -> OAuthCallbackLocale {
    preferred_language
        .and_then(OAuthCallbackLocale::from_language_code)
        .or_else(|| accept_language.map(OAuthCallbackLocale::from_accept_language))
        .unwrap_or(OAuthCallbackLocale::ZhCN)
}

fn render_oauth_callback_page(
    payload: &OAuthCallbackPayload,
    locale: OAuthCallbackLocale,
) -> String {
    let copy = locale.copy();
    let (badge, badge_class, title, message, detail_title, detail_body, icon_label) =
        if let Some(error) = payload.error.as_deref() {
            let description = payload
                .error_description
                .as_deref()
                .unwrap_or(match locale {
                    OAuthCallbackLocale::ZhCN => "OAuth 提供方拒绝了这次授权请求。",
                    OAuthCallbackLocale::ZhTW => "OAuth 提供方拒絕了這次授權請求。",
                    OAuthCallbackLocale::EnUS => "The provider rejected the authorization request.",
                });
            (
                copy.badge_error,
                "is-error",
                copy.error_title,
                copy.error_message,
                copy.error_detail_title,
                format!("{}: {}", escape_html(error), escape_html(description)),
                "!",
            )
        } else if payload.code.is_some() && payload.state.is_some() {
            (
                copy.badge_success,
                "is-success",
                copy.success_title,
                copy.success_message,
                copy.success_detail_title,
                copy.success_detail_body.to_string(),
                match locale {
                    OAuthCallbackLocale::ZhCN => "完成",
                    OAuthCallbackLocale::ZhTW => "完成",
                    OAuthCallbackLocale::EnUS => "Done",
                },
            )
        } else {
            let mut missing = Vec::new();
            if payload.code.is_none() {
                missing.push("code");
            }
            if payload.state.is_none() {
                missing.push("state");
            }
            (
                copy.badge_warning,
                "is-warning",
                copy.warning_title,
                copy.warning_message,
                copy.warning_detail_title,
                escape_html(&missing.join(", ")),
                "?",
            )
        };

    format!(
        r#"<!DOCTYPE html>
<html lang="{html_lang}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{page_title}</title>
    <style>
      :root {{
        color-scheme: light;
        --bg-0: #f3efe5;
        --bg-1: #dbe7ff;
        --bg-2: #f8c98b;
        --panel: rgba(255, 252, 246, 0.88);
        --panel-border: rgba(53, 66, 97, 0.14);
        --text-strong: #172033;
        --text-muted: #5c6474;
        --shadow: 0 24px 80px rgba(23, 32, 51, 0.16);
        --success: #176b52;
        --success-soft: rgba(23, 107, 82, 0.12);
        --warning: #9a5a00;
        --warning-soft: rgba(154, 90, 0, 0.14);
        --error: #a63232;
        --error-soft: rgba(166, 50, 50, 0.12);
      }}

      * {{
        box-sizing: border-box;
      }}

      body {{
        margin: 0;
        min-height: 100vh;
        font-family: "Segoe UI Variable Display", "Aptos", "Trebuchet MS", sans-serif;
        color: var(--text-strong);
        background:
          radial-gradient(circle at top left, rgba(255, 255, 255, 0.72), transparent 34%),
          radial-gradient(circle at bottom right, rgba(255, 230, 202, 0.9), transparent 30%),
          linear-gradient(135deg, var(--bg-0) 0%, var(--bg-1) 52%, var(--bg-2) 100%);
        overflow: hidden;
      }}

      .orb {{
        position: fixed;
        border-radius: 999px;
        filter: blur(12px);
        opacity: 0.56;
        pointer-events: none;
      }}

      .orb-a {{
        width: 320px;
        height: 320px;
        top: -96px;
        right: -48px;
        background: rgba(126, 159, 255, 0.34);
      }}

      .orb-b {{
        width: 260px;
        height: 260px;
        bottom: -84px;
        left: -40px;
        background: rgba(255, 193, 118, 0.38);
      }}

      .shell {{
        position: relative;
        min-height: 100vh;
        display: grid;
        place-items: center;
        padding: 28px;
      }}

      .panel {{
        width: min(100%, 720px);
        padding: 32px;
        border: 1px solid var(--panel-border);
        border-radius: 28px;
        background: var(--panel);
        backdrop-filter: blur(18px);
        box-shadow: var(--shadow);
      }}

      .brand {{
        display: flex;
        align-items: center;
        gap: 16px;
        margin-bottom: 24px;
      }}

      .brand-mark {{
        width: 52px;
        height: 52px;
        border-radius: 16px;
        display: grid;
        place-items: center;
        font-weight: 700;
        letter-spacing: 0.08em;
        color: #fffaf0;
        background: linear-gradient(135deg, #172033 0%, #335c95 100%);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.16);
      }}

      .eyebrow {{
        display: block;
        margin-bottom: 4px;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.16em;
        text-transform: uppercase;
        color: var(--text-muted);
      }}

      h1 {{
        margin: 0;
        font-size: clamp(28px, 5vw, 44px);
        line-height: 1.04;
        letter-spacing: -0.04em;
      }}

      .badge {{
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 8px 14px;
        border-radius: 999px;
        font-size: 13px;
        font-weight: 700;
      }}

      .badge.is-success {{
        color: var(--success);
        background: var(--success-soft);
      }}

      .badge.is-warning {{
        color: var(--warning);
        background: var(--warning-soft);
      }}

      .badge.is-error {{
        color: var(--error);
        background: var(--error-soft);
      }}

      .content {{
        display: grid;
        gap: 20px;
      }}

      .content > * {{
        min-width: 0;
      }}

      .lead {{
        margin: 0;
        max-width: 58ch;
        font-size: 17px;
        line-height: 1.7;
        color: var(--text-muted);
      }}

      .status-card {{
        display: grid;
        grid-template-columns: auto 1fr;
        gap: 18px;
        padding: 20px;
        border-radius: 22px;
        background: rgba(255, 255, 255, 0.58);
        border: 1px solid rgba(53, 66, 97, 0.08);
      }}

      .status-icon {{
        width: 52px;
        height: 52px;
        border-radius: 18px;
        display: grid;
        place-items: center;
        font-weight: 800;
        font-size: 16px;
        color: var(--text-strong);
        background: linear-gradient(135deg, rgba(255, 255, 255, 0.94), rgba(227, 235, 255, 0.92));
        border: 1px solid rgba(53, 66, 97, 0.08);
      }}

      .status-title {{
        margin: 0 0 8px;
        font-size: 15px;
        font-weight: 700;
        letter-spacing: 0.01em;
      }}

      .status-body {{
        margin: 0;
        font-family: "Cascadia Code", "Consolas", monospace;
        font-size: 13px;
        line-height: 1.7;
        color: var(--text-muted);
        word-break: break-word;
      }}

      .close-note {{
        padding: 16px 18px;
        border-radius: 18px;
        background: rgba(23, 32, 51, 0.06);
        border: 1px solid rgba(53, 66, 97, 0.08);
      }}

      .footnote {{
        margin: 0;
        font-size: 13px;
        line-height: 1.7;
        color: var(--text-muted);
      }}

      @media (max-width: 640px) {{
        .panel {{
          padding: 24px;
          border-radius: 24px;
        }}

        .brand,
        .status-card {{
          grid-template-columns: 1fr;
        }}

        .status-card {{
          gap: 14px;
        }}
      }}
    </style>
  </head>
  <body>
    <div class="orb orb-a"></div>
    <div class="orb orb-b"></div>
    <main class="shell">
      <section class="panel">
        <div class="brand">
          <div class="brand-mark">BF</div>
          <div>
            <span class="eyebrow">{brand_label}</span>
            <h1>{title}</h1>
          </div>
        </div>
        <div class="content">
          <div class="badge {badge_class}">{badge}</div>
          <p class="lead">{message}</p>
          <div class="status-card">
            <div class="status-icon">{icon_label}</div>
            <div>
              <p class="status-title">{detail_title}</p>
              <p class="status-body">{detail_body}</p>
            </div>
          </div>
          <div class="close-note">
            <p class="footnote">{close_hint}</p>
          </div>
        </div>
      </section>
    </main>
  </body>
</html>"#,
        html_lang = copy.html_lang,
        page_title = copy.page_title,
        brand_label = copy.brand_label,
        title = title,
        badge = badge,
        badge_class = badge_class,
        message = message,
        detail_title = detail_title,
        detail_body = detail_body,
        icon_label = icon_label,
        close_hint = copy.close_hint,
    )
}

#[derive(Clone)]
struct OAuthCallbackAppState {
    callback_tx: Arc<Mutex<Option<oneshot::Sender<OAuthCallbackPayload>>>>,
    preferred_language: String,
}

impl MCPServerManager {
    pub(super) async fn set_oauth_snapshot(
        session: &Arc<ActiveRemoteOAuthSession>,
        snapshot: MCPRemoteOAuthSessionSnapshot,
    ) {
        *session.snapshot.write().await = snapshot;
    }

    pub(super) async fn update_oauth_snapshot<F>(
        session: &Arc<ActiveRemoteOAuthSession>,
        update: F,
    ) -> MCPRemoteOAuthSessionSnapshot
    where
        F: FnOnce(&mut MCPRemoteOAuthSessionSnapshot),
    {
        let mut snapshot = session.snapshot.write().await;
        update(&mut snapshot);
        snapshot.clone()
    }

    pub(super) async fn insert_oauth_session(
        &self,
        server_id: &str,
        session: Arc<ActiveRemoteOAuthSession>,
    ) -> Option<Arc<ActiveRemoteOAuthSession>> {
        self.oauth_sessions
            .write()
            .await
            .insert(server_id.to_string(), session)
    }

    pub(super) async fn shutdown_oauth_session(session: &Arc<ActiveRemoteOAuthSession>) {
        if let Some(shutdown_tx) = session.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }
    }

    async fn fail_oauth_session(
        session: &Arc<ActiveRemoteOAuthSession>,
        message: String,
    ) -> MCPRemoteOAuthSessionSnapshot {
        let snapshot = MCPServerManager::update_oauth_snapshot(session, |snapshot| {
            snapshot.status = MCPRemoteOAuthStatus::Failed;
            snapshot.message = Some(message);
        })
        .await;
        Self::shutdown_oauth_session(session).await;
        snapshot
    }

    pub async fn start_remote_oauth_authorization(
        &self,
        server_id: &str,
    ) -> BitFunResult<MCPRemoteOAuthSessionSnapshot> {
        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        if config.server_type != MCPServerType::Remote {
            return Err(BitFunError::Validation(format!(
                "MCP server '{}' is not a remote server",
                server_id
            )));
        }

        if let Some(existing) = self.oauth_sessions.write().await.remove(server_id) {
            Self::shutdown_oauth_session(&existing).await;
        }

        let prepared = prepare_remote_oauth_authorization(&config).await?;
        let callback_path = Url::parse(&prepared.redirect_uri)
            .map_err(|error| {
                BitFunError::MCPError(format!(
                    "Invalid OAuth redirect URI for server '{}': {}",
                    server_id, error
                ))
            })?
            .path()
            .to_string();

        let initial_snapshot = MCPRemoteOAuthSessionSnapshot::new(
            server_id.to_string(),
            MCPRemoteOAuthStatus::AwaitingBrowser,
            Some(prepared.authorization_url.clone()),
            Some(prepared.redirect_uri.clone()),
            Some("Open the authorization URL to continue OAuth sign-in.".to_string()),
        );

        let (callback_tx, callback_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let session = Arc::new(ActiveRemoteOAuthSession {
            snapshot: Arc::new(tokio::sync::RwLock::new(initial_snapshot.clone())),
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        });

        if let Some(previous) = self.insert_oauth_session(server_id, session.clone()).await {
            Self::shutdown_oauth_session(&previous).await;
        }

        let callback_state = OAuthCallbackAppState {
            callback_tx: Arc::new(Mutex::new(Some(callback_tx))),
            preferred_language: get_app_language_code().await,
        };
        let router = Router::new()
            .route(&callback_path, get(handle_oauth_callback))
            .with_state(callback_state);
        let callback_server_session = session.clone();
        let callback_server_id = server_id.to_string();
        tokio::spawn(async move {
            let server =
                axum::serve(prepared.listener, router).with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                });

            if let Err(error) = server.await {
                let _ =
                    MCPServerManager::update_oauth_snapshot(&callback_server_session, |snapshot| {
                        if matches!(
                            snapshot.status,
                            MCPRemoteOAuthStatus::Authorized | MCPRemoteOAuthStatus::Cancelled
                        ) {
                            return;
                        }
                        snapshot.status = MCPRemoteOAuthStatus::Failed;
                        snapshot.message = Some(format!(
                            "OAuth callback listener failed for server '{}': {}",
                            callback_server_id, error
                        ));
                    })
                    .await;
            }
        });

        let manager = self.clone();
        let callback_session = session.clone();
        let callback_server_id = server_id.to_string();
        let authorization_url = prepared.authorization_url.clone();
        let redirect_uri = prepared.redirect_uri.clone();
        let mut oauth_state = prepared.state;
        tokio::spawn(async move {
            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::AwaitingCallback;
                snapshot.message =
                    Some("Waiting for the OAuth provider to redirect back to BitFun.".to_string());
            })
            .await;

            let callback = match timeout(OAUTH_CALLBACK_TIMEOUT, callback_rx).await {
                Ok(Ok(callback)) => callback,
                Ok(Err(_)) => {
                    let _ =
                        MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                            snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                            snapshot.message =
                                Some("OAuth authorization was cancelled.".to_string());
                        })
                        .await;
                    Self::shutdown_oauth_session(&callback_session).await;
                    return;
                }
                Err(_) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth authorization timed out before the provider redirected back."
                            .to_string(),
                    )
                    .await;
                    return;
                }
            };

            if let Some(error) = callback.error {
                let description = callback
                    .error_description
                    .map(|value| format!(": {}", value))
                    .unwrap_or_default();
                let _ = MCPServerManager::fail_oauth_session(
                    &callback_session,
                    format!("OAuth provider returned '{}{}'", error, description),
                )
                .await;
                return;
            }

            let code = match callback.code {
                Some(code) => code,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include an authorization code.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let state = match callback.state {
                Some(state) => state,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include a state token.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::ExchangingToken;
                snapshot.message =
                    Some("Exchanging the authorization code for an access token.".to_string());
            })
            .await;

            match oauth_state.handle_callback(&code, &state).await {
                Ok(_) => {
                    let _ = MCPServerManager::set_oauth_snapshot(
                        &callback_session,
                        MCPRemoteOAuthSessionSnapshot::new(
                            callback_server_id.clone(),
                            MCPRemoteOAuthStatus::Authorized,
                            Some(authorization_url.clone()),
                            Some(redirect_uri.clone()),
                            Some(
                                "OAuth authorization completed. Reconnecting MCP server."
                                    .to_string(),
                            ),
                        ),
                    )
                    .await;

                    if let Some(shutdown_tx) = callback_session.shutdown_tx.lock().await.take() {
                        let _ = shutdown_tx.send(());
                    }

                    manager.clear_reconnect_state(&callback_server_id).await;
                    let _ = manager.stop_server(&callback_server_id).await;
                    if let Err(error) = manager.start_server(&callback_server_id).await {
                        let _ = MCPServerManager::update_oauth_snapshot(
                            &callback_session,
                            |snapshot| {
                                snapshot.message = Some(format!(
                                    "OAuth token saved, but reconnect failed: {}",
                                    error
                                ));
                            },
                        )
                        .await;
                    }
                }
                Err(error) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        map_auth_error(error).to_string(),
                    )
                    .await;
                }
            }
        });

        Ok(initial_snapshot)
    }

    pub async fn get_remote_oauth_session(
        &self,
        server_id: &str,
    ) -> Option<MCPRemoteOAuthSessionSnapshot> {
        let session = self.oauth_sessions.read().await.get(server_id).cloned()?;
        let snapshot = session.snapshot.read().await.clone();
        Some(snapshot)
    }

    pub async fn cancel_remote_oauth_authorization(&self, server_id: &str) -> BitFunResult<()> {
        let session = self.oauth_sessions.write().await.remove(server_id);
        if let Some(session) = session {
            let _ = MCPServerManager::update_oauth_snapshot(&session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                snapshot.message = Some("OAuth authorization was cancelled.".to_string());
            })
            .await;
            Self::shutdown_oauth_session(&session).await;
        }
        Ok(())
    }

    pub async fn clear_remote_oauth_credentials(&self, server_id: &str) -> BitFunResult<()> {
        self.cancel_remote_oauth_authorization(server_id).await?;
        clear_stored_oauth_credentials(server_id).await
    }
}

async fn handle_oauth_callback(
    State(state): State<OAuthCallbackAppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let payload = OAuthCallbackPayload {
        code: params.get("code").cloned(),
        state: params.get("state").cloned(),
        error: params.get("error").cloned(),
        error_description: params.get("error_description").cloned(),
    };
    let accept_language = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    let locale =
        resolve_oauth_callback_locale(Some(state.preferred_language.as_str()), accept_language);
    let page = render_oauth_callback_page(&payload, locale);

    if let Some(callback_tx) = state.callback_tx.lock().await.take() {
        let _ = callback_tx.send(payload);
    }

    Html(page)
}
