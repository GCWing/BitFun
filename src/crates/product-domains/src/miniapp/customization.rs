use crate::miniapp::types::MiniAppPermissions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppCustomizationOriginKind {
    Builtin,
    Imported,
    UserCreated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppCustomizationOrigin {
    pub kind: MiniAppCustomizationOriginKind,
    pub builtin_id: Option<String>,
    pub builtin_version: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppAvailableBuiltinUpdate {
    pub builtin_version: u32,
    #[serde(default)]
    pub source_hash: String,
    pub detected_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppDeclinedBuiltinUpdate {
    pub builtin_version: u32,
    pub source_hash: String,
    pub declined_at: i64,
    #[serde(default)]
    pub local_app_version: Option<u32>,
    #[serde(default)]
    pub local_app_updated_at: Option<i64>,
    #[serde(default)]
    pub last_applied_draft_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppCustomizationMetadata {
    pub origin: MiniAppCustomizationOrigin,
    pub local_override: bool,
    pub last_applied_draft_id: Option<String>,
    #[serde(default)]
    pub available_builtin_update: Option<MiniAppAvailableBuiltinUpdate>,
    #[serde(default)]
    pub declined_builtin_updates: Vec<MiniAppDeclinedBuiltinUpdate>,
    pub updated_at: i64,
}

pub const MAX_DECLINED_BUILTIN_UPDATES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiniAppCustomizationLocalSnapshot {
    pub version: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppBuiltinUpdateAvailabilityDecision {
    pub metadata: MiniAppCustomizationMetadata,
    pub should_surface_update: bool,
    pub metadata_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiniAppCustomizationBaseline {
    Builtin {
        builtin_id: String,
        builtin_version: u32,
    },
    UserCreated,
}

pub fn apply_draft_customization_metadata(
    existing: Option<MiniAppCustomizationMetadata>,
    baseline: MiniAppCustomizationBaseline,
    draft_id: &str,
    now: i64,
) -> MiniAppCustomizationMetadata {
    let mut metadata = existing.unwrap_or_else(|| match baseline.clone() {
        MiniAppCustomizationBaseline::Builtin {
            builtin_id,
            builtin_version,
        } => MiniAppCustomizationMetadata {
            origin: MiniAppCustomizationOrigin {
                kind: MiniAppCustomizationOriginKind::Builtin,
                builtin_id: Some(builtin_id),
                builtin_version: Some(builtin_version),
            },
            local_override: true,
            last_applied_draft_id: None,
            available_builtin_update: None,
            declined_builtin_updates: Vec::new(),
            updated_at: now,
        },
        MiniAppCustomizationBaseline::UserCreated => MiniAppCustomizationMetadata {
            origin: MiniAppCustomizationOrigin {
                kind: MiniAppCustomizationOriginKind::UserCreated,
                builtin_id: None,
                builtin_version: None,
            },
            local_override: false,
            last_applied_draft_id: None,
            available_builtin_update: None,
            declined_builtin_updates: Vec::new(),
            updated_at: now,
        },
    });

    if matches!(
        metadata.origin.kind,
        MiniAppCustomizationOriginKind::Builtin
    ) {
        metadata.local_override = true;
        if let MiniAppCustomizationBaseline::Builtin {
            builtin_version, ..
        } = baseline
        {
            metadata.origin.builtin_version = Some(builtin_version);
            metadata.available_builtin_update = None;
        }
    }

    metadata.last_applied_draft_id = Some(draft_id.to_string());
    metadata.updated_at = now;
    metadata
}

pub fn declined_builtin_update_needs_local_snapshot(
    metadata: &MiniAppCustomizationMetadata,
    source_hash: &str,
) -> bool {
    metadata
        .declined_builtin_updates
        .iter()
        .rev()
        .find(|record| record.source_hash == source_hash)
        .is_some_and(|record| {
            record.local_app_version.is_some() && record.local_app_updated_at.is_some()
        })
}

pub fn is_current_declined_builtin_update(
    metadata: &MiniAppCustomizationMetadata,
    source_hash: &str,
    local_snapshot: Option<MiniAppCustomizationLocalSnapshot>,
) -> bool {
    let Some(record) = metadata
        .declined_builtin_updates
        .iter()
        .rev()
        .find(|record| record.source_hash == source_hash)
    else {
        return false;
    };

    if let (Some(record_version), Some(record_updated_at), Some(snapshot)) = (
        record.local_app_version,
        record.local_app_updated_at,
        local_snapshot,
    ) {
        return snapshot.version == record_version && snapshot.updated_at == record_updated_at;
    }

    record.last_applied_draft_id == metadata.last_applied_draft_id
}

pub fn mark_builtin_update_available_metadata(
    mut metadata: MiniAppCustomizationMetadata,
    builtin_version: u32,
    source_hash: &str,
    detected_at: i64,
    declined_update_current: bool,
) -> MiniAppBuiltinUpdateAvailabilityDecision {
    if declined_update_current {
        let metadata_changed = metadata.available_builtin_update.take().is_some();
        return MiniAppBuiltinUpdateAvailabilityDecision {
            metadata,
            should_surface_update: false,
            metadata_changed,
        };
    }

    metadata.available_builtin_update = Some(MiniAppAvailableBuiltinUpdate {
        builtin_version,
        source_hash: source_hash.to_string(),
        detected_at,
    });
    metadata.updated_at = detected_at;
    MiniAppBuiltinUpdateAvailabilityDecision {
        metadata,
        should_surface_update: true,
        metadata_changed: true,
    }
}

pub fn decline_builtin_update_metadata(
    mut metadata: MiniAppCustomizationMetadata,
    builtin_version: u32,
    source_hash: &str,
    declined_at: i64,
    local_snapshot: Option<MiniAppCustomizationLocalSnapshot>,
) -> MiniAppCustomizationMetadata {
    let (local_app_version, local_app_updated_at) = local_snapshot
        .map(|snapshot| (Some(snapshot.version), Some(snapshot.updated_at)))
        .unwrap_or((None, None));
    let last_applied_draft_id = metadata.last_applied_draft_id.clone();

    if let Some(record) = metadata.declined_builtin_updates.iter_mut().find(|record| {
        record.builtin_version == builtin_version
            && record.source_hash == source_hash
            && record.last_applied_draft_id == last_applied_draft_id
    }) {
        record.declined_at = declined_at;
        record.local_app_version = local_app_version;
        record.local_app_updated_at = local_app_updated_at;
    } else {
        metadata
            .declined_builtin_updates
            .push(MiniAppDeclinedBuiltinUpdate {
                builtin_version,
                source_hash: source_hash.to_string(),
                declined_at,
                local_app_version,
                local_app_updated_at,
                last_applied_draft_id,
            });
        if metadata.declined_builtin_updates.len() > MAX_DECLINED_BUILTIN_UPDATES {
            let remove_count =
                metadata.declined_builtin_updates.len() - MAX_DECLINED_BUILTIN_UPDATES;
            metadata.declined_builtin_updates.drain(0..remove_count);
        }
    }

    metadata.available_builtin_update = None;
    metadata.updated_at = declined_at;
    metadata
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppPermissionDiff {
    pub high_risk: bool,
    pub added: Vec<String>,
    pub expanded: Vec<String>,
    pub removed: Vec<String>,
}

pub fn diff_permissions(
    active: &MiniAppPermissions,
    draft: &MiniAppPermissions,
) -> MiniAppPermissionDiff {
    let mut added = Vec::new();
    let mut expanded = Vec::new();
    let mut removed = Vec::new();

    diff_string_list(
        "fs.read",
        active.fs.as_ref().and_then(|fs| fs.read.as_ref()),
        draft.fs.as_ref().and_then(|fs| fs.read.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "fs.write",
        active.fs.as_ref().and_then(|fs| fs.write.as_ref()),
        draft.fs.as_ref().and_then(|fs| fs.write.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "shell.allow",
        active.shell.as_ref().and_then(|shell| shell.allow.as_ref()),
        draft.shell.as_ref().and_then(|shell| shell.allow.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );
    diff_string_list(
        "net.allow",
        active.net.as_ref().and_then(|net| net.allow.as_ref()),
        draft.net.as_ref().and_then(|net| net.allow.as_ref()),
        &mut added,
        &mut expanded,
        &mut removed,
    );

    diff_enabled_flag(
        "node.enabled",
        active.node.as_ref().map(|node| node.enabled),
        draft.node.as_ref().map(|node| node.enabled),
        &mut added,
        &mut removed,
    );
    diff_enabled_flag(
        "ai.enabled",
        active.ai.as_ref().map(|ai| ai.enabled),
        draft.ai.as_ref().map(|ai| ai.enabled),
        &mut added,
        &mut removed,
    );

    let high_risk = added
        .iter()
        .chain(expanded.iter())
        .any(|item| is_high_risk_permission_change(item));

    MiniAppPermissionDiff {
        high_risk,
        added,
        expanded,
        removed,
    }
}

pub fn is_high_risk_permission_change(item: &str) -> bool {
    item.starts_with("fs.read:")
        || item.starts_with("fs.write:")
        || item.starts_with("shell.allow:")
        || item.starts_with("net.allow:")
        || item == "node.enabled"
        || item == "ai.enabled"
}

fn diff_enabled_flag(
    label: &str,
    active: Option<bool>,
    draft: Option<bool>,
    added: &mut Vec<String>,
    removed: &mut Vec<String>,
) {
    let active_enabled = active.unwrap_or(false);
    let draft_enabled = draft.unwrap_or(false);
    match (active_enabled, draft_enabled) {
        (false, true) => added.push(label.to_string()),
        (true, false) => removed.push(label.to_string()),
        _ => {}
    }
}

fn diff_string_list(
    label: &str,
    active: Option<&Vec<String>>,
    draft: Option<&Vec<String>>,
    added: &mut Vec<String>,
    expanded: &mut Vec<String>,
    removed: &mut Vec<String>,
) {
    let active = active.cloned().unwrap_or_default();
    let draft = draft.cloned().unwrap_or_default();

    for value in &draft {
        if !active.contains(value) {
            if active.is_empty() {
                added.push(format!("{label}:{value}"));
            } else {
                expanded.push(format!("{label}:{value}"));
            }
        }
    }

    for value in &active {
        if !draft.contains(value) {
            removed.push(format!("{label}:{value}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::miniapp::customization::MiniAppCustomizationMetadata;
    use crate::miniapp::types::{
        AiPermissions, FsPermissions, MiniAppPermissions, NetPermissions, NodePermissions,
        ShellPermissions,
    };

    fn empty_permissions() -> MiniAppPermissions {
        MiniAppPermissions::default()
    }

    #[test]
    fn customization_metadata_defaults_declined_updates_for_existing_files() {
        let metadata: MiniAppCustomizationMetadata = serde_json::from_value(serde_json::json!({
            "origin": {
                "kind": "builtin",
                "builtin_id": "builtin-demo",
                "builtin_version": 1
            },
            "local_override": true,
            "last_applied_draft_id": "draft-1",
            "available_builtin_update": {
                "builtin_version": 2,
                "detected_at": 123
            },
            "updated_at": 124
        }))
        .unwrap();

        assert!(metadata.declined_builtin_updates.is_empty());
        assert_eq!(metadata.available_builtin_update.unwrap().source_hash, "");
    }

    #[test]
    fn permission_diff_marks_fs_write_addition_high_risk() {
        let active = empty_permissions();
        let mut draft = empty_permissions();
        draft.fs = Some(FsPermissions {
            read: None,
            write: Some(vec!["{workspace}".to_string()]),
        });

        let diff = super::diff_permissions(&active, &draft);

        assert!(diff.high_risk);
        assert_eq!(diff.added, vec!["fs.write:{workspace}".to_string()]);
        assert!(diff.expanded.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn permission_diff_marks_shell_and_net_expansions_high_risk() {
        let mut active = empty_permissions();
        active.shell = Some(ShellPermissions {
            allow: Some(vec!["git".to_string()]),
        });
        active.net = Some(NetPermissions {
            allow: Some(vec!["api.example.com".to_string()]),
        });

        let mut draft = empty_permissions();
        draft.shell = Some(ShellPermissions {
            allow: Some(vec!["git".to_string(), "node".to_string()]),
        });
        draft.net = Some(NetPermissions {
            allow: Some(vec!["api.example.com".to_string(), "*".to_string()]),
        });

        let diff = super::diff_permissions(&active, &draft);

        assert!(diff.high_risk);
        assert!(diff.expanded.contains(&"shell.allow:node".to_string()));
        assert!(diff.expanded.contains(&"net.allow:*".to_string()));
    }

    #[test]
    fn permission_diff_marks_node_and_ai_enablement_high_risk() {
        let active = empty_permissions();
        let mut draft = empty_permissions();
        draft.node = Some(NodePermissions {
            enabled: true,
            max_memory_mb: None,
            timeout_ms: None,
        });
        draft.ai = Some(AiPermissions {
            enabled: true,
            allowed_models: None,
            max_tokens_per_request: None,
            rate_limit_per_minute: None,
        });

        let diff = super::diff_permissions(&active, &draft);

        assert!(diff.high_risk);
        assert!(diff.added.contains(&"node.enabled".to_string()));
        assert!(diff.added.contains(&"ai.enabled".to_string()));
    }

    #[test]
    fn permission_diff_tracks_removed_permissions_without_high_risk() {
        let mut active = empty_permissions();
        active.fs = Some(FsPermissions {
            read: Some(vec!["{workspace}".to_string()]),
            write: None,
        });
        let draft = empty_permissions();

        let diff = super::diff_permissions(&active, &draft);

        assert!(!diff.high_risk);
        assert_eq!(diff.removed, vec!["fs.read:{workspace}".to_string()]);
    }
}
