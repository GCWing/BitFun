//! Enumerate running desktop applications on Windows.
//!
//! Mirrors the cua-driver-rs window enumeration model
//! (`platform-windows/src/win32/windows.rs` + `apps.rs`): walk `EnumWindows`
//! for every visible, non-minimized, titled top-level window, group by owning
//! pid, and resolve each pid's executable basename for the app name. This is
//! the Windows analogue of `macos_list_apps::list_running_apps` — a process
//! that has at least one visible top-level window is a "running app" the agent
//! can target.
//!
//! `QueryFullProcessImageNameW` / `OpenProcess` / `CloseHandle` are declared
//! via `extern "system"` (same convention as `windows_bg_input.rs`) so we do
//! not need to broaden the desktop crate's `windows` Cargo features.

#![cfg(target_os = "windows")]
#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

use bitfun_core::agentic::tools::computer_use_host::AppInfo;
use bitfun_core::util::errors::BitFunResult;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible,
};

type Handle = *mut c_void;

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(handle: Handle, flags: u32, buf: *mut u16, len: *mut u32) -> i32;
    fn CloseHandle(h: Handle) -> i32;
}

/// One visible top-level window discovered during enumeration.
struct WindowEntry {
    pid: u32,
    title: String,
}

struct EnumState {
    windows: Vec<WindowEntry>,
}

/// List running applications that own at least one visible, titled top-level
/// window, sorted by name. `include_hidden` is accepted for parity with the
/// macOS host; on Windows there is no per-app hidden flag, so every windowed
/// process is returned regardless.
pub(super) fn list_running_apps(_include_hidden: bool) -> BitFunResult<Vec<AppInfo>> {
    let windows = enumerate_windows();

    // Group by pid: keep the first non-empty title as a fallback display name.
    let mut by_pid: HashMap<u32, String> = HashMap::new();
    for w in windows {
        by_pid.entry(w.pid).or_insert(w.title);
    }

    let mut apps: Vec<AppInfo> = Vec::with_capacity(by_pid.len());
    for (pid, fallback_title) in by_pid {
        let name = exe_basename_for_pid(pid)
            .map(|exe| strip_exe_suffix(&exe))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback_title.clone());
        apps.push(AppInfo {
            name,
            bundle_id: None,
            pid: Some(pid as i32),
            running: true,
            last_used_ms: None,
            launch_count: 0,
        });
    }

    sort_apps_by_name(&mut apps);
    Ok(apps)
}

fn sort_apps_by_name(apps: &mut [AppInfo]) {
    apps.sort_by_cached_key(|app| app.name.to_lowercase());
}

/// Find a visible top-level window owned by `pid`, for callers (e.g.
/// `get_app_shortcuts`) that need an `HWND` to hand to UI Automation but
/// only have a pid. Returns the first visible, non-minimized window found
/// by `EnumWindows` order; does not require the window to be foreground.
pub(super) fn find_top_window_for_pid(pid: u32) -> Option<HWND> {
    struct FindState {
        target_pid: u32,
        found: Option<isize>,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut FindState);
        if IsWindowVisible(hwnd).0 == 0 || IsIconic(hwnd).0 != 0 {
            return TRUE;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == state.target_pid {
            state.found = Some(hwnd.0 as isize);
            return windows::Win32::Foundation::FALSE;
        }
        TRUE
    }
    let mut state = FindState {
        target_pid: pid,
        found: None,
    };
    let state_ptr = &mut state as *mut FindState as isize;
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(state_ptr));
    }
    state.found.map(|raw| HWND(raw as *mut c_void))
}

fn enumerate_windows() -> Vec<WindowEntry> {
    let state = Mutex::new(EnumState {
        windows: Vec::new(),
    });
    let state_ptr = &state as *const Mutex<EnumState> as isize;
    unsafe {
        let _ = EnumWindows(Some(enum_windows_cb), LPARAM(state_ptr));
    }
    state.into_inner().unwrap().windows
}

unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &*(lparam.0 as *const Mutex<EnumState>);

    // Skip invisible or minimized windows.
    if IsWindowVisible(hwnd).0 == 0 || IsIconic(hwnd).0 != 0 {
        return TRUE;
    }

    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return TRUE;
    }
    let mut buf = vec![0u16; (title_len + 1) as usize];
    let n = GetWindowTextW(hwnd, &mut buf);
    let title = {
        let len = (n as usize).min(buf.len());
        String::from_utf16_lossy(&buf[..len])
    };
    if title.trim().is_empty() {
        return TRUE;
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return TRUE;
    }

    if let Ok(mut s) = state.lock() {
        s.windows.push(WindowEntry { pid, title });
    }
    TRUE
}

/// Resolve the full image path of `pid` and return its `.exe` basename.
fn exe_basename_for_pid(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len: u32 = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) } != 0;
    unsafe {
        CloseHandle(handle);
    }
    if !ok || len == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Strip a trailing `.exe` (case-insensitive) from an executable basename so
/// the app name reads as `notepad` rather than `notepad.exe`.
fn strip_exe_suffix(basename: &str) -> String {
    if let Some(stripped) = basename
        .strip_suffix(".exe")
        .or_else(|| basename.strip_suffix(".EXE"))
    {
        stripped.to_string()
    } else {
        basename.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, pid: i32) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            bundle_id: None,
            pid: Some(pid),
            running: true,
            last_used_ms: None,
            launch_count: 0,
        }
    }

    #[test]
    fn app_name_sort_is_case_insensitive_ascending_and_stable() {
        let mut apps = vec![app("beta", 1), app("ALPHA", 2), app("Beta", 3)];

        sort_apps_by_name(&mut apps);

        assert_eq!(
            apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
            [Some(2), Some(1), Some(3)]
        );
    }
}
