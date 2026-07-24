//! Auto-start service integration for the CLI daemon.
//!
//! Linux: systemd user unit + `loginctl enable-linger` so the daemon starts at
//! boot and keeps running without an interactive login session.
//! macOS: LaunchAgent (`launchctl bootstrap`).
//! Other platforms: explicit unsupported error.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

#[cfg(target_os = "macos")]
const LAUNCH_AGENT_LABEL: &str = "com.bitfun.cli.daemon";

#[cfg(all(unix, not(target_os = "macos")))]
const SYSTEMD_UNIT_NAME: &str = "bitfun-cli-daemon.service";

fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("resolve current executable path")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn systemd_unit_path() -> Result<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .ok_or_else(|| anyhow!("cannot determine config directory"))?;
    Ok(config_home.join("systemd/user").join(SYSTEMD_UNIT_NAME))
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn render_systemd_unit(executable: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=BitFun CLI account device host\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={} daemon run\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        executable.display()
    )
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn render_launch_agent(executable: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key>\n\
         \t<string>{LAUNCH_AGENT_LABEL}</string>\n\
         \t<key>ProgramArguments</key>\n\
         \t<array>\n\
         \t\t<string>{}</string>\n\
         \t\t<string>daemon</string>\n\
         \t\t<string>run</string>\n\
         \t</array>\n\
         \t<key>RunAtLoad</key>\n\
         \t<true/>\n\
         \t<key>KeepAlive</key>\n\
         \t<dict>\n\
         \t\t<key>Crashed</key>\n\
         \t\t<true/>\n\
         \t</dict>\n\
         </dict>\n\
         </plist>\n",
        executable.display()
    )
}

fn run_command(program: &str, args: &[&str]) -> Result<std::process::Output> {
    std::process::Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("run `{program} {}`", args.join(" ")))
}

fn ensure_success(program: &str, args: &[&str]) -> Result<()> {
    let output = run_command(program, args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!(
        "`{program} {}` failed: {}",
        args.join(" "),
        stderr.trim()
    ))
}

/// Whether the auto-start service unit file exists.
fn service_unit_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        launch_agent_path().is_ok_and(|path| path.exists())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        systemd_unit_path().is_ok_and(|path| path.exists())
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn ensure_systemd_user_available() -> Result<()> {
    match run_command("systemctl", &["--user", "show-environment"]) {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(anyhow!(
            "systemd user session is not available (no user bus; common in containers, WSL, or SSH sessions without systemd --user).\n\
             Enable a systemd user session for this account, or run the daemon under your own supervisor instead:\n\
             \x20 bitfun daemon run"
        )),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_platform_service(executable: &Path) -> Result<String> {
    ensure_systemd_user_available()?;

    let unit_path = systemd_unit_path()?;
    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create systemd user directory {}", parent.display()))?;
    }
    std::fs::write(&unit_path, render_systemd_unit(executable))
        .with_context(|| format!("write systemd unit {}", unit_path.display()))?;

    ensure_success("systemctl", &["--user", "daemon-reload"])?;
    ensure_success(
        "systemctl",
        &["--user", "enable", "--now", SYSTEMD_UNIT_NAME],
    )?;

    // Linger keeps the user manager (and therefore the daemon) running without
    // an interactive login session, and starts it at boot. This is what makes
    // the device reachable right after a server reboot.
    let linger_note = match run_command("loginctl", &["enable-linger"]) {
        Ok(output) if output.status.success() => {
            "Linger enabled: the daemon starts at boot without login.".to_string()
        }
        _ => format!(
            "Warning: could not enable linger automatically; run `loginctl enable-linger {}` so the daemon starts at boot.",
            std::env::var("USER").unwrap_or_else(|_| "<user>".to_string())
        ),
    };

    Ok(format!(
        "Installed and started systemd user service {SYSTEMD_UNIT_NAME} ({}).\n{linger_note}",
        unit_path.display()
    ))
}

#[cfg(target_os = "macos")]
fn install_platform_service(executable: &Path) -> Result<String> {
    let plist_path = launch_agent_path()?;
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create LaunchAgents directory {}", parent.display()))?;
    }
    std::fs::write(&plist_path, render_launch_agent(executable))
        .with_context(|| format!("write LaunchAgent {}", plist_path.display()))?;

    let uid = unsafe { libc::getuid() };
    let domain = format!("gui/{uid}");
    // Best-effort bootout first so reinstall is idempotent.
    let _ = run_command(
        "launchctl",
        &["bootout", &format!("{domain}/{LAUNCH_AGENT_LABEL}")],
    );
    ensure_success(
        "launchctl",
        &["bootstrap", &domain, &plist_path.to_string_lossy()],
    )?;

    Ok(format!(
        "Installed and started LaunchAgent {LAUNCH_AGENT_LABEL} ({}).",
        plist_path.display()
    ))
}

