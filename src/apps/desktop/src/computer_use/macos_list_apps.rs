//! Enumerate currently running GUI applications on macOS.
//!
//! We use AppleScript via `osascript` to read `System Events` —
//! pragmatically the same data NSWorkspace.runningApplications exposes,
//! without requiring a full objc/cocoa binding stack here. This is "good
//! enough" for the AX-first plan: the list is used to resolve
//! `AppSelector::ByName` / `ByBundleId` to a pid, after which all real work
//! happens through AX + bg-input.
//!
//! Last-used / launch-count signals from LaunchServices are not available
//! through AppleScript; we expose `last_used_at_ms = 0` and
//! `launch_count = 0` so the trait shape is preserved. A future enhancement
//! can swap this out for a real NSWorkspace + LSSharedFileList implementation
//! without changing callers.

#![allow(dead_code)]

use bitfun_core::agentic::tools::computer_use_host::AppInfo;
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Short-lived cache for `list_running_apps` results.
///
/// `osascript` cold-start costs ~150–250ms on a quiet machine. The AX-first
/// dispatch path resolves an `AppSelector → pid` *before every* `app_*`
/// action, so without caching every click would pay this latency twice
/// (once for the action, once for the post-action re-snapshot). A 5-second
/// TTL is short enough that newly-launched apps appear quickly while
/// eliminating the back-to-back duplicate calls inside one agent step.
static CACHE: Mutex<Option<(Instant, bool, Vec<AppInfo>)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(5);

const ASCRIPT: &str = r#"
set out to ""
tell application "System Events"
    set procs to (every application process whose background only is false)
    repeat with p in procs
        try
            set bid to bundle identifier of p
        on error
            set bid to ""
        end try
        try
            set pname to name of p
        on error
            set pname to ""
        end try
        try
            set ppid to unix id of p
        on error
            set ppid to 0
        end try
        try
            set ph to (visible of p as string)
        on error
            set ph to "true"
        end try
        set out to out & pname & "\t" & bid & "\t" & ppid & "\t" & ph & "\n"
    end repeat
end tell
return out
"#;

pub fn list_running_apps(include_hidden: bool) -> BitFunResult<Vec<AppInfo>> {
    if let Ok(guard) = CACHE.lock() {
        if let Some((ts, cached_hidden, ref apps)) = *guard {
            if cached_hidden == include_hidden && ts.elapsed() < CACHE_TTL {
                return Ok(apps.clone());
            }
        }
    }
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(ASCRIPT)
        .output()
        .map_err(|e| BitFunError::tool(format!("osascript spawn: {}", e)))?;
    if !out.status.success() {
        return Err(BitFunError::tool(format!(
            "osascript list_apps failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let mut apps = Vec::new();
    for line in body.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].trim().to_string();
        let bundle_id = {
            let s = parts[1].trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        };
        let pid: i32 = parts[2].trim().parse().unwrap_or(0);
        let visible = parts[3].trim().eq_ignore_ascii_case("true");
        if name.is_empty() || pid <= 0 {
            continue;
        }
        if !include_hidden && !visible {
            continue;
        }
        apps.push(AppInfo {
            name,
            bundle_id,
            pid: Some(pid),
            running: true,
            last_used_ms: None,
            launch_count: 0,
        });
    }
    // Best-effort stable order: alphabetical by name. The richer
    // "recently used / most launched" sort is left to a future
    // LaunchServices-backed implementation.
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), include_hidden, apps.clone()));
    }
    Ok(apps)
}

/// Drop the cached `list_running_apps` result so the next call re-probes
/// `osascript`. Used when the agent has just launched / quit an app and
/// needs the freshest pid set.
pub fn invalidate_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}
