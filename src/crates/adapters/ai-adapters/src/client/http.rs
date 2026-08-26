use crate::client::AIClient;
use crate::types::ProxyConfig;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use reqwest::{Client, Proxy};

pub(crate) fn create_http_client(
    proxy_config: Option<ProxyConfig>,
    skip_ssl_verify: bool,
) -> Client {
    bitfun_services_core::tls_provider::ensure_ring_crypto_provider();
    let mut builder = Client::builder()
        .tls_backend_rustls()
        .connect_timeout(std::time::Duration::from_secs(
            AIClient::STREAM_CONNECT_TIMEOUT_SECS,
        ))
        .user_agent("BitFun/1.0")
        .pool_idle_timeout(std::time::Duration::from_secs(
            AIClient::HTTP_POOL_IDLE_TIMEOUT_SECS,
        ))
        .pool_max_idle_per_host(4)
        .tcp_keepalive(Some(std::time::Duration::from_secs(
            AIClient::HTTP_TCP_KEEPALIVE_SECS,
        )))
        .danger_accept_invalid_certs(skip_ssl_verify);

    if skip_ssl_verify {
        warn!(
            "SSL certificate verification disabled - security risk, use only in test environments"
        );
    }

    if let Some(proxy_cfg) = proxy_config {
        if proxy_cfg.enabled && !proxy_cfg.url.is_empty() {
            match build_proxy(&proxy_cfg) {
                Ok(proxy) => {
                    info!("Using proxy: {}", proxy_cfg.url);
                    builder = builder.proxy(proxy);
                }
                Err(e) => {
                    error!(
                        "Proxy configuration failed: {}, proceeding without proxy",
                        e
                    );
                    builder = builder.no_proxy();
                }
            }
        } else {
            builder = builder.no_proxy();
        }
    } else {
        builder = builder.no_proxy();
    }

    match builder.build() {
        Ok(client) => client,
        Err(e) => {
            error!(
                "HTTP client initialization failed: {}, using default client",
                e
            );
            Client::new()
        }
    }
}

pub(crate) fn build_proxy(config: &ProxyConfig) -> Result<Proxy> {
    let proxy_url = normalize_proxy_url(&config.url);
    let mut proxy = Proxy::all(&proxy_url).map_err(|e| anyhow!("Failed to create proxy: {}", e))?;

    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        if !username.is_empty() && !password.is_empty() {
            proxy = proxy.basic_auth(username, password);
            debug!("Proxy authentication configured for user: {}", username);
        }
    }

    Ok(proxy)
}

fn normalize_proxy_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::{build_proxy, normalize_proxy_url};
    use crate::types::ProxyConfig;

    #[test]
    fn normalizes_bare_host_and_port_to_http_proxy_url() {
        assert_eq!(
            normalize_proxy_url("127.0.0.1:7897"),
            "http://127.0.0.1:7897"
        );
    }

    #[test]
    fn preserves_explicit_proxy_scheme() {
        assert_eq!(
            normalize_proxy_url("socks5://127.0.0.1:1080"),
            "socks5://127.0.0.1:1080"
        );
    }

    #[test]
    fn accepts_bare_host_and_port_proxy_configuration() {
        let config = ProxyConfig {
            enabled: true,
            url: "127.0.0.1:7897".to_string(),
            username: None,
            password: None,
        };

        assert!(build_proxy(&config).is_ok());
    }

    #[test]
    fn accepts_explicit_socks5_proxy_configuration() {
        let config = ProxyConfig {
            enabled: true,
            url: "socks5://127.0.0.1:1080".to_string(),
            username: None,
            password: None,
        };

        assert!(build_proxy(&config).is_ok());
    }
}
