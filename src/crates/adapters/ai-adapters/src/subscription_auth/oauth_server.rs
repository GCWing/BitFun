//! Minimal loopback HTTP server used to capture browser OAuth redirects.
//!
//! Providers pre-bind a `TcpListener` on a registered port and hand it to
//! [`wait_for_callback`], which accepts connections, parses the redirect query
//! string, validates the `state`, serves an HTML result page, and returns the
//! query parameters.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// IPv4 loopback address used for the TCP listener.
pub(crate) const LOOPBACK_BIND_HOST: &str = "127.0.0.1";

/// Hostname used by provider-registered OAuth redirect URIs.
///
/// OAuth servers compare redirect URIs exactly. Codex and Antigravity register
/// `localhost`, while binding the local listener to IPv4 avoids macOS resolving
/// `localhost` to `::1` and missing a listener bound only on `127.0.0.1`.
pub(crate) const LOOPBACK_REDIRECT_HOST: &str = "localhost";

/// Builds a provider-registered `http://localhost:{port}{path}` redirect URI.
pub(crate) fn loopback_redirect_uri(port: u16, path: &str) -> String {
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("http://{LOOPBACK_REDIRECT_HOST}:{port}{path}")
}

/// Binds the first available provider-supported callback port. A final `0`
/// entry requests an ephemeral port for desktop OAuth providers that permit it.
///
/// Fallback is attempted only when a preferred port is already in use. The
/// returned port must be used to construct both the authorize and token
/// exchange redirect URI.
pub(crate) async fn bind_loopback_ports(ports: &[u16]) -> Result<(TcpListener, u16)> {
    let Some(preferred_port) = ports.first().copied() else {
        return Err(anyhow!("OAuth callback port list is empty"));
    };

    for (index, port) in ports.iter().copied().enumerate() {
        match TcpListener::bind((LOOPBACK_BIND_HOST, port)).await {
            Ok(listener) => {
                let actual_port = listener
                    .local_addr()
                    .context("read OAuth callback listener address")?
                    .port();
                if index > 0 {
                    log::warn!(
                        "OAuth callback port {preferred_port} is unavailable; using fallback port {actual_port}"
                    );
                }
                return Ok((listener, actual_port));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && index + 1 < ports.len() => {
                continue;
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "bind OAuth callback on {LOOPBACK_BIND_HOST}:{port} (is another app using this port?)"
                    )
                });
            }
        }
    }

    unreachable!("a non-empty callback port list always returns from the loop")
}

/// Accepts loopback connections until the OAuth redirect arrives on
/// `callback_path`, then returns its query parameters.
pub(crate) async fn wait_for_callback(
    listener: TcpListener,
    callback_path: &str,
    expected_state: &str,
) -> Result<HashMap<String, String>> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = match stream.read(&mut buf).await {
            Ok(0) => continue,
            Ok(n) => n,
            Err(err) => {
                log::debug!("subscription oauth callback read failed: {err}");
                continue;
            }
        };
        let request = String::from_utf8_lossy(&buf[..n]);
        let locale = preferred_locale(&request);
        let Some(request_line) = request.lines().next() else {
            write_response(
                &mut stream,
                400,
                &error_page(&callback_messages(locale).bad_request, locale),
            )
            .await;
            continue;
        };
        let target = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .to_string();
        let (path, query) = match target.split_once('?') {
            Some((path, query)) => (path, query),
            None => (target.as_str(), ""),
        };
        if path != callback_path {
            write_response(&mut stream, 404, "Not found").await;
            continue;
        }

        let params = parse_query(query);
        // Ignore unsolicited loopback requests instead of letting a local
        // process/browser probe terminate the real OAuth session. Validate
        // state before accepting provider errors for the same reason.
        match params.get("state") {
            Some(state) if state == expected_state => {}
            _ => {
                write_response(
                    &mut stream,
                    400,
                    &error_page(&callback_messages(locale).invalid_state, locale),
                )
                .await;
                continue;
            }
        }
        if let Some(error) = params.get("error") {
            let message = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| error.clone());
            write_response(&mut stream, 200, &error_page(&message, locale)).await;
            return Err(anyhow!("authorization failed: {message}"));
        }
        if params.get("code").map(String::is_empty).unwrap_or(true) {
            write_response(
                &mut stream,
                400,
                &error_page(&callback_messages(locale).missing_code, locale),
            )
            .await;
            return Err(anyhow!("authorization callback missing code"));
        }
        write_response(&mut stream, 200, &success_page(locale)).await;
        return Ok(params);
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => (pair, ""),
        };
        let key = urlencoding::decode(key)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| key.to_string());
        let value = urlencoding::decode(value)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| value.to_string());
        out.insert(key, value);
    }
    out
}

async fn write_response(stream: &mut tokio::net::TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(err) = stream.write_all(response.as_bytes()).await {
        log::debug!("subscription oauth callback response write failed: {err}");
    }
    let _ = stream.flush().await;
}

#[derive(Debug, Deserialize)]
struct CallbackMessages {
    success_title: String,
    success_message: String,
    error_title: String,
    bad_request: String,
    missing_code: String,
    invalid_state: String,
}

fn callback_locales() -> &'static HashMap<String, CallbackMessages> {
    static LOCALES: OnceLock<HashMap<String, CallbackMessages>> = OnceLock::new();
    LOCALES.get_or_init(|| {
        serde_json::from_str(include_str!("oauth_callback_locales.json"))
            .expect("embedded OAuth callback locales are valid JSON")
    })
}

fn callback_messages(locale: &str) -> &'static CallbackMessages {
    callback_locales()
        .get(locale)
        .or_else(|| callback_locales().get("en"))
        .expect("OAuth callback English locale is embedded")
}

