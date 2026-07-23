//! Device identity for Remote Connect pairing and account device routing.
//!
//! `device_id` is generated once and persisted under `~/.bitfun/device_identity.json`.
//! Hostname/MAC are refreshed for display only — they must not rewrite `device_id`,
//! because macOS private Wi‑Fi addresses and interface order make MAC unstable.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

/// Represents a device's identity used for pairing and account routing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    pub mac_address: String,
}

static CACHED_IDENTITY: Mutex<Option<DeviceIdentity>> = Mutex::new(None);

#[cfg(test)]
thread_local! {
    static TEST_IDENTITY_PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

impl DeviceIdentity {
    /// Load the stable machine identity (persisted), creating it on first use.
    pub fn from_current_machine() -> Result<Self> {
        if let Ok(guard) = CACHED_IDENTITY.lock() {
            if let Some(cached) = guard.as_ref() {
                let mut identity = cached.clone();
                identity.device_name = get_hostname();
                identity.mac_address = get_mac_address();
                return Ok(identity);
            }
        }

        let mut identity = load_persisted()?.unwrap_or_else(compute_initial_identity);
        identity.device_name = get_hostname();
        identity.mac_address = get_mac_address();
        save_persisted(&identity)?;
        cache_identity(identity.clone());
        Ok(identity)
    }

    /// Align the local identity with the account-bound `device_id` from a
    /// login token / `AuthOk`. Needed when a prior MAC-derived id drifted
    /// while an existing session still authenticates as the old id.
    pub fn adopt_account_device_id(device_id: &str) -> Result<Self> {
        let device_id = device_id.trim();
        if !is_valid_device_id(device_id) {
            return Err(anyhow!("invalid account device_id"));
        }

        let mut identity = Self::from_current_machine()?;
        if identity.device_id == device_id {
            return Ok(identity);
        }

        log::info!(
            "Adopting account device_id {} (was {})",
            device_id,
            identity.device_id
        );
        identity.device_id = device_id.to_string();
        identity.device_name = get_hostname();
        identity.mac_address = get_mac_address();
        save_persisted(&identity)?;
        cache_identity(identity.clone());
        Ok(identity)
    }
}

fn is_valid_device_id(device_id: &str) -> bool {
    device_id.len() == 32 && device_id.chars().all(|c| c.is_ascii_hexdigit())
}

fn compute_initial_identity() -> DeviceIdentity {
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

    DeviceIdentity {
        device_id,
        device_name,
        mac_address,
    }
}

fn identity_file_path() -> Result<PathBuf> {
    #[cfg(test)]
    {
        let override_path = TEST_IDENTITY_PATH.with(|cell| cell.borrow().clone());
        if let Some(path) = override_path {
            return Ok(path);
        }
    }

    if let Ok(path) = std::env::var("BITFUN_DEVICE_IDENTITY_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("device_identity.json"))
}

fn load_persisted() -> Result<Option<DeviceIdentity>> {
    let path = identity_file_path()?;
    let json = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(anyhow!("read device identity: {e}").context(path.display().to_string()))
        }
    };
    let identity: DeviceIdentity =
        serde_json::from_str(&json).context("parse device identity file")?;
    if !is_valid_device_id(&identity.device_id) {
        return Err(anyhow!("persisted device_id is invalid"));
    }
    Ok(Some(identity))
}

fn save_persisted(identity: &DeviceIdentity) -> Result<()> {
    let path = identity_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create device identity dir")?;
    }
    let json = serde_json::to_string_pretty(identity).context("serialize device identity")?;
    std::fs::write(&path, json).context("write device identity file")?;
    Ok(())
}

fn cache_identity(identity: DeviceIdentity) {
    if let Ok(mut guard) = CACHED_IDENTITY.lock() {
        *guard = Some(identity);
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
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_identity_path<F: FnOnce()>(f: F) {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "bitfun-device-identity-{}-{}.json",
            std::process::id(),
            uuid_like()
        ));
        let _ = std::fs::remove_file(&path);
        TEST_IDENTITY_PATH.with(|cell| *cell.borrow_mut() = Some(path.clone()));
        if let Ok(mut cache) = CACHED_IDENTITY.lock() {
            *cache = None;
        }
        f();
        let _ = std::fs::remove_file(&path);
        TEST_IDENTITY_PATH.with(|cell| *cell.borrow_mut() = None);
        if let Ok(mut cache) = CACHED_IDENTITY.lock() {
            *cache = None;
        }
    }

    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{nanos:x}")
    }

    #[test]
    fn test_device_identity_creation() {
        with_temp_identity_path(|| {
            let identity = DeviceIdentity::from_current_machine().unwrap();
            assert!(!identity.device_id.is_empty());
            assert!(!identity.device_name.is_empty());
            assert_eq!(identity.device_id.len(), 32);
        });
    }

    #[test]
    fn test_device_identity_stable_across_calls() {
        with_temp_identity_path(|| {
            let id1 = DeviceIdentity::from_current_machine().unwrap();
            let id2 = DeviceIdentity::from_current_machine().unwrap();
            assert_eq!(id1.device_id, id2.device_id);
        });
    }

    #[test]
    fn test_device_identity_persists_across_cache_clear() {
        with_temp_identity_path(|| {
            let id1 = DeviceIdentity::from_current_machine().unwrap();
            if let Ok(mut cache) = CACHED_IDENTITY.lock() {
                *cache = None;
            }
            let id2 = DeviceIdentity::from_current_machine().unwrap();
            assert_eq!(id1.device_id, id2.device_id);
        });
    }

    #[test]
    fn test_adopt_account_device_id_rewrites_persisted_id() {
        with_temp_identity_path(|| {
            let original = DeviceIdentity::from_current_machine().unwrap();
            let adopted_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            let adopted = DeviceIdentity::adopt_account_device_id(adopted_id).unwrap();
            assert_eq!(adopted.device_id, adopted_id);
            assert_ne!(adopted.device_id, original.device_id);

            if let Ok(mut cache) = CACHED_IDENTITY.lock() {
                *cache = None;
            }
            let reloaded = DeviceIdentity::from_current_machine().unwrap();
            assert_eq!(reloaded.device_id, adopted_id);
        });
    }
}
