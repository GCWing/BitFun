use tauri::webview::cookie::{time::OffsetDateTime, Cookie as NativeCookie, SameSite};

use crate::executor::BridgeExecutor;
use crate::platform::Cookie;
use crate::server::response::WebDriverErrorResponse;

impl BridgeExecutor {
    pub async fn get_all_cookies(&self) -> Result<Vec<Cookie>, WebDriverErrorResponse> {
        let window = self.webview_window()?;
        let cookies = window.cookies().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
        })?;
        Ok(cookies.iter().map(to_webdriver_cookie).collect())
    }

    pub async fn get_cookie(&self, name: &str) -> Result<Option<Cookie>, WebDriverErrorResponse> {
        let cookies = self.get_all_cookies().await?;
        Ok(cookies.into_iter().find(|cookie| cookie.name == name))
    }

    pub async fn add_cookie(&self, mut cookie: Cookie) -> Result<(), WebDriverErrorResponse> {
        let window = self.webview_window()?;

        if cookie.domain.is_none() {
            if let Ok(url) = window.url() {
                cookie.domain = url.host_str().map(str::to_owned);
            }
        }
        if cookie.path.is_none() {
            cookie.path = Some("/".to_string());
        }

        let cookie = build_native_cookie(&cookie)?;
        window.set_cookie(cookie).map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to set cookie: {error}"))
        })?;
        Ok(())
    }

    pub async fn delete_cookie(&self, name: &str) -> Result<(), WebDriverErrorResponse> {
        let window = self.webview_window()?;
        let cookies = window.cookies().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
        })?;

        for cookie in cookies.into_iter().filter(|cookie| cookie.name() == name) {
            window.delete_cookie(cookie).map_err(|error| {
                WebDriverErrorResponse::unknown_error(format!("Failed to delete cookie: {error}"))
            })?;
        }

        Ok(())
    }

    pub async fn delete_all_cookies(&self) -> Result<(), WebDriverErrorResponse> {
        let window = self.webview_window()?;
        let cookies = window.cookies().map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!("Failed to read cookies: {error}"))
        })?;

        for cookie in cookies {
            window.delete_cookie(cookie).map_err(|error| {
                WebDriverErrorResponse::unknown_error(format!("Failed to delete cookie: {error}"))
            })?;
        }

        Ok(())
    }
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