fn preferred_locale(request: &str) -> &'static str {
    for line in request.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.eq_ignore_ascii_case("accept-language") {
            continue;
        }
        let language = value
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if language.starts_with("zh-tw") || language.starts_with("zh-hk") {
            return "zh-TW";
        }
        if language.starts_with("zh") {
            return "zh-CN";
        }
        break;
    }
    "en"
}

fn success_page(locale: &str) -> String {
    let messages = callback_messages(locale);
    result_page(locale, &messages.success_title, &messages.success_message)
}

fn error_page(message: &str, locale: &str) -> String {
    result_page(locale, &callback_messages(locale).error_title, message)
}

fn result_page(language: &str, title: &str, message: &str) -> String {
    let message = escape_html(message);
    format!(
        "<!doctype html><html lang=\"{language}\"><head><meta charset=\"utf-8\"><meta name=\"color-scheme\" content=\"light dark\"><title>{title}</title>\
<style>:root{{color-scheme:light dark;--page:#f5f7fb;--card:#ffffff;--title:#172033;--text:#5d687c;--shadow:rgba(31,41,55,.14)}}\
@media(prefers-color-scheme:dark){{:root{{--page:#0f172a;--card:#1e293b;--title:#e2e8f0;--text:#94a3b8;--shadow:rgba(0,0,0,.35)}}}}\
body{{font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;background:var(--page);color:var(--title);\
display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
.card{{background:var(--card);padding:32px 40px;border-radius:12px;max-width:420px;text-align:center;\
box-shadow:0 10px 30px var(--shadow)}}h1{{font-size:20px;margin:0 0 12px}}p{{margin:0;color:var(--text);\
line-height:1.5}}</style></head><body><div class=\"card\"><h1>{title}</h1><p>{message}</p></div></body></html>"
    )
}

/// Escapes text interpolated into the callback result page. Provider-supplied
/// `error_description` values must not be able to inject markup.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::{
        bind_loopback_ports, callback_messages, escape_html, loopback_redirect_uri,
        preferred_locale, success_page, wait_for_callback, LOOPBACK_BIND_HOST,
        LOOPBACK_REDIRECT_HOST,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn escapes_html_injection() {
        assert_eq!(
            escape_html("<script>alert(\"x\")</script>&'"),
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;&#39;"
        );
    }

    #[test]
    fn loopback_redirect_uri_uses_registered_localhost() {
        assert_eq!(
            loopback_redirect_uri(1455, "/auth/callback"),
            format!("http://{LOOPBACK_REDIRECT_HOST}:1455/auth/callback")
        );
        assert_eq!(
            loopback_redirect_uri(51121, "oauth-callback"),
            format!("http://{LOOPBACK_REDIRECT_HOST}:51121/oauth-callback")
        );
        assert_eq!(LOOPBACK_BIND_HOST, "127.0.0.1");
        assert_eq!(LOOPBACK_REDIRECT_HOST, "localhost");
    }

    #[tokio::test]
    async fn invalid_state_does_not_terminate_the_real_callback_session() {
        let listener = tokio::net::TcpListener::bind((LOOPBACK_BIND_HOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let waiter = tokio::spawn(async move {
            wait_for_callback(listener, "/auth/callback", "expected-state").await
        });

        let mut invalid = tokio::net::TcpStream::connect(address).await.unwrap();
        invalid
            .write_all(
                b"GET /auth/callback?error=denied&state=attacker-state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .unwrap();
        let mut invalid_response = Vec::new();
        invalid.read_to_end(&mut invalid_response).await.unwrap();
        assert!(String::from_utf8_lossy(&invalid_response).contains("400 Bad Request"));
        assert!(!waiter.is_finished());

        let mut valid = tokio::net::TcpStream::connect(address).await.unwrap();
        valid
            .write_all(
                b"GET /auth/callback?code=real-code&state=expected-state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .unwrap();
        let mut valid_response = Vec::new();
        valid.read_to_end(&mut valid_response).await.unwrap();
        assert!(String::from_utf8_lossy(&valid_response).contains("200 OK"));

        let params = waiter.await.unwrap().unwrap();
        assert_eq!(params.get("code").map(String::as_str), Some("real-code"));
    }

    #[test]
    fn callback_page_uses_browser_language_and_color_scheme() {
        assert_eq!(
            preferred_locale("GET / HTTP/1.1\r\nAccept-Language: zh-CN,zh;q=0.9\r\n"),
            "zh-CN"
        );
        assert_eq!(
            preferred_locale("GET / HTTP/1.1\r\nAccept-Language: zh-TW,zh;q=0.9\r\n"),
            "zh-TW"
        );
        assert_eq!(
            preferred_locale("GET / HTTP/1.1\r\nAccept-Language: en-US,en;q=0.9\r\n"),
            "en"
        );
        assert_ne!(
            callback_messages("zh-CN").success_title,
            callback_messages("en").success_title
        );
        let chinese = success_page("zh-CN");
        assert!(chinese.contains("lang=\"zh-CN\""));
        assert!(chinese.contains(&callback_messages("zh-CN").success_title));
        assert!(chinese.contains("prefers-color-scheme:dark"));
    }

    #[tokio::test]
    async fn falls_back_when_preferred_callback_port_is_occupied() {
        let occupied = tokio::net::TcpListener::bind((LOOPBACK_BIND_HOST, 0))
            .await
            .unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();

        let (fallback, actual_port) = bind_loopback_ports(&[occupied_port, 0]).await.unwrap();

        assert_ne!(actual_port, occupied_port);
        assert_eq!(fallback.local_addr().unwrap().port(), actual_port);
    }
}