#[cfg(not(unix))]
fn install_platform_service(_executable: &Path) -> Result<String> {
    Err(anyhow!(
        "daemon auto-start service is not supported on this platform; run `bitfun daemon run` in a terminal instead"
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn uninstall_platform_service() -> Result<String> {
    let unit_path = systemd_unit_path()?;
    if !unit_path.exists() {
        return Ok("Auto-start service is not installed.".to_string());
    }
    // disable --now stops and disables; ignore failure when already stopped.
    let _ = run_command(
        "systemctl",
        &["--user", "disable", "--now", SYSTEMD_UNIT_NAME],
    );
    std::fs::remove_file(&unit_path)
        .with_context(|| format!("remove systemd unit {}", unit_path.display()))?;
    let _ = run_command("systemctl", &["--user", "daemon-reload"]);
    Ok("Stopped and removed the systemd user service.".to_string())
}

#[cfg(target_os = "macos")]
fn uninstall_platform_service() -> Result<String> {
    let plist_path = launch_agent_path()?;
    if !plist_path.exists() {
        return Ok("Auto-start service is not installed.".to_string());
    }
    let uid = unsafe { libc::getuid() };
    let _ = run_command(
        "launchctl",
        &["bootout", &format!("gui/{uid}/{LAUNCH_AGENT_LABEL}")],
    );
    std::fs::remove_file(&plist_path)
        .with_context(|| format!("remove LaunchAgent {}", plist_path.display()))?;
    Ok("Stopped and removed the LaunchAgent.".to_string())
}

#[cfg(not(unix))]
fn uninstall_platform_service() -> Result<String> {
    Err(anyhow!(
        "daemon auto-start service is not supported on this platform"
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_service_active() -> Option<bool> {
    let output = run_command("systemctl", &["--user", "is-active", SYSTEMD_UNIT_NAME]).ok()?;
    Some(output.status.success())
}

#[cfg(target_os = "macos")]
fn platform_service_active() -> Option<bool> {
    let uid = unsafe { libc::getuid() };
    let output = run_command(
        "launchctl",
        &["print", &format!("gui/{uid}/{LAUNCH_AGENT_LABEL}")],
    )
    .ok()?;
    Some(output.status.success())
}

#[cfg(not(unix))]
fn platform_service_active() -> Option<bool> {
    None
}

/// Install and start the auto-start service. Requires a persisted account
/// session (the daemon logs in from `~/.bitfun/account_session.enc`).
pub(crate) fn install_service() -> Result<()> {
    match bitfun_core::service::remote_connect::session_store::load_session() {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(anyhow!(
                "not logged in; run `bitfun`, log in with `/login`, then re-run `bitfun daemon install`"
            ));
        }
        Err(error) => return Err(anyhow!("read account session: {error}")),
    }

    let executable = current_exe_path()?;
    let message = install_platform_service(&executable)?;
    println!("{message}");
    Ok(())
}

/// Stop and remove the auto-start service.
pub(crate) fn uninstall_service() -> Result<()> {
    let message = uninstall_platform_service()?;
    println!("{message}");
    Ok(())
}

/// Print daemon liveness and auto-start service status.
pub(crate) fn print_status() -> Result<()> {
    let running = super::pid::is_daemon_running();
    let installed = service_unit_installed();
    let active = platform_service_active();

    println!(
        "daemon process: {}",
        if running { "running" } else { "not running" }
    );
    println!(
        "auto-start service: {}",
        if installed {
            "installed"
        } else {
            "not installed"
        }
    );
    if let Some(active) = active {
        println!(
            "service state: {}",
            if active { "active" } else { "inactive" }
        );
    }
    if !installed && !running {
        println!("hint: `bitfun daemon install` keeps this device reachable after reboot");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_unit_runs_daemon_run_and_restarts_on_failure() {
        let unit = render_systemd_unit(Path::new("/home/u/.local/bin/bitfun"));
        assert!(unit.contains("ExecStart=/home/u/.local/bin/bitfun daemon run"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("After=network-online.target"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launch_agent_runs_daemon_run_at_load() {
        let plist = render_launch_agent(Path::new("/usr/local/bin/bitfun"));
        assert!(plist.contains("<string>/usr/local/bin/bitfun</string>"));
        assert!(plist.contains("<string>run</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains(LAUNCH_AGENT_LABEL));
    }
}
