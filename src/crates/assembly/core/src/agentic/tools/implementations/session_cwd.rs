//! Session CWD store — direct port of deveco-code `session-cwd.ts`.
//!
//! A simple in-memory map from session ID to project directory.
//! `switch_cwd` writes to it; `devecocli_run::resolve_harmony_cwd` reads from it
//! before falling back to the workspace root.

use std::collections::HashMap;
use std::sync::OnceLock;

static SESSION_CWD_MAP: OnceLock<std::sync::RwLock<HashMap<String, String>>> = OnceLock::new();

fn map() -> &'static std::sync::RwLock<HashMap<String, String>> {
    SESSION_CWD_MAP.get_or_init(|| std::sync::RwLock::new(HashMap::new()))
}

pub(crate) fn set_session_cwd(session_id: &str, cwd: &str) {
    let id = session_id.trim();
    let dir = cwd.trim();
    if id.is_empty() || dir.is_empty() {
        return;
    }
    if let Ok(mut map) = map().write() {
        map.insert(id.to_string(), dir.to_string());
    }
}

pub(crate) fn get_session_cwd(session_id: Option<&str>) -> Option<String> {
    let id = session_id?.trim();
    if id.is_empty() {
        return None;
    }
    if let Ok(map) = map().read() {
        map.get(id).cloned()
    } else {
        None
    }
}

pub(crate) fn clear_session_cwd(session_id: Option<&str>) {
    if let Ok(mut map) = map().write() {
        match session_id {
            Some(id) => {
                let id = id.trim();
                if !id.is_empty() {
                    map.remove(id);
                }
            }
            None => {
                map.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_session_cwd() {
        set_session_cwd("test-session-1", "/tmp/project");
        assert_eq!(
            get_session_cwd(Some("test-session-1")),
            Some("/tmp/project".to_string())
        );
    }

    #[test]
    fn get_missing_session_returns_none() {
        assert!(get_session_cwd(Some("nonexistent")).is_none());
        assert!(get_session_cwd(None).is_none());
    }

    #[test]
    fn clear_removes_session_cwd() {
        set_session_cwd("test-session-clear", "/tmp/x");
        assert!(get_session_cwd(Some("test-session-clear")).is_some());
        clear_session_cwd(Some("test-session-clear"));
        assert!(get_session_cwd(Some("test-session-clear")).is_none());
    }

    #[test]
    fn empty_inputs_are_ignored() {
        set_session_cwd("", "/tmp/x");
        set_session_cwd("session", "");
        assert!(get_session_cwd(Some("")).is_none());
        assert!(get_session_cwd(Some("session")).is_none());
    }
}
