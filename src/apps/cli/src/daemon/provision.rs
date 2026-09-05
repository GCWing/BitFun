//! Narrow SSH bootstrap surface for an account-backed daemon.
//!
//! The controller first reads the non-secret machine identity, asks the relay
//! to mint a distinct device token, then stages one owner-only request file.
//! This command consumes and removes that file, re-encrypts the session with
//! this machine's key, and installs the platform service transactionally.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use openbitfun_core::service::remote_connect::{
    session_store, validate_relay_base_url, DeviceIdentity,
};
use openbitfun_services_core::dispatch_contract::{
    DispatchAccountDaemonIdentity, DispatchAccountDaemonProvisionRequest,
    DispatchAccountDaemonProvisionResponse, DISPATCH_ACCOUNT_DAEMON_PROVISIONING_SCHEMA_VERSION,
};

const MAX_PROVISION_REQUEST_BYTES: u64 = 16 * 1024;

struct ProvisionRequestGuard(PathBuf);

impl Drop for ProvisionRequestGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) fn print_identity() -> Result<()> {
    let identity = DeviceIdentity::from_current_machine().context("resolve target identity")?;
    let response = DispatchAccountDaemonIdentity {
        device_id: identity.device_id,
        device_name: identity.device_name,
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

pub(crate) fn provision(request_path: PathBuf) -> Result<()> {
    ensure_private_request_file(&request_path)?;
    let _request_guard = ProvisionRequestGuard(request_path.clone());
    let bytes = std::fs::read(&request_path).context("read daemon provisioning request")?;
    let request: DispatchAccountDaemonProvisionRequest =
        serde_json::from_slice(&bytes).context("decode daemon provisioning request")?;
    let validated = validate_request(request)?;

    match session_store::load_session_detailed() {
        Ok(None) => {}
        Ok(Some(_)) => {
            return Err(anyhow!(
                "the target already has an account session; refusing to replace it"
            ));
        }
        Err(error) => {
            return Err(anyhow!(
                "the target account session cannot be read; refusing to replace it: {error}"
            ));
        }
    }

    session_store::save_session_with_device(
        &validated.token,
        &validated.user_id,
        &validated.master_key,
        &validated.relay_url,
        Some(&validated.device_id),
    )
    .context("persist provisioned account session")?;

    if let Err(error) = super::service::install_service_for_provisioning() {
        let _ = super::service::uninstall_service_for_provisioning();
        session_store::clear_session();
        return Err(error).context("install persistent OpenBitFun daemon service");
    }

    let response = DispatchAccountDaemonProvisionResponse {
        device_id: validated.device_id,
        service_installed: true,
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

/// Roll back only the account/device named by the controller. This prevents a
/// stale failed operation from logging out an unrelated session that appeared
/// on the target in the meantime.
pub(crate) fn deprovision(device_id: String, user_id: String) -> Result<()> {
    let existing = session_store::load_session_detailed().context("read target account session")?;
    let Some(existing) = existing else {
        println!("{{\"removed\":false}}");
        return Ok(());
    };
    if existing.device_id.as_deref() != Some(device_id.trim()) || existing.user_id != user_id.trim()
    {
        return Err(anyhow!(
            "the target account session changed; refusing stale provisioning rollback"
        ));
    }

    let uninstall_result = super::service::uninstall_service_for_provisioning();
    session_store::clear_session();
    uninstall_result.context("remove persistent OpenBitFun daemon service")?;
    println!("{{\"removed\":true}}");
    Ok(())
}

struct ValidatedProvisionRequest {
    token: String,
    user_id: String,
    master_key: [u8; 32],
    relay_url: String,
    device_id: String,
}

fn validate_request(
    request: DispatchAccountDaemonProvisionRequest,
) -> Result<ValidatedProvisionRequest> {
    if request.schema_version != DISPATCH_ACCOUNT_DAEMON_PROVISIONING_SCHEMA_VERSION {
        return Err(anyhow!("unsupported daemon provisioning schema"));
    }
    if request.token.len() != 64
        || !request
            .token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(anyhow!("invalid account device token"));
    }
    if request.user_id.trim().is_empty()
        || request.user_id.len() > 256
        || request.user_id.chars().any(char::is_control)
    {
        return Err(anyhow!("invalid account user id"));
    }

    let identity = DeviceIdentity::from_current_machine().context("resolve target identity")?;
    if request.device_id != identity.device_id {
        return Err(anyhow!(
            "daemon provisioning device identity does not match this target"
        ));
    }

    let master_key_bytes = BASE64
        .decode(request.master_key_base64.trim())
        .context("decode account master key")?;
    let master_key: [u8; 32] = master_key_bytes
        .try_into()
        .map_err(|_| anyhow!("invalid account master key length"))?;
    let relay_url = validate_relay_base_url(request.relay_url.trim())?;
    let relay_url = relay_url.as_str().trim_end_matches('/').to_string();

    Ok(ValidatedProvisionRequest {
        token: request.token,
        user_id: request.user_id.trim().to_string(),
        master_key,
        relay_url,
        device_id: request.device_id,
    })
}

fn ensure_private_request_file(path: &Path) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).context("inspect daemon provisioning request")?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_PROVISION_REQUEST_BYTES {
        return Err(anyhow!("invalid daemon provisioning request file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(anyhow!(
                "daemon provisioning request must be owned by the current user with mode 0600"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[test]
    fn token_shape_is_lowercase_hex() {
        let valid = "ab".repeat(32);
        assert!(valid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')));
        for invalid in ["A".repeat(64), "g".repeat(64), "a".repeat(63)] {
            assert!(
                invalid.len() != 64
                    || !invalid
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn provisioning_request_must_be_owner_only_and_not_a_symlink() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let temp = tempfile::tempdir().unwrap();
        let request = temp.path().join("request.json");
        std::fs::write(&request, b"{}").unwrap();
        std::fs::set_permissions(&request, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(ensure_private_request_file(&request).is_err());

        std::fs::set_permissions(&request, std::fs::Permissions::from_mode(0o600)).unwrap();
        ensure_private_request_file(&request).unwrap();

        let link = temp.path().join("request-link.json");
        symlink(&request, &link).unwrap();
        assert!(ensure_private_request_file(&link).is_err());
    }
}
