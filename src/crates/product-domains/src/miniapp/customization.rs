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
    pub detected_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniAppCustomizationMetadata {
    pub origin: MiniAppCustomizationOrigin,
    pub local_override: bool,
    pub last_applied_draft_id: Option<String>,
    pub available_builtin_update: Option<MiniAppAvailableBuiltinUpdate>,
    pub updated_at: i64,
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
    use crate::miniapp::types::{
        AiPermissions, FsPermissions, MiniAppPermissions, NetPermissions, NodePermissions,
        ShellPermissions,
    };

    fn empty_permissions() -> MiniAppPermissions {
        MiniAppPermissions::default()
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
