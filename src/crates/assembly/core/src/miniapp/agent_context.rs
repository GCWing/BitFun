//! Host-owned, in-memory context snapshots for MiniApp Agent turns.
//!
//! Marketplace MiniApps may write their own appdata and may have allowlisted
//! process capabilities. Keeping Agent context in that filesystem would make a
//! path check vulnerable to replacement races. This registry publishes bounded
//! immutable snapshots inside the Agent Runtime process instead. Read and Grep
//! resolve the virtual `.miniapp-context/<scope>` namespace through this store.

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path};
use std::sync::{Arc, OnceLock, RwLock};

pub const MINIAPP_AGENT_CONTEXT_DIR: &str = ".miniapp-context";
pub const MAX_MINIAPP_AGENT_CONTEXT_FILES: usize = 8;
pub const MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES: usize = 128;
pub const MAX_MINIAPP_AGENT_CONTEXT_SCOPES_PER_APP: usize = 8;
pub const MAX_MINIAPP_AGENT_CONTEXT_SCOPES_GLOBAL: usize = 64;
pub const MAX_MINIAPP_AGENT_CONTEXT_BYTES_GLOBAL: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAgentContextInput {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAgentContextSnapshot {
    pub scope: String,
    pub relative_root: String,
    pub file_names: Vec<String>,
}

#[derive(Debug)]
struct StoredSnapshot {
    app_id: String,
    session_id: Option<String>,
    turn_id: String,
    total_bytes: usize,
    files: Arc<BTreeMap<String, Arc<str>>>,
}

#[derive(Default)]
struct MiniAppAgentContextRegistry {
    by_scope: HashMap<String, StoredSnapshot>,
    total_bytes: usize,
}

impl MiniAppAgentContextRegistry {
    fn reserve(
        &mut self,
        app_id: &str,
        turn_id: &str,
        files: Arc<BTreeMap<String, Arc<str>>>,
        total_bytes: usize,
    ) -> Result<MiniAppAgentContextSnapshot, String> {
        let active_for_app = self
            .by_scope
            .values()
            .filter(|snapshot| snapshot.app_id == app_id)
            .count();
        if active_for_app >= MAX_MINIAPP_AGENT_CONTEXT_SCOPES_PER_APP {
            return Err(format!(
                "This MiniApp already has {} active context snapshots; wait for a turn to finish",
                MAX_MINIAPP_AGENT_CONTEXT_SCOPES_PER_APP
            ));
        }
        if self.by_scope.len() >= MAX_MINIAPP_AGENT_CONTEXT_SCOPES_GLOBAL {
            return Err(format!(
                "The Agent Runtime already has {} active MiniApp context snapshots; wait for a turn to finish",
                MAX_MINIAPP_AGENT_CONTEXT_SCOPES_GLOBAL
            ));
        }
        let next_total_bytes = self
            .total_bytes
            .checked_add(total_bytes)
            .ok_or_else(|| "MiniApp agent context memory accounting overflowed".to_string())?;
        if next_total_bytes > MAX_MINIAPP_AGENT_CONTEXT_BYTES_GLOBAL {
            return Err(format!(
                "MiniApp agent contexts exceed the {} byte Runtime limit",
                MAX_MINIAPP_AGENT_CONTEXT_BYTES_GLOBAL
            ));
        }

        let scope = loop {
            let candidate = uuid::Uuid::new_v4().simple().to_string();
            if !self.by_scope.contains_key(&candidate) {
                break candidate;
            }
        };
        let file_names = files.keys().cloned().collect::<Vec<_>>();
        self.by_scope.insert(
            scope.clone(),
            StoredSnapshot {
                app_id: app_id.to_string(),
                session_id: None,
                turn_id: turn_id.to_string(),
                total_bytes,
                files,
            },
        );
        self.total_bytes = next_total_bytes;

        Ok(MiniAppAgentContextSnapshot {
            relative_root: format!("{MINIAPP_AGENT_CONTEXT_DIR}/{scope}"),
            scope,
            file_names,
        })
    }

    fn bind_session(&mut self, scope: &str, session_id: &str) -> Result<(), String> {
        let turn_id = self
            .by_scope
            .get(scope)
            .ok_or_else(|| "MiniApp agent context reservation expired".to_string())?
            .turn_id
            .clone();
        if self.by_scope.iter().any(|(candidate_scope, snapshot)| {
            candidate_scope != scope
                && snapshot.session_id.as_deref() == Some(session_id)
                && snapshot.turn_id == turn_id
        }) {
            return Err(
                "This MiniApp agent turn already has an active context snapshot".to_string(),
            );
        }
        self.by_scope
            .get_mut(scope)
            .expect("scope existence checked above")
            .session_id = Some(session_id.to_string());
        Ok(())
    }

    fn remove_scope(&mut self, scope: &str) -> bool {
        let Some(snapshot) = self.by_scope.remove(scope) else {
            return false;
        };
        self.total_bytes = self.total_bytes.saturating_sub(snapshot.total_bytes);
        true
    }

    fn remove_turn(&mut self, session_id: &str, turn_id: &str) -> bool {
        let scope = self.by_scope.iter().find_map(|(scope, snapshot)| {
            (snapshot.session_id.as_deref() == Some(session_id) && snapshot.turn_id == turn_id)
                .then(|| scope.clone())
        });
        scope.is_some_and(|scope| self.remove_scope(&scope))
    }
}

/// RAII reservation created before a hidden session is mutated or created.
/// Unless retained after successful scheduler submission, dropping it returns
/// both the per-app and Runtime-wide capacity immediately.
pub struct MiniAppAgentContextLease {
    snapshot: MiniAppAgentContextSnapshot,
    retained: bool,
}

impl MiniAppAgentContextLease {
    pub fn snapshot(&self) -> &MiniAppAgentContextSnapshot {
        &self.snapshot
    }

    pub fn bind_session(&self, session_id: &str) -> Result<(), String> {
        registry()
            .write()
            .map_err(|_| "MiniApp agent context registry is unavailable".to_string())?
            .bind_session(&self.snapshot.scope, session_id)
    }

    pub fn retain(mut self) {
        self.retained = true;
    }
}

impl Drop for MiniAppAgentContextLease {
    fn drop(&mut self) {
        if self.retained {
            return;
        }
        if let Ok(mut registry) = registry().write() {
            registry.remove_scope(&self.snapshot.scope);
        }
    }
}

static AGENT_CONTEXT_REGISTRY: OnceLock<RwLock<MiniAppAgentContextRegistry>> = OnceLock::new();

fn registry() -> &'static RwLock<MiniAppAgentContextRegistry> {
    AGENT_CONTEXT_REGISTRY.get_or_init(|| RwLock::new(MiniAppAgentContextRegistry::default()))
}

fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && Path::new(name).components().count() == 1
}

fn validate_files(files: &[MiniAppAgentContextInput]) -> Result<usize, String> {
    if files.len() > MAX_MINIAPP_AGENT_CONTEXT_FILES {
        return Err(format!(
            "contextFiles supports at most {} files",
            MAX_MINIAPP_AGENT_CONTEXT_FILES
        ));
    }

    let mut total_bytes = 0usize;
    let mut normalized_names = std::collections::HashSet::with_capacity(files.len());
    for file in files {
        if !is_safe_file_name(&file.name) {
            return Err(format!(
                "Invalid context file name '{}': use one plain file name",
                file.name
            ));
        }
        if !normalized_names.insert(file.name.to_ascii_lowercase()) {
            return Err(format!("Duplicate context file name: {}", file.name));
        }
        let file_bytes = file.content.len();
        if file_bytes > MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES {
            return Err(format!(
                "Context file '{}' exceeds the {} byte limit",
                file.name, MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES
            ));
        }
        total_bytes = total_bytes
            .checked_add(file_bytes)
            .ok_or_else(|| "contextFiles total size overflowed".to_string())?;
        if total_bytes > MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES {
            return Err(format!(
                "contextFiles exceeds the {} byte total limit",
                MAX_MINIAPP_AGENT_CONTEXT_TOTAL_BYTES
            ));
        }
    }
    Ok(total_bytes)
}

pub fn validate_agent_context_files(files: &[MiniAppAgentContextInput]) -> Result<(), String> {
    validate_files(files).map(|_| ())
}

