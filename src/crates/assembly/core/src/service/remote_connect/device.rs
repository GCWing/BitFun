//! Device identity for Remote Connect pairing.
//!
//! Generates a stable `device_id` from `SHA-256(hostname + MAC address)`.
//! Falls back gracefully when MAC or hostname are unavailable.

use anyhow::Result;
use sha2::{Digest, Sha256};

/// Represents a device's identity used for pairing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    pub mac_address: String,
}

impl DeviceIdentity {
    /// Build the device identity from the current machine.
    pub fn from_current_machine() -> Result<Self> {
        let device_name = get_hostname();
        let mac_address = get_mac_address();

        let mut hasher = Sha256::new();
        hasher.update(device_name.as_bytes());
        hasher.update(b":");
        hasher.update(mac_address.as_bytes());
        let hash = hasher.finalize();
        let device_id = hash[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        Ok(Self {
            device_id,
            device_name,
            mac_address,
        })
    }
}

fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown-host".to_string())
}

fn get_mac_address() -> String {
    mac_address::get_mac_address()
        .ok()
        .flatten()
        .map(|ma| ma.to_string())
        .unwrap_or_else(|| "00:00:00:00:00:00".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_identity_creation() {
        let identity = DeviceIdentity::from_current_machine().unwrap();
        assert!(!identity.device_id.is_empty());
        assert!(!identity.device_name.is_empty());
        assert_eq!(identity.device_id.len(), 32); // 16 bytes hex = 32 chars
    }

    #[test]
    fn test_device_identity_stable() {
        let id1 = DeviceIdentity::from_current_machine().unwrap();
        let id2 = DeviceIdentity::from_current_machine().unwrap();
        assert_eq!(id1.device_id, id2.device_id);
    }
}
