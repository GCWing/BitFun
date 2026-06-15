//! ngrok tunnel mode for Remote Connect.
//!
//! Supports macOS (pgrep) and Windows (tasklist) for process detection.

use crate::util::process_manager;
use anyhow::{anyhow, Result};
use log::{info, warn};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Tracks the PID of the ngrok process we started, so it can be killed
/// synchronously during application exit even if async cleanup didn't run.
static NGROK_PID: AtomicU32 = AtomicU32::new(0);

/// Find the ngrok binary, checking common locations beyond just PATH.
fn find_ngrok() -> Option<PathBuf> {
    if let Ok(path) = which::which("ngrok") {
        return Some(path);
    }

    let candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/local/bin/ngrok"),
        PathBuf::from("/opt/homebrew/bin/ngrok"),
        dirs::home_dir()
            .map(|h| h.join("ngrok"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join(".ngrok/ngrok"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join("bin/ngrok"))
            .unwrap_or_default(),
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
            PathBuf::from(format!("{appdata}\\ngrok\\ngrok.exe"))
        },
        #[cfg(target_os = "windows")]
        PathBuf::from("C:\\ngrok\\ngrok.exe"),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists() && path.is_file())
}

/// Check if ngrok is installed and available.
pub async fn is_ngrok_available() -> bool {
    find_ngrok().is_some()
}

/// Check if any ngrok process is already running on the system.
/// Returns `Some(pids)` if found, `None` if not.
pub fn detect_running_ngrok() -> Option<Vec<u32>> {
    let pids = list_ngrok_pids();
    if pids.is_empty() {
        None
    } else {
        Some(pids)
    }
}

#[cfg(unix)]
fn list_ngrok_pids() -> Vec<u32> {
    std::process::Command::new("pgrep")
        .args(["-x", "ngrok"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                Some(
                    text.lines()
                        .filter_map(|l| l.trim().parse::<u32>().ok())
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[cfg(windows)]
fn list_ngrok_pids() -> Vec<u32> {
    process_manager::create_command("tasklist")
        .args(["/FI", "IMAGENAME eq ngrok.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()
        .map(|out| {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .filter_map(|line| {
                    // CSV format: "ngrok.exe","PID",...
                    let parts: Vec<&str> = line.split(',').collect();
                    parts
                        .get(1)
                        .and_then(|s| s.trim_matches('"').trim().parse::<u32>().ok())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Start an ngrok HTTP tunnel and return the public URL.
///
/// Parses the tunnel URL directly from ngrok's stdout JSON logs instead of
/// querying the shared 4040 API, which avoids conflicts with any pre-existing
/// ngrok process.
///
/// Returns a descriptive error if:
/// - ngrok is not installed
/// - another ngrok process is already running
/// - the tunnel fails to establish within the timeout
pub async fn start_ngrok_tunnel(local_port: u16) -> Result<NgrokTunnel> {
    let ngrok_path = find_ngrok().ok_or_else(|| {
        anyhow!(
            "ngrok is not installed.\n\
             Please install ngrok and configure your auth token, then retry.\n\
             No need to start ngrok manually — BitFun will start it automatically.\n\
             Setup guide: https://dashboard.ngrok.com/get-started/setup"
        )
    })?;

    if let Some(pids) = detect_running_ngrok() {
        return Err(anyhow!(
            "An ngrok process is already running (PID: {}).\n\
             Please stop the existing ngrok process before starting a new tunnel,\n\
             or use the existing tunnel directly.",
            pids.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    info!("Using ngrok at: {}", ngrok_path.display());

    let mut child = process_manager::create_tokio_command(&ngrok_path)
        .args([
            "http",
            &local_port.to_string(),
            "--log",
            "stdout",
            "--log-format",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow!(
                "Failed to start ngrok process: {e}\n\
                 Please ensure ngrok is installed and your auth token is configured \
                 (run: ngrok config add-authtoken <YOUR_TOKEN>).\n\
                 No need to start ngrok manually — BitFun will start it automatically."
            )
        })?;

    let pid = child.id().unwrap_or(0);
    NGROK_PID.store(pid, Ordering::Relaxed);
    info!("ngrok process started, pid={pid}");

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture ngrok stdout"))?;

    let public_url = match parse_tunnel_url_from_stdout(stdout).await {
        Ok(url) => url,
        Err(e) => {
            let _ = child.kill().await;
            return Err(anyhow!(
                "ngrok tunnel failed to establish: {e}\n\
                 Possible causes:\n\
                 - ngrok auth token not configured (run: ngrok config add-authtoken <YOUR_TOKEN>)\n\
                 - Network connectivity issue\n\
                 - ngrok service outage\n\
                 Note: You do not need to start ngrok manually."
            ));
        }
    };

    info!("ngrok tunnel established: {public_url}");

    Ok(NgrokTunnel {
        public_url,
        local_port,
        pid: Some(pid),
        process: Some(child),
    })
}

/// Read ngrok's JSON log lines from stdout until we find the tunnel URL.
/// ngrok v3 emits: `{"url":"https://xxx.ngrok-free.app", "msg":"started tunnel", ...}`
async fn parse_tunnel_url_from_stdout(stdout: tokio::process::ChildStdout) -> Result<String> {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);

    let (url_tx, url_rx) = tokio::sync::oneshot::channel::<String>();
    let mut url_tx = Some(url_tx);

    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        if let Some(tx) = url_tx.take() {
                            let _ = tx.send(url.to_string());
                        }
                    }
                }
            }
        }
        drop(url_tx);
    });

    match tokio::time::timeout_at(deadline, url_rx).await {
        Ok(Ok(url)) => Ok(url),
        Ok(Err(_)) => Err(anyhow!("ngrok exited before establishing a tunnel")),
        Err(_) => Err(anyhow!("timed out (15s)")),
    }
}

/// Force-kill an ngrok process by PID.
#[cfg(unix)]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
}

#[cfg(windows)]
fn kill_process(pid: u32) {
    let _ = process_manager::create_command("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}

pub struct NgrokTunnel {
    pub public_url: String,
    pub local_port: u16,
    pid: Option<u32>,
    process: Option<tokio::process::Child>,
}

impl NgrokTunnel {
    pub fn ws_url(&self) -> String {
        self.public_url
            .replace("https://", "wss://")
            .replace("http://", "ws://")
    }

    pub async fn stop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill().await;
            info!("ngrok tunnel stopped");
        }
        self.process = None;
        self.pid = None;
        NGROK_PID.store(0, Ordering::Relaxed);
    }
}

impl Drop for NgrokTunnel {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.start_kill();
        }
        if let Some(pid) = self.pid.take() {
            kill_process(pid);
            warn!("Force-killed ngrok process pid={pid} during cleanup");
        }
        NGROK_PID.store(0, Ordering::Relaxed);
    }
}

/// Synchronous cleanup: kill the ngrok process we started (if any).
/// Safe to call from exit handlers and drop implementations.
pub fn cleanup_all_ngrok() {
    let pid = NGROK_PID.swap(0, Ordering::Relaxed);
    if pid != 0 {
        info!("Cleaning up ngrok process pid={pid} on application exit");
        kill_process(pid);
    }
}
