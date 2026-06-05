//! LAN mode: starts an embedded relay server on the local network.
//!
//! The desktop runs a mini relay server, and the QR code points to the local IP.

use anyhow::{anyhow, Result};
use log::info;

/// Detect the local LAN IP address.
pub fn get_local_ip() -> Result<String> {
    let ip = local_ip_address::local_ip().map_err(|e| anyhow!("failed to detect LAN IP: {e}"))?;
    Ok(ip.to_string())
}

/// Build the relay URL for LAN mode.
pub fn build_lan_relay_url(port: u16) -> Result<String> {
    let ip = get_local_ip()?;
    let url = format!("http://{ip}:{port}");
    info!("LAN relay URL: {url}");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_ip() {
        let ip = get_local_ip();
        // May fail in CI environments without network, so just check it doesn't panic
        if let Ok(ip) = ip {
            assert!(!ip.is_empty());
        }
    }
}
