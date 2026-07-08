use crate::client::AIClient;
use crate::types::ProxyConfig;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use reqwest::{Certificate, Client, Proxy};
use std::error::Error as StdError;

pub(crate) fn create_http_client(
    proxy_config: Option<ProxyConfig>,
    skip_ssl_verify: bool,
) -> Result<Client> {
    let proxy_enabled = proxy_config
        .as_ref()
        .is_some_and(|proxy_cfg| proxy_cfg.enabled && !proxy_cfg.url.is_empty());
    let mut builder = Client::builder()
        .use_rustls_tls()
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

    if !skip_ssl_verify {
        builder = builder.tls_certs_only(webpki_root_certificates()?);
    }

    if skip_ssl_verify {
        warn!(
            "SSL certificate verification disabled - security risk, use only in test environments"
        );
    }

    if let Some(proxy_cfg) = proxy_config.as_ref() {
        if proxy_cfg.enabled && !proxy_cfg.url.is_empty() {
            match build_proxy(proxy_cfg) {
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
        Ok(client) => Ok(client),
        Err(e) => {
            error!(
                "HTTP client initialization failed: {}; debug={:?}; source_chain={}; proxy_enabled={}; skip_ssl_verify={}",
                e,
                e,
                format_error_chain(&e),
                proxy_enabled,
                skip_ssl_verify
            );
            Err(anyhow!(
                "HTTP client initialization failed: {}; source_chain={}",
                e,
                format_error_chain(&e)
            ))
        }
    }
}

fn webpki_root_certificates() -> Result<Vec<Certificate>> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .map(|cert| {
            Certificate::from_der(cert.as_ref())
                .map_err(|e| anyhow!("Failed to load bundled webpki root certificate: {}", e))
        })
        .collect()
}

fn format_error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();

    while let Some(err) = source {
        parts.push(err.to_string());
        source = err.source();
    }

    parts.join(" | caused by: ")
}

fn build_proxy(config: &ProxyConfig) -> Result<Proxy> {
    let mut proxy =
        Proxy::all(&config.url).map_err(|e| anyhow!("Failed to create proxy: {}", e))?;

    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        if !username.is_empty() && !password.is_empty() {
            proxy = proxy.basic_auth(username, password);
            debug!("Proxy authentication configured for user: {}", username);
        }
    }

    Ok(proxy)
}
