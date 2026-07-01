//! LAN mode: starts an embedded relay server on the local network.
//!
//! The desktop runs a mini relay server, and the QR code points to the local IP.

use anyhow::{anyhow, Result};
use local_ip_address::list_afinet_netifas;
use log::info;
use std::net::IpAddr;

/// A local network interface with its IPv4 address.
#[derive(Debug, Clone)]
pub struct LocalNetworkInterface {
    pub interface_name: String,
    pub ip: String,
}

/// List all local LAN IPv4 addresses, sorted by likely-usefulness.
///
/// Private addresses (192.168.x, 10.x, 172.16-31.x) are prioritized over
/// public addresses.  Loopback (127.x) and link-local (169.254.x) are excluded.
pub fn list_local_ips() -> Result<Vec<LocalNetworkInterface>> {
    let interfaces =
        list_afinet_netifas().map_err(|e| anyhow!("failed to list network interfaces: {e}"))?;

    let mut entries: Vec<LocalNetworkInterface> = interfaces
        .into_iter()
        .filter(|(_, ip)| matches!(ip, IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local()))
        .filter_map(|(name, ip)| {
            // Only keep IPv4 for LAN relay URLs.
            let v4 = match ip {
                IpAddr::V4(v4) => v4,
                IpAddr::V6(_) => return None,
            };
            // Exclude loopback (127.x) and link-local (169.254.x) as a safety net
            // (is_loopback / is_link_local above already cover this, but be explicit).
            if v4.is_loopback() || v4.is_link_local() {
                return None;
            }
            Some(LocalNetworkInterface {
                interface_name: name,
                ip: v4.to_string(),
            })
        })
        .collect();

    // Sort: 192.168.x first, 10.x second, 172.16-31.x third, other IPv4 last.
    entries.sort_by(|a, b| ip_sort_key(&a.ip).cmp(&ip_sort_key(&b.ip)));

    if entries.is_empty() {
        return Err(anyhow!("no local IPv4 addresses found"));
    }
    Ok(entries)
}

/// Return a sort priority for an IPv4 string.
/// Lower value = higher priority (shown first).
fn ip_sort_key(ip: &str) -> u8 {
    if ip.starts_with("192.168.") {
        0
    } else if ip.starts_with("10.") {
        1
    } else if ip.starts_with("172.") {
        // 172.16.0.0 – 172.31.255.255 is private; treat all 172.x as private-tier.
        2
    } else {
        3
    }
}

/// Detect the local LAN IP address (first from the sorted list).
pub fn get_local_ip() -> Result<String> {
    let ips = list_local_ips()?;
    Ok(ips[0].ip.clone())
}

/// Build the relay URL for LAN mode, auto-detecting the local IP.
pub fn build_lan_relay_url(port: u16) -> Result<String> {
    let ip = get_local_ip()?;
    let url = format!("http://{ip}:{port}");
    info!("LAN relay URL: {url}");
    Ok(url)
}

/// Build the relay URL for LAN mode using a user-selected IP.
pub fn build_lan_relay_url_with_ip(port: u16, ip: &str) -> Result<String> {
    let url = format!("http://{ip}:{port}");
    info!("LAN relay URL (selected): {url}");
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

    #[test]
    fn test_ip_sort_key() {
        assert_eq!(ip_sort_key("192.168.1.100"), 0);
        assert_eq!(ip_sort_key("10.0.0.5"), 1);
        assert_eq!(ip_sort_key("172.16.0.1"), 2);
        assert_eq!(ip_sort_key("8.8.8.8"), 3);
    }

    #[test]
    fn test_list_local_ips_sorted() {
        let ips = list_local_ips();
        if let Ok(ips) = ips {
            assert!(!ips.is_empty());
            // Verify sorting: first entry should have the lowest sort key.
            let keys: Vec<u8> = ips.iter().map(|e| ip_sort_key(&e.ip)).collect();
            let mut sorted_keys = keys.clone();
            sorted_keys.sort();
            assert_eq!(keys, sorted_keys);
        }
    }
}
