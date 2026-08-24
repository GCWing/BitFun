//! Windows-native correction for the persisted main-window state.
//!
//! BitFun drives [tauri_plugin_window_state] explicitly (`with_state_flags`
//! empty at registration, explicit save/restore around known geometry
//! boundaries). The plugin captures geometry through generic window queries.
//! On Windows the main window is undecorated, and when a quit happens while a
//! maximized frameless window is on screen the persisted entry can degrade to
//! `maximized: false` together with the stretched maximized frame stored as
//! normal bounds. Every later launch then faithfully restores that degenerate
//! near-fullscreen normal window instead of the remembered geometry.
//!
//! This module uses [`GetWindowPlacement`] as the authoritative maximized
//! signal: after each successful save the persisted `main` entry is corrected
//! in place when the native placement reports a maximized window. Unreadable
//! or missing files are never recreated or deleted.

use std::path::Path;

use tauri::Manager;
use tauri_plugin_window_state::AppHandleExt;

const MAIN_WINDOW_LABEL: &str = "main";

// ─── Authoritative maximized placement ────────────────────────────────────────

/// Native window placement facts, mirroring the maximized-signal parts of
/// Win32 `WINDOWPLACEMENT`.
///
/// Kept platform-independent so the correction logic is unit-testable
/// everywhere; only the query itself is Windows-specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativePlacementReport {
    pub show_cmd: i32,
    pub restore_to_maximized: bool,
}

impl NativePlacementReport {
    /// Whether the placement describes a window that is zoomed now or will be
    /// maximized once it leaves the minimized state.
    ///
    /// `show_cmd` comparison targets `SW_SHOWMAXIMIZED`; the constant is
    /// inlined because this type is shared across platforms.
    pub(crate) fn reports_maximized(&self) -> bool {
        const SW_SHOWMAXIMIZED: i32 = 3;
        self.show_cmd == SW_SHOWMAXIMIZED || self.restore_to_maximized
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::NativePlacementReport;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS, WPF_RESTORETOMAXIMIZED,
    };

    pub(super) fn query(hwnd_inner: isize) -> Option<NativePlacementReport> {
        let mut placement = WINDOWPLACEMENT::default();
        placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        // SAFETY: the handle belongs to the live main window and the output
        // buffer outlives the single call.
        unsafe { GetWindowPlacement(HWND(hwnd_inner as *mut _), &mut placement) }.ok()?;
        Some(NativePlacementReport {
            show_cmd: placement.showCmd as i32,
            restore_to_maximized: placement.flags & WPF_RESTORETOMAXIMIZED
                != WINDOWPLACEMENT_FLAGS(0),
        })
    }
}

#[cfg(target_os = "windows")]
fn query_native_window_placement(window: &tauri::WebviewWindow) -> Option<NativePlacementReport> {
    let handle = window.hwnd().ok()?;
    native::query(handle.0 as isize)
}

// ─── Persisted-state correction ───────────────────────────────────────────────

/// Flags the persisted `main` entry as maximized when the authoritative native
/// placement disagrees with what the plugin captured.
///
/// Geometry fields are deliberately never rewritten: for a maximized
/// undecorated window `rcNormalPosition` is unreliable (it has been observed
/// mixing the pre-restore centered origin with monitor-sized dimensions), so
/// the last persisted normal bounds stay authoritative.
///
/// Returns `true` when the flag flipped. Non-maximized placements and entries
/// already marked maximized never modify the document.
pub(crate) fn apply_maximized_correction(
    document: &mut serde_json::Value,
    report: &NativePlacementReport,
) -> bool {
    if !report.reports_maximized() {
        return false;
    }

    let Some(entry) = document
        .get_mut(MAIN_WINDOW_LABEL)
        .and_then(|value| value.as_object_mut())
    else {
        return false;
    };

    set_bool_if_changed(entry, "maximized", true)
}

