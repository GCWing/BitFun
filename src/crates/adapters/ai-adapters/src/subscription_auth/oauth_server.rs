//! Minimal loopback HTTP server used to capture browser OAuth redirects.
//!
//! Providers pre-bind a `TcpListener` on their fixed port and hand it to
//! [`wait_for_callback`], which accepts connections, parses the redirect query
//! string, validates the `state`, serves an HTML result page, and returns the
//! query parameters.

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Loopback host used in both the OAuth `redirect_uri` and the TCP bind.
///
/// Prefer `127.0.0.1` over `localhost` so the browser and the listener always
/// agree on IPv4. On macOS, `localhost` often resolves to `::1`, which would
/// miss a listener bound only to `127.0.0.1` (and vice versa).
pub(crate) const LOOPBACK_HOST: &str = "127.0.0.1";

/// Builds `http://127.0.0.1:{port}{path}` for authorize/token exchange.
pub(crate) fn loopback_redirect_uri(port: u16, path: &str) -> String {
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("http://{LOOPBACK_HOST}:{port}{path}")
}

/// Binds the OAuth callback listener on [`LOOPBACK_HOST`].
pub(crate) async fn bind_loopback(port: u16) -> Result<TcpListener> {
    TcpListener::bind((LOOPBACK_HOST, port))
        .await
        .with_context(|| {
            format!(
                "bind OAuth callback on {LOOPBACK_HOST}:{port} (is another app using this port?)"
            )
        })
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
        let Some(request_line) = request.lines().next() else {
            write_response(&mut stream, 400, &error_page("Bad request")).await;
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
        if let Some(error) = params.get("error") {
            let message = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| error.clone());
            write_response(&mut stream, 200, &error_page(&message)).await;
            return Err(anyhow!("authorization failed: {message}"));
        }
        if params.get("code").map(String::is_empty).unwrap_or(true) {
            write_response(&mut stream, 400, &error_page("Missing authorization code")).await;
            return Err(anyhow!("authorization callback missing code"));
        }
        match params.get("state") {
            Some(state) if state == expected_state => {}
            _ => {
                write_response(&mut stream, 400, &error_page("Invalid state")).await;
                return Err(anyhow!("authorization state mismatch"));
            }
        }
        write_response(&mut stream, 200, &success_page()).await;
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

fn success_page() -> String {
    result_page(
        "Sign-in complete",
        "You are now signed in. You can close this window and return to BitFun.",
    )
}

fn error_page(message: &str) -> String {
    result_page("Sign-in failed", message)
}

fn result_page(title: &str, message: &str) -> String {
    let message = escape_html(message);
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
<style>body{{font-family:-apple-system,Segoe UI,Roboto,sans-serif;background:#0f172a;color:#e2e8f0;\
display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
.card{{background:#1e293b;padding:32px 40px;border-radius:12px;max-width:420px;text-align:center;\
box-shadow:0 10px 30px rgba(0,0,0,0.35)}}h1{{font-size:20px;margin:0 0 12px}}p{{margin:0;color:#94a3b8;\
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
    use super::{escape_html, loopback_redirect_uri, LOOPBACK_HOST};

    #[test]
    fn escapes_html_injection() {
        assert_eq!(
            escape_html("<script>alert(\"x\")</script>&'"),
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;&#39;"
        );
    }

    #[test]
    fn loopback_redirect_uri_uses_ipv4_literal() {
        assert_eq!(
            loopback_redirect_uri(1455, "/auth/callback"),
            format!("http://{LOOPBACK_HOST}:1455/auth/callback")
        );
        assert_eq!(
            loopback_redirect_uri(51121, "oauth-callback"),
            format!("http://{LOOPBACK_HOST}:51121/oauth-callback")
        );
        assert!(
            !loopback_redirect_uri(1455, "/auth/callback").contains("localhost"),
            "must not use localhost (macOS often resolves it to ::1)"
        );
    }
}
