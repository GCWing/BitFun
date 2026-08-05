//! SDLC Lifecycle Events — Minimal Event Registry (P-1)
//!
//! These event types make fast-path, security prompts, tool/verification
//! summaries, and task results traceable without depending on the full
//! evidence graph or artifact-graph infrastructure.
//!
//! See `docs/sdlc-harness/implementation-plan.md` (P-1 deliverable:
//! 最小事件注册表) for the design specification.

use serde::{Deserialize, Serialize};

/// Minimal SDLC lifecycle event for traceability of development tasks.
///
/// Each variant serializes with a dot-notation `type` tag (e.g.
/// `"project.profiled.light"`) so external consumers and evidence
/// pipelines can route events by stable string identifiers without
/// matching on Rust enum variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SdlcLifecycleEvent {
    /// Lightweight project profiling result.
    ///
    /// Emitted after a fast scan of the workspace identifying languages,
    /// package managers, common scripts, and entry points.
    #[serde(rename = "project.profiled.light")]
    ProjectProfiledLight {
        workspace_path: String,
        /// Detected programming languages (e.g. `["rust", "typescript"]`).
        languages: Vec<String>,
        /// Detected package managers (e.g. `["pnpm", "cargo"]`).
        package_managers: Vec<String>,
        /// Known entry-point files or commands (e.g. `["pnpm run desktop:dev"]`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entry_points: Vec<String>,
    },

    /// A development task has started.
    #[serde(rename = "task.started")]
    TaskStarted {
        task_id: String,
        session_id: String,
        /// Task kind (e.g. `"issue_fix"`, `"feature"`, `"refactor"`).
        task_kind: String,
    },

    /// A development task has completed.
    #[serde(rename = "task.completed")]
    TaskCompleted {
        task_id: String,
        session_id: String,
        /// Outcome label (e.g. `"success"`, `"failure"`, `"cancelled"`).
        outcome: String,
        /// Elapsed wall-clock duration in milliseconds.
        duration_ms: u64,
    },

    /// A policy decision was made for a capability or scope.
    #[serde(rename = "policy.decided")]
    PolicyDecided {
        /// Decision label (e.g. `"allow"`, `"ask"`, `"deny"`).
        decision: String,
        /// Scope the decision applies to (e.g. `"workspace_write"`, `"shell"`).
        scope: String,
        /// Human-readable reason for the decision.
        reason: String,
    },

    /// A security boundary decision was made.
    #[serde(rename = "security.decided")]
    SecurityDecided {
        /// Decision label (e.g. `"allow"`, `"ask"`, `"deny"`, `"emergency_bypass"`).
        decision: String,
        /// Capability that was evaluated (e.g. `"shell"`, `"network"`, `"credential"`).
        capability: String,
        /// Resolved execution location (e.g. `"local"`, `"remote_ssh"`, `"container"`).
        execution_location: String,
        /// Sandbox isolation level if applicable.
        #[serde(skip_serializing_if = "Option::is_none")]
        sandbox_level: Option<String>,
        /// Reason if the sandbox was downgraded or unavailable.
        #[serde(skip_serializing_if = "Option::is_none")]
        downgrade_reason: Option<String>,
    },

    /// A tool execution completed.
    #[serde(rename = "tool.completed")]
    ToolCompleted {
        tool_name: String,
        session_id: String,
        success: bool,
        /// Elapsed wall-clock duration in milliseconds.
        duration_ms: u64,
    },

    /// A verification step completed.
    #[serde(rename = "verification.completed")]
    VerificationCompleted {
        /// Verification kind (e.g. `"build"`, `"test"`, `"lint"`, `"type_check"`).
        verification_kind: String,
        /// Target that was verified (e.g. package path or test filter).
        target: String,
        passed: bool,
        /// Short human-readable summary of the verification result.
        summary: String,
    },

    /// A confidence summary was generated for a task or change set.
    #[serde(rename = "confidence.summary.generated")]
    ConfidenceSummaryGenerated {
        task_id: String,
        /// Number of verification items that passed.
        verified_count: usize,
        /// Number of verification items that were not verified.
        unverified_count: usize,
        /// Number of verification items that were explicitly skipped.
        skipped_count: usize,
        /// Human-readable confidence summary.
        summary: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_profiled_light_serializes_with_dot_type() {
        let event = SdlcLifecycleEvent::ProjectProfiledLight {
            workspace_path: "/repo".into(),
            languages: vec!["rust".into()],
            package_managers: vec!["cargo".into()],
            entry_points: vec!["cargo build".into()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"project.profiled.light""#));
    }

    #[test]
    fn task_started_serializes_with_dot_type() {
        let event = SdlcLifecycleEvent::TaskStarted {
            task_id: "t1".into(),
            session_id: "s1".into(),
            task_kind: "issue_fix".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"task.started""#));
    }

    #[test]
    fn security_decided_round_trips() {
        let event = SdlcLifecycleEvent::SecurityDecided {
            decision: "ask".into(),
            capability: "network".into(),
            execution_location: "local".into(),
            sandbox_level: Some("none".into()),
            downgrade_reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: SdlcLifecycleEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            SdlcLifecycleEvent::SecurityDecided {
                decision,
                capability,
                execution_location,
                sandbox_level,
                downgrade_reason,
            } => {
                assert_eq!(decision, "ask");
                assert_eq!(capability, "network");
                assert_eq!(execution_location, "local");
                assert_eq!(sandbox_level.as_deref(), Some("none"));
                assert!(downgrade_reason.is_none());
            }
            _ => panic!("wrong variant after round-trip"),
        }
    }

    #[test]
    fn confidence_summary_omits_optional_fields_when_empty() {
        let event = SdlcLifecycleEvent::ConfidenceSummaryGenerated {
            task_id: "t1".into(),
            verified_count: 3,
            unverified_count: 1,
            skipped_count: 0,
            summary: "3 passed, 1 unverified".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"confidence.summary.generated""#));
        assert!(json.contains(r#""verified_count":3"#));
    }
}