/// Atomically reserve capacity and publish an immutable snapshot before any
/// persistent hidden-session mutation. The lease rolls back on every error
/// path until the caller retains it after scheduler admission.
pub fn reserve_agent_context_snapshot(
    app_id: &str,
    turn_id: &str,
    files: Vec<MiniAppAgentContextInput>,
) -> Result<Option<MiniAppAgentContextLease>, String> {
    if files.is_empty() {
        return Ok(None);
    }
    let total_bytes = validate_files(&files)?;
    let files = Arc::new(
        files
            .into_iter()
            .map(|file| (file.name, Arc::<str>::from(file.content)))
            .collect::<BTreeMap<_, _>>(),
    );
    let snapshot = registry()
        .write()
        .map_err(|_| "MiniApp agent context registry is unavailable".to_string())?
        .reserve(app_id, turn_id, files, total_bytes)?;
    Ok(Some(MiniAppAgentContextLease {
        snapshot,
        retained: false,
    }))
}

/// Publish one immutable context snapshot for an admitted Agent turn.
///
/// The per-app limit applies across every MiniApp workspace. Active snapshots
/// are never evicted; callers receive backpressure until a terminal turn event
/// releases an existing lease.
pub fn publish_agent_context_snapshot(
    app_id: &str,
    session_id: &str,
    turn_id: &str,
    files: Vec<MiniAppAgentContextInput>,
) -> Result<Option<MiniAppAgentContextSnapshot>, String> {
    let Some(lease) = reserve_agent_context_snapshot(app_id, turn_id, files)? else {
        return Ok(None);
    };
    lease.bind_session(session_id)?;
    let snapshot = lease.snapshot().clone();
    lease.retain();
    Ok(Some(snapshot))
}

/// Release the snapshot for one terminal, cancelled, or failed turn.
pub fn remove_agent_context_snapshot(session_id: &str, turn_id: &str) -> bool {
    let Ok(mut registry) = registry().write() else {
        return false;
    };
    registry.remove_turn(session_id, turn_id)
}

pub fn agent_context_file(scope: &str, file_name: &str) -> Option<Arc<str>> {
    registry()
        .read()
        .ok()?
        .by_scope
        .get(scope)?
        .files
        .get(file_name)
        .cloned()
}