/// Reads the persisted `maximized` flag of the `main` entry so the restore
/// path can re-assert the maximized state after the window becomes visible.
#[cfg(target_os = "windows")]
pub(crate) fn read_persisted_main_maximized(app: &tauri::AppHandle) -> Option<bool> {
    let config_dir = app.path().app_config_dir().ok()?;
    let state_path = config_dir.join(app.filename());
    let bytes = std::fs::read(state_path).ok()?;
    let document: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    document.get(MAIN_WINDOW_LABEL)?.get("maximized")?.as_bool()
}

fn set_bool_if_changed(
    entry: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: bool,
) -> bool {
    if entry.get(key).and_then(serde_json::Value::as_bool) == Some(value) {
        return false;
    }
    entry.insert(key.to_string(), serde_json::Value::Bool(value));
    true
}

/// Post-corrects the saved state file after a successful plugin save.
///
/// Skipped unless the authoritative placement says the window is maximized.
/// Existing files are never created or deleted; unparsable content is logged
/// and left untouched.
#[cfg(target_os = "windows")]
pub(crate) fn correct_saved_main_window_state(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::debug!("Saved main-window state correction skipped: main window not found");
        return;
    };
    let Some(report) = query_native_window_placement(&window) else {
        log::debug!("Saved main-window state correction skipped: native placement unavailable");
        return;
    };
    if !report.reports_maximized() {
        return;
    }

    let Ok(config_dir) = app.path().app_config_dir() else {
        log::warn!("Saved main-window state correction skipped: app config dir unavailable");
        return;
    };
    let state_path = config_dir.join(app.filename());

    match correct_saved_state_file(&state_path, &report) {
        Ok(_) => {}
        Err(error) => {
            log::warn!("Failed to correct persisted main-window state: {}", error)
        }
    }
}

fn correct_saved_state_file(
    state_path: &Path,
    report: &NativePlacementReport,
) -> Result<bool, String> {
    let bytes = std::fs::read(state_path).map_err(|error| format!("read failed: {}", error))?;
    let mut document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "state file is not valid JSON, keeping it untouched: {}",
            error
        )
    })?;

    if !apply_maximized_correction(&mut document, report) {
        return Ok(false);
    }

    let serialized = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("serialize failed: {}", error))?;
    let temporary_path = state_path.with_extension("json.tmp");
    std::fs::write(&temporary_path, serialized)
        .map_err(|error| format!("temporary write failed: {}", error))?;
    replace_state_file_atomically(state_path, &temporary_path)?;
    Ok(true)
}

