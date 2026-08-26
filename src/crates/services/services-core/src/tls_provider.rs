//! Process-wide TLS crypto-provider selection for BitFun product clients.

/// Installs the workspace-owned ring provider before a provider-neutral TLS
/// client is constructed.
///
/// Repeated calls are safe: rustls keeps the first process-wide provider and
/// every reviewed BitFun build selects ring as its only provider.
pub fn ensure_ring_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    #[test]
    fn installs_the_workspace_owned_provider() {
        super::ensure_ring_crypto_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