pub fn agent_context_files(scope: &str) -> Option<Arc<BTreeMap<String, Arc<str>>>> {
    registry()
        .read()
        .ok()?
        .by_scope
        .get(scope)
        .map(|snapshot| snapshot.files.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, content: &str) -> MiniAppAgentContextInput {
        MiniAppAgentContextInput {
            name: name.to_string(),
            content: content.to_string(),
        }
    }

    fn stored_files(name: &str, content: &str) -> Arc<BTreeMap<String, Arc<str>>> {
        Arc::new(BTreeMap::from([(
            name.to_string(),
            Arc::<str>::from(content),
        )]))
    }

    #[test]
    fn snapshots_are_immutable_and_released_by_turn_identity() {
        let snapshot = publish_agent_context_snapshot(
            "immutable-app",
            "immutable-session",
            "immutable-turn",
            vec![input("market.json", "{\"value\":1}")],
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            agent_context_file(&snapshot.scope, "market.json").as_deref(),
            Some("{\"value\":1}")
        );
        assert!(!remove_agent_context_snapshot(
            "immutable-session",
            "other-turn"
        ));
        assert!(remove_agent_context_snapshot(
            "immutable-session",
            "immutable-turn"
        ));
        assert!(agent_context_file(&snapshot.scope, "market.json").is_none());
    }

    #[test]
    fn active_limit_is_per_app_and_never_evicts_live_snapshots() {
        let mut scopes = Vec::new();
        for index in 0..MAX_MINIAPP_AGENT_CONTEXT_SCOPES_PER_APP {
            scopes.push(
                publish_agent_context_snapshot(
                    "quota-app",
                    &format!("quota-session-{index}"),
                    &format!("quota-turn-{index}"),
                    vec![input("context.json", &index.to_string())],
                )
                .unwrap()
                .unwrap(),
            );
        }
        let error = publish_agent_context_snapshot(
            "quota-app",
            "quota-session-overflow",
            "quota-turn-overflow",
            vec![input("context.json", "overflow")],
        )
        .unwrap_err();
        assert!(error.contains("active context snapshots"));
        for snapshot in &scopes {
            assert!(agent_context_file(&snapshot.scope, "context.json").is_some());
        }

        publish_agent_context_snapshot(
            "quota-other-app",
            "quota-other-session",
            "quota-other-turn",
            vec![input("context.json", "independent")],
        )
        .expect("a different app has its own quota");
        for index in 0..MAX_MINIAPP_AGENT_CONTEXT_SCOPES_PER_APP {
            assert!(remove_agent_context_snapshot(
                &format!("quota-session-{index}"),
                &format!("quota-turn-{index}")
            ));
        }
        assert!(remove_agent_context_snapshot(
            "quota-other-session",
            "quota-other-turn"
        ));
    }

    #[test]
    fn dropped_reservation_returns_capacity_before_session_creation() {
        let lease = reserve_agent_context_snapshot(
            "lease-app",
            "lease-turn",
            vec![input("context.json", "reserved")],
        )
        .unwrap()
        .unwrap();
        let scope = lease.snapshot().scope.clone();
        assert!(agent_context_file(&scope, "context.json").is_some());
        drop(lease);
        assert!(agent_context_file(&scope, "context.json").is_none());
    }

    #[test]
    fn registry_enforces_runtime_wide_scope_and_byte_budgets() {
        let mut registry = MiniAppAgentContextRegistry::default();
        let mut scopes = Vec::new();
        for index in 0..MAX_MINIAPP_AGENT_CONTEXT_SCOPES_GLOBAL {
            scopes.push(
                registry
                    .reserve(
                        &format!("app-{index}"),
                        &format!("turn-{index}"),
                        stored_files("context.json", "x"),
                        1,
                    )
                    .expect("distinct apps should share bounded Runtime capacity")
                    .scope,
            );
        }
        let scope_error = registry
            .reserve(
                "overflow-app",
                "overflow-turn",
                stored_files("context.json", "x"),
                1,
            )
            .unwrap_err();
        assert!(scope_error.contains("Agent Runtime"));
        for scope in scopes {
            assert!(registry.remove_scope(&scope));
        }
        assert_eq!(registry.total_bytes, 0);

        registry.total_bytes = MAX_MINIAPP_AGENT_CONTEXT_BYTES_GLOBAL - 1;
        let byte_error = registry
            .reserve(
                "byte-app",
                "byte-turn",
                stored_files("context.json", "xx"),
                2,
            )
            .unwrap_err();
        assert!(byte_error.contains("Runtime limit"));
    }

    #[test]
    fn file_validation_preserves_count_name_and_byte_bounds() {
        let duplicate = publish_agent_context_snapshot(
            "app",
            "session",
            "turn",
            vec![input("Summary.json", "{}"), input("summary.json", "{}")],
        )
        .unwrap_err();
        assert!(duplicate.contains("Duplicate"));

        let escaped = publish_agent_context_snapshot(
            "app",
            "session",
            "turn",
            vec![input("../summary.json", "{}")],
        )
        .unwrap_err();
        assert!(escaped.contains("Invalid context file name"));

        validate_agent_context_files(&[input(
            &"a".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES),
            "x",
        )])
        .expect("exact file-name byte limit is valid");
        assert!(validate_agent_context_files(&[input(
            &"a".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_NAME_BYTES + 1),
            "x",
        )])
        .unwrap_err()
        .contains("Invalid context file name"));

        let oversized = publish_agent_context_snapshot(
            "app",
            "session",
            "turn",
            vec![input(
                "summary.json",
                &"x".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES + 1),
            )],
        )
        .unwrap_err();
        assert!(oversized.contains("byte limit"));

        let maximum_count = (0..MAX_MINIAPP_AGENT_CONTEXT_FILES)
            .map(|index| input(&format!("context-{index}.json"), "x"))
            .collect::<Vec<_>>();
        validate_agent_context_files(&maximum_count).expect("maximum file count is valid");
        let over_count = (0..=MAX_MINIAPP_AGENT_CONTEXT_FILES)
            .map(|index| input(&format!("context-{index}.json"), "x"))
            .collect::<Vec<_>>();
        assert!(validate_agent_context_files(&over_count)
            .unwrap_err()
            .contains("at most"));

        let exact_total = vec![
            input(
                "first.bin",
                &"x".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES),
            ),
            input(
                "second.bin",
                &"y".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES),
            ),
        ];
        validate_agent_context_files(&exact_total).expect("exact total byte limit is valid");
        let over_total = vec![
            input(
                "first.bin",
                &"x".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES),
            ),
            input(
                "second.bin",
                &"y".repeat(MAX_MINIAPP_AGENT_CONTEXT_FILE_BYTES),
            ),
            input("extra.bin", "z"),
        ];
        assert!(validate_agent_context_files(&over_total)
            .unwrap_err()
            .contains("total limit"));
    }
}
