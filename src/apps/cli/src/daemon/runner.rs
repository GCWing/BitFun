//! Headless daemon main loop.
//!
//! The daemon is a full Peer Device Mode host without the TUI: it initializes
//! the same core services as the interactive CLI, restores the persisted
//! account session, and holds the relay device-routing connection so this
//! device stays reachable whenever the machine is up.

use std::time::Duration;

use anyhow::{anyhow, Result};

use bitfun_core::service::remote_connect::DeviceIdentity;

use crate::{account, runtime, BootstrapProfile};

use super::pid;

/// Run the daemon in the foreground until signalled to stop, or until the
/// relay rejects the account token (logout / token revoked elsewhere).
pub(crate) async fn run_daemon() -> Result<()> {
    if pid::is_daemon_running() {
        return Err(anyhow!(
            "another bitfun daemon is already running (see `bitfun daemon status`)"
        ));
    }

    // The daemon is not bound to the caller's cwd; peer commands carry their
    // own workspace paths. Home is a stable root for the runtime context.
    let workspace_root = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let _runtime = crate::initialize_core_services(
        &workspace_root,
        runtime::approval::CliApprovalPolicy::Ask,
        BootstrapProfile::Interactive,
    )
    .await?;

    let Some(user_id) = account::try_restore_session().await else {
        return Err(anyhow!(
            "not logged in; run `bitfun`, log in with `/login`, then start the daemon again"
        ));
    };
    tracing::info!("Daemon restored account session for user {user_id}");

    let device =
        DeviceIdentity::from_current_machine().map_err(|e| anyhow!("detect device: {e}"))?;
    account::restore_device_routing(&device.device_name).await?;

    // Continuous account settings sync (30s pull + debounced push) so this
    // always-on host converges with cloud changes made on other devices and
    // attached controllers see fresh config without reconnecting.
    crate::account_sync::start_settings_sync_loop();

    pid::write_pid_file()?;
    tracing::info!("bitfun daemon running (pid {})", std::process::id());

    let mut expired_check = tokio::time::interval(Duration::from_secs(5));
    expired_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown_signal() => {
                tracing::info!("Daemon shutdown signal received");
                break;
            }
            _ = expired_check.tick() => {
                if account::is_token_expired() {
                    // Exit 0 on purpose: re-authentication needs a human, so
                    // Restart=on-failure must not loop the daemon.
                    tracing::warn!("Account token rejected by the relay; daemon exiting");
                    break;
                }
            }
        }
    }

    account::stop_device_routing().await;
    pid::remove_pid_file();
    crate::shutdown_mcp_servers().await;
    tracing::info!("bitfun daemon stopped");
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    match (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) {
        (Ok(mut sigterm), Ok(mut sigint)) => {
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sigint.recv() => {}
            }
        }
        _ => {
            tracing::warn!(
                "Failed to install signal handlers; daemon can only stop on token expiry"
            );
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
