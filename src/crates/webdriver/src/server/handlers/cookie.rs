use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tauri::{
    webview::cookie::{time::OffsetDateTime, Cookie as NativeCookie, SameSite},
    Manager, WebviewWindow,
};

use super::get_session;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default, rename = "httpOnly")]
    pub http_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sameSite")]
    pub same_site: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddCookieRequest {
    cookie: Cookie,
}

fn current_webview_window(
    state: &Arc<AppState>,
    label: &str,
) -> Result<WebviewWindow, WebDriverErrorResponse> {
    state
        .app
        .get_webview_window(label)
        .ok_or_else(|| WebDriverErrorResponse::no_such_window(format!("Window not found: {label}")))
}

fn to_webdriver_cookie(cookie: &NativeCookie<'_>) -> Cookie {
    Cookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        path: cookie.path().map(ToOwned::to_owned),
        domain: cookie.domain().map(ToOwned::to_owned),
        secure: cookie.secure().unwrap_or(false),
        http_only: cookie.http_only().unwrap_or(false),
        expiry: cookie.expires_datetime().and_then(|value| {
            let timestamp = value.unix_timestamp();
            u64::try_from(timestamp).ok()
        }),
        same_site: cookie.same_site().map(|value| value.to_string()),
    }
}

fn parse_same_site(value: Option<&str>) -> Result<Option<SameSite>, WebDriverErrorResponse> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(value) if value.eq_ignore_ascii_case("strict") => Ok(Some(SameSite::Strict)),
        Some(value) if value.eq_ignore_ascii_case("lax") => Ok(Some(SameSite::Lax)),
        Some(value) if value.eq_ignore_ascii_case("none") => Ok(Some(SameSite::None)),
        Some(value) => Err(WebDriverErrorResponse::invalid_argument(format!(
            "Invalid SameSite value: {value}"
        ))),
    }
}

fn build_native_cookie(cookie: &Cookie) -> Result<NativeCookie<'static>, WebDriverErrorResponse> {
    let mut builder = NativeCookie::build((cookie.name.clone(), cookie.value.clone()));

    if let Some(path) = cookie.path.clone() {
        builder = builder.path(path);
    }
    if let Some(domain) = cookie.domain.clone() {
        builder = builder.domain(domain);
    }
    if cookie.secure {
        builder = builder.secure(true);
    }
    if cookie.http_only {
        builder = builder.http_only(true);
    }
    if let Some(same_site) = parse_same_site(cookie.same_site.as_deref())? {
        builder = builder.same_site(same_site);
    }
    if let Some(expiry) = cookie.expiry {
        let expiry = i64::try_from(expiry).map_err(|_| {
            WebDriverErrorResponse::invalid_argument("Cookie expiry is out of range")
        })?;
        let expiry = OffsetDateTime::from_unix_timestamp(expiry).map_err(|error| {
            WebDriverErrorResponse::invalid_argument(format!("Cookie expiry is invalid: {error}"))
        })?;
        builder = builder.expires(expiry);
    }

    Ok(builder.build())
}

pub async fn get_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = current_webview_window(&state, &session.current_window)?;
    let cookies = webview
        .cookies()
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
        })?
        .into_iter()
        .map(|cookie| to_webdriver_cookie(&cookie))
        .collect::<Vec<_>>();
    Ok(WebDriverResponse::success(cookies))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = current_webview_window(&state, &session.current_window)?;
    let cookie = webview
        .cookies()
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
        })?
        .into_iter()
        .find(|cookie| cookie.name() == name);

    let Some(cookie) = cookie else {
        return Err(WebDriverErrorResponse::no_such_cookie(format!(
            "Cookie '{name}' not found"
        )));
    };

    Ok(WebDriverResponse::success(to_webdriver_cookie(&cookie)))
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<AddCookieRequest>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = current_webview_window(&state, &session.current_window)?;
    let mut cookie_request = request.cookie;

    if cookie_request.domain.is_none() {
        if let Ok(url) = webview.url() {
            cookie_request.domain = url.host_str().map(str::to_owned);
        }
    }
    if cookie_request.path.is_none() {
        cookie_request.path = Some("/".to_string());
    }

    let cookie = build_native_cookie(&cookie_request)?;
    webview.set_cookie(cookie).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to set cookie: {error}"))
    })?;
    Ok(WebDriverResponse::null())
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = current_webview_window(&state, &session.current_window)?;
    let cookies = webview.cookies().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
    })?;

    for cookie in cookies.into_iter().filter(|cookie| cookie.name() == name) {
        webview.delete_cookie(cookie).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to delete cookie: {error}"))
        })?;
    }

    Ok(WebDriverResponse::null())
}

pub async fn delete_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = current_webview_window(&state, &session.current_window)?;
    let cookies = webview.cookies().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
    })?;

    for cookie in cookies {
        webview.delete_cookie(cookie).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to delete cookie: {error}"))
        })?;
    }

    Ok(WebDriverResponse::null())
}
