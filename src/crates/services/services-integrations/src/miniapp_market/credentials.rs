use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "macos"))]
use std::sync::{Mutex, OnceLock};

#[cfg(not(target_os = "macos"))]
const KEYRING_SERVICE: &str = "openbitfun.bitfun.miniapp-market.v1";
const CREDENTIAL_ENTRY: &str = "github-oauth";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMarketCredentials {
    pub access_token: String,
    pub access_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
}

#[cfg(not(target_os = "macos"))]
fn keyring_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(not(target_os = "macos"))]
fn open_entry() -> Result<keyring_core::Entry, String> {
    if keyring_core::get_default_store().is_none() {
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        let store = zbus_secret_service_keyring_store::Store::new();
        #[cfg(not(any(
            target_os = "windows",
            all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            )
        )))]
        let store: keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> =
            Err(keyring_core::Error::NoDefaultStore);

        let store =
            store.map_err(|error| format!("initialize system credential store: {error}"))?;
        keyring_core::set_default_store(store);
    }
    keyring_core::Entry::new(KEYRING_SERVICE, CREDENTIAL_ENTRY)
        .map_err(|error| format!("open market credential entry: {error}"))
}

#[cfg(target_os = "macos")]
fn macos_credential_vault(
) -> Result<bitfun_services_core::credential_vault::CredentialVault, String> {
    let base = dirs::config_dir()
        .ok_or_else(|| "system config directory unavailable".to_string())?
        .join("bitfun")
        .join("data");
    Ok(
        bitfun_services_core::credential_vault::CredentialVault::new(
            base.join(".market_credentials_vault.key"),
            base.join("market_credentials_vault.json"),
        ),
    )
}

pub async fn load_market_credentials() -> Result<Option<StoredMarketCredentials>, String> {
    #[cfg(target_os = "macos")]
    {
        let Some(secret) = macos_credential_vault()?
            .get(CREDENTIAL_ENTRY)
            .await
            .map_err(|error| format!("read market credentials: {error:#}"))?
        else {
            return Ok(None);
        };
        return serde_json::from_slice(&secret)
            .map(Some)
            .map_err(|error| format!("parse market credentials: {error}"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        tokio::task::spawn_blocking(move || {
            let _guard = keyring_lock()
                .lock()
                .map_err(|_| "market credential lock poisoned".to_string())?;
            let entry = open_entry()?;
            let secret = match entry.get_secret() {
                Ok(secret) => secret,
                Err(keyring_core::Error::NoEntry) => return Ok(None),
                Err(error) => return Err(format!("read market credentials: {error}")),
            };
            serde_json::from_slice(&secret)
                .map(Some)
                .map_err(|error| format!("parse market credentials: {error}"))
        })
        .await
        .map_err(|error| format!("join market credential read: {error}"))?
    }
}

pub async fn save_market_credentials(credentials: &StoredMarketCredentials) -> Result<(), String> {
    let secret = serde_json::to_vec(credentials)
        .map_err(|error| format!("serialize market credentials: {error}"))?;
    #[cfg(target_os = "macos")]
    {
        return macos_credential_vault()?
            .set(CREDENTIAL_ENTRY, &secret)
            .await
            .map_err(|error| format!("write market credentials: {error:#}"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        tokio::task::spawn_blocking(move || {
            let _guard = keyring_lock()
                .lock()
                .map_err(|_| "market credential lock poisoned".to_string())?;
            open_entry()?
                .set_secret(&secret)
                .map_err(|error| format!("write market credentials: {error}"))
        })
        .await
        .map_err(|error| format!("join market credential write: {error}"))?
    }
}

pub async fn clear_market_credentials() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos_credential_vault()?
            .remove(CREDENTIAL_ENTRY)
            .await
            .map_err(|error| format!("delete market credentials: {error:#}"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        tokio::task::spawn_blocking(move || {
            let _guard = keyring_lock()
                .lock()
                .map_err(|_| "market credential lock poisoned".to_string())?;
            match open_entry()?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(error) => Err(format!("delete market credentials: {error}")),
            }
        })
        .await
        .map_err(|error| format!("join market credential delete: {error}"))?
    }
}
