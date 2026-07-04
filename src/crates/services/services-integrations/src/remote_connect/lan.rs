//! LAN mode helpers for Remote Connect.
//!
//! This module owns local-network interface discovery and LAN relay URL
//! construction so product assembly does not depend on OS network crates.

use anyhow::{anyhow, Result};
use local_ip_address::list_afinet_netifas;
use log::info;
use std::net::IpAddr;

/// A local network interface with its IPv4 address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNetworkInterface {
    pub interface_name: String,
    pub ip: String,
}

/// List all local LAN IPv4 addresses, sorted by likely usefulness.
///
/// Private addresses are prioritized over public addresses. Loopback and
/// link-local addresses are excluded.
pub fn list_local_ips() -> Result<Vec<LocalNetworkInterface>> {
    let interfaces = list_afinet_netifas()
        .map_err(|error| anyhow!("failed to list network interfaces: {error}"))?;

    let mut entries: Vec<LocalNetworkInterface> = interfaces
        .into_iter()
        .filter(|(_, ip)| matches!(ip, IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local()))
        .filter_map(|(name, ip)| {
            let v4 = match ip {
                IpAddr::V4(v4) => v4,
                IpAddr::V6(_) => return None,
            };
            if v4.is_loopback() || v4.is_link_local() {
                return None;
            }
            Some(LocalNetworkInterface {
                interface_name: name,
                ip: v4.to_string(),
            })
        })
        .collect();

    entries.sort_by(|left, right| ip_sort_key(&left.ip).cmp(&ip_sort_key(&right.ip)));

    if entries.is_empty() {
        return Err(anyhow!("no local IPv4 addresses found"));
    }
    Ok(entries)
}

/// Return a sort priority for an IPv4 string.
/// Lower value means higher priority.
fn ip_sort_key(ip: &str) -> u8 {
    if ip.starts_with("192.168.") {
        0
    } else if ip.starts_with("10.") {
        1
    } else if ip.starts_with("172.") {
        2
    } else {
        3
    }
}

/// Detect the local LAN IP address.
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
    fn ip_sort_key_prioritizes_private_lan_ranges() {
        assert_eq!(ip_sort_key("192.168.1.100"), 0);
        assert_eq!(ip_sort_key("10.0.0.5"), 1);
        assert_eq!(ip_sort_key("172.16.0.1"), 2);
        assert_eq!(ip_sort_key("8.8.8.8"), 3);
    }

    #[test]
    fn selected_lan_url_preserves_legacy_shape() {
        assert_eq!(
            build_lan_relay_url_with_ip(9700, "192.168.1.8").unwrap(),
            "http://192.168.1.8:9700"
        );
    }

    #[test]
    fn local_ip_detection_does_not_panic_without_network() {
        if let Ok(ip) = get_local_ip() {
            assert!(!ip.is_empty());
        }
    }
}