fn replace_state_file_atomically(state_path: &Path, temporary_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACEFILE_WRITE_THROUGH};

        let state_path_wide: Vec<u16> = state_path
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();
        let temporary_path_wide: Vec<u16> = temporary_path
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        // SAFETY: both UTF-16 buffers are NUL-terminated and live for the
        // duration of the call. The backup and reserved parameters are unused.
        unsafe {
            ReplaceFileW(
                PCWSTR::from_raw(state_path_wide.as_ptr()),
                PCWSTR::from_raw(temporary_path_wide.as_ptr()),
                None,
                REPLACEFILE_WRITE_THROUGH,
                None,
                None,
            )
        }
        .map_err(|error| format!("atomic replace failed: {}", error))?;
    }

    #[cfg(not(target_os = "windows"))]
    std::fs::rename(temporary_path, state_path)
        .map_err(|error| format!("atomic rename failed: {}", error))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn maximized_report() -> NativePlacementReport {
        NativePlacementReport {
            show_cmd: 3,
            restore_to_maximized: false,
        }
    }

    /// Mirrors the degraded shape observed in the wild: stretched maximized
    /// frame stored as normal bounds with `maximized: false`.
    fn degraded_document() -> serde_json::Value {
        json!({
            "main": {
                "width": 2560,
                "height": 1537,
                "x": -11,
                "y": -11,
                "prev_x": -11,
                "prev_y": -11,
                "maximized": false,
                "visible": true,
                "decorated": true,
                "fullscreen": false,
            }
        })
    }

    fn flipped_degraded_document() -> serde_json::Value {
        let mut document = degraded_document();
        document["main"]["maximized"] = json!(true);
        document
    }

    #[test]
    fn degraded_maximized_entry_flips_flag_without_touching_geometry() {
        let mut document = degraded_document();

        let changed = apply_maximized_correction(&mut document, &maximized_report());

        assert!(changed);
        assert_eq!(document, flipped_degraded_document());
    }

    #[test]
    fn correction_is_idempotent() {
        let mut document = degraded_document();
        assert!(apply_maximized_correction(
            &mut document,
            &maximized_report()
        ));
        // Second pass on the already-flipped entry must be a no-op: geometry
        // fields must never be rewritten from the untrustworthy placement.
        assert!(!apply_maximized_correction(
            &mut document,
            &maximized_report()
        ));
    }

    #[test]
    fn already_maximized_entry_is_never_rewritten() {
        let mut document = flipped_degraded_document();

        assert!(!apply_maximized_correction(
            &mut document,
            &maximized_report()
        ));
        assert_eq!(document, flipped_degraded_document());
    }

    #[test]
    fn non_maximized_placement_never_modifies_the_document() {
        let mut document = degraded_document();
        let mut report = maximized_report();
        report.show_cmd = 1;

        assert!(!apply_maximized_correction(&mut document, &report));
        assert_eq!(document, degraded_document());
    }

    #[test]
    fn minimized_restore_to_maximized_flag_counts_as_maximized() {
        let mut report = maximized_report();
        report.show_cmd = 2;
        report.restore_to_maximized = true;

        assert!(report.reports_maximized());
    }

    #[test]
    fn missing_main_entry_is_ignored() {
        let mut document = json!({ "other_window": { "width": 5 } });

        assert!(!apply_maximized_correction(
            &mut document,
            &maximized_report()
        ));
        assert_eq!(
            document.get("other_window").unwrap().get("width"),
            Some(&json!(5))
        );
    }

    #[test]
    fn legacy_partial_entry_is_tolerated_and_completed() {
        let mut document = json!({ "main": { "width": 100 } });

        assert!(apply_maximized_correction(
            &mut document,
            &maximized_report()
        ));
        let entry = document.get("main").unwrap();
        assert_eq!(entry.get("maximized"), Some(&json!(true)));
        assert_eq!(entry.get("width"), Some(&json!(100)));
        assert!(entry.get("visible").is_none());
    }

    #[test]
    fn saved_state_file_round_trip_flips_only_the_flag() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state_path = directory.path().join(".window-state.json");
        std::fs::write(&state_path, degraded_document().to_string()).expect("seed state file");

        let changed =
            correct_saved_state_file(&state_path, &maximized_report()).expect("correction");

        assert!(changed);
        let corrected: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).expect("reread"))
                .expect("corrected state parses");
        assert_eq!(corrected["main"]["maximized"], json!(true));
        assert_eq!(corrected["main"], flipped_degraded_document()["main"]);
        assert!(!state_path.with_extension("json.tmp").exists());
    }

    #[test]
    fn saved_state_file_keeps_invalid_content_untouched() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state_path = directory.path().join(".window-state.json");
        std::fs::write(&state_path, "{not json").expect("seed invalid state file");

        let error = correct_saved_state_file(&state_path, &maximized_report())
            .expect_err("invalid content must fail instead of being replaced");

        assert!(error.contains("not valid JSON"));
        assert_eq!(
            std::fs::read_to_string(&state_path).expect("content preserved"),
            "{not json"
        );
    }

    #[test]
    fn failed_state_file_replacement_keeps_original_content() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state_path = directory.path().join(".window-state.json");
        let missing_temporary_path = directory.path().join("missing.json.tmp");
        std::fs::write(&state_path, "original").expect("seed state file");

        let error = replace_state_file_atomically(&state_path, &missing_temporary_path)
            .expect_err("missing replacement must fail");

        assert!(error.contains("replace") || error.contains("rename"));
        assert_eq!(
            std::fs::read_to_string(&state_path).expect("original content preserved"),
            "original"
        );
    }
}
