//! Pure domain contracts and evaluation rules for tool-call permissions.
//!
//! This module intentionally has no runtime, persistence, or interaction
//! responsibilities. Product assembly and execution owners may consume these
//! decisions in later integration phases.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The effect produced by a matching permission rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionEffect {
    Allow,
    Ask,
    Deny,
}

/// An ordered action/resource permission rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,
    pub resource: String,
    pub effect: PermissionEffect,
}

impl PermissionRule {
    pub fn new(
        action: impl Into<String>,
        resource: impl Into<String>,
        effect: PermissionEffect,
    ) -> Self {
        Self {
            action: action.into(),
            resource: resource.into(),
            effect,
        }
    }
}

/// A rule list whose order is significant: later matching rules win.
pub type PermissionRuleset = Vec<PermissionRule>;

/// Identifies the boundary that originated a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequestSourceKind {
    ToolCall,
    Provider,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestSource {
    pub kind: PermissionRequestSourceKind,
    pub identity: String,
}

/// A process-local permission request projected to an interactive surface.
///
/// Resource and display values stored here must already be safe for user
/// presentation and audit persistence. Raw secrets and unrestricted command
/// payloads must remain outside this DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub request_id: String,
    pub project_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub action: String,
    pub resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub save_resources: Vec<String>,
    pub source: PermissionRequestSource,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub display_metadata: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum PermissionReply {
    Once,
    Always,
    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionReplySource {
    User,
    AutoApprove,
    System,
}

/// A remembered allow scoped by project, action, and resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGrant {
    pub project_id: String,
    pub action: String,
    pub resource: String,
    pub created_at_ms: i64,
}

impl PermissionGrant {
    pub fn key(&self) -> PermissionGrantKey {
        PermissionGrantKey {
            project_id: self.project_id.clone(),
            action: self.action.clone(),
            resource: self.resource.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGrantKey {
    pub project_id: String,
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PermissionAuditEvent {
    Requested,
    Replied {
        reply: PermissionReply,
        source: PermissionReplySource,
    },
    Cancelled {
        reason: String,
    },
}

/// An append-only audit fact containing only presentation-safe request data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAuditRecord {
    pub audit_id: String,
    pub request: PermissionRequest,
    pub event: PermissionAuditEvent,
    pub timestamp_ms: i64,
}

/// Controls resource matching for local or remote workspace path semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResourceCaseSensitivity {
    Sensitive,
    Insensitive,
}

/// Pure evaluator for ordered tool permission rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionEvaluator {
    resource_case_sensitivity: PermissionResourceCaseSensitivity,
}

impl PermissionEvaluator {
    pub const fn new(resource_case_sensitivity: PermissionResourceCaseSensitivity) -> Self {
        Self {
            resource_case_sensitivity,
        }
    }

    pub const fn case_sensitive() -> Self {
        Self::new(PermissionResourceCaseSensitivity::Sensitive)
    }

    pub const fn windows_compatible() -> Self {
        Self::new(PermissionResourceCaseSensitivity::Insensitive)
    }

    pub const fn for_current_platform() -> Self {
        if cfg!(windows) {
            Self::windows_compatible()
        } else {
            Self::case_sensitive()
        }
    }

    /// Returns the effect of the last rule matching both action and resource.
    /// Unmatched requests default to `ask`.
    pub fn evaluate_resource(
        &self,
        action: &str,
        resource: &str,
        rules: &[PermissionRule],
    ) -> PermissionEffect {
        rules
            .iter()
            .rev()
            .find(|rule| {
                wildcard_matches(
                    action,
                    &rule.action,
                    PermissionResourceCaseSensitivity::Sensitive,
                ) && wildcard_matches(resource, &rule.resource, self.resource_case_sensitivity)
            })
            .map(|rule| rule.effect)
            .unwrap_or(PermissionEffect::Ask)
    }

    /// Evaluates every resource in one tool call atomically.
    ///
    /// Any denied resource denies the call. Otherwise any resource that still
    /// requires confirmation makes the call ask. Only an all-allow result is
    /// allowed. A request without resources fails closed as `ask`.
    pub fn evaluate_resources(
        &self,
        action: &str,
        resources: &[String],
        rules: &[PermissionRule],
    ) -> PermissionEffect {
        if resources.is_empty() {
            return PermissionEffect::Ask;
        }

        let mut aggregate = PermissionEffect::Allow;
        for resource in resources {
            match self.evaluate_resource(action, resource, rules) {
                PermissionEffect::Deny => return PermissionEffect::Deny,
                PermissionEffect::Ask => aggregate = PermissionEffect::Ask,
                PermissionEffect::Allow => {}
            }
        }
        aggregate
    }
}

impl Default for PermissionEvaluator {
    fn default() -> Self {
        Self::for_current_platform()
    }
}

/// Merges global, project, and agent rule layers without changing their order.
pub fn merge_permission_rule_layers(layers: &[&[PermissionRule]]) -> PermissionRuleset {
    let capacity = layers.iter().map(|layer| layer.len()).sum();
    let mut merged = Vec::with_capacity(capacity);
    for layer in layers {
        merged.extend_from_slice(layer);
    }
    merged
}

/// Matches `*` and `?` wildcards after normalizing path separators.
///
/// Like the OpenCode V2 reference, a pattern ending in ` *` also matches the
/// prefix without a trailing argument (for example, `git *` matches `git`).
pub fn wildcard_matches(
    input: &str,
    pattern: &str,
    case_sensitivity: PermissionResourceCaseSensitivity,
) -> bool {
    let input = normalize_wildcard_value(input, case_sensitivity);
    let pattern = normalize_wildcard_value(pattern, case_sensitivity);

    if pattern
        .strip_suffix(" *")
        .is_some_and(|prefix| input == prefix)
    {
        return true;
    }

    glob_matches(&input, &pattern)
}

fn normalize_wildcard_value(
    value: &str,
    case_sensitivity: PermissionResourceCaseSensitivity,
) -> String {
    let normalized = value.replace('\\', "/");
    match case_sensitivity {
        PermissionResourceCaseSensitivity::Sensitive => normalized,
        PermissionResourceCaseSensitivity::Insensitive => normalized.to_lowercase(),
    }
}

fn glob_matches(input: &str, pattern: &str) -> bool {
    let input: Vec<char> = input.chars().collect();
    let mut previous = vec![false; input.len() + 1];
    previous[0] = true;

    for pattern_char in pattern.chars() {
        let mut current = vec![false; input.len() + 1];
        if pattern_char == '*' {
            current[0] = previous[0];
        }

        for (index, input_char) in input.iter().enumerate() {
            current[index + 1] = match pattern_char {
                '*' => previous[index + 1] || current[index],
                '?' => previous[index],
                literal => previous[index] && literal == *input_char,
            };
        }
        previous = current;
    }

    previous[input.len()]
}
