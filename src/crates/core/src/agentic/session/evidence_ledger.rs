use crate::agentic::core::{CompressionContract, CompressionContractItem};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PARTIAL_OUTPUT_BYTES: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceLedgerTargetKind {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "subagent")]
    Subagent,
    #[serde(rename = "artifact")]
    Artifact,
    #[serde(rename = "checkpoint")]
    Checkpoint,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceLedgerEventStatus {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "partial_timeout")]
    PartialTimeout,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLedgerCheckpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
    pub dirty_state_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLedgerEvent {
    pub event_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub target_kind: EvidenceLedgerTargetKind,
    pub target: String,
    pub status: EvidenceLedgerEventStatus,
    pub exit_code_or_error_kind: Option<String>,
    pub touched_files: Vec<String>,
    pub artifact_path: Option<String>,
    pub summary: String,
    pub partial_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<EvidenceLedgerCheckpoint>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLedgerSummaryItem {
    pub event_id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub target_kind: EvidenceLedgerTargetKind,
    pub target: String,
    pub status: EvidenceLedgerEventStatus,
    pub summary: String,
    pub error_kind: Option<String>,
    pub partial_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<EvidenceLedgerCheckpoint>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLedgerSummary {
    pub touched_files: Vec<String>,
    pub latest_failed_commands: Vec<EvidenceLedgerSummaryItem>,
    pub latest_verification_commands: Vec<EvidenceLedgerSummaryItem>,
    pub partial_subagent_results: Vec<EvidenceLedgerSummaryItem>,
    pub latest_checkpoints: Vec<EvidenceLedgerSummaryItem>,
}

#[derive(Debug, Default)]
pub struct SessionEvidenceLedger {
    events_by_session: Arc<DashMap<String, Vec<EvidenceLedgerEvent>>>,
}

impl EvidenceLedgerEvent {
    pub fn new(
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
        tool_name: impl Into<String>,
        target_kind: EvidenceLedgerTargetKind,
        target: impl Into<String>,
        status: EvidenceLedgerEventStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            turn_id: turn_id.into(),
            tool_name: tool_name.into(),
            target_kind,
            target: target.into(),
            status,
            exit_code_or_error_kind: None,
            touched_files: Vec::new(),
            artifact_path: None,
            summary: summary.into(),
            partial_output: None,
            checkpoint: None,
            created_at_ms: current_time_millis(),
        }
    }

    pub fn checkpoint_created(
        session_id: impl Into<String>,
        turn_id: impl Into<String>,
        tool_name: impl Into<String>,
        target: impl Into<String>,
        checkpoint: EvidenceLedgerCheckpoint,
    ) -> Self {
        let target = target.into();
        Self::new(
            session_id,
            turn_id,
            tool_name,
            EvidenceLedgerTargetKind::Checkpoint,
            target.clone(),
            EvidenceLedgerEventStatus::Created,
            format!("Checkpoint created before modifying {}.", target),
        )
        .with_touched_files(checkpoint.touched_files.clone())
        .with_checkpoint(checkpoint)
    }

    pub fn with_error_kind(mut self, error_kind: impl Into<String>) -> Self {
        self.exit_code_or_error_kind = Some(error_kind.into());
        self
    }

    pub fn with_partial_output(mut self, partial_output: impl Into<String>) -> Self {
        let partial_output = partial_output.into();
        self.partial_output = Some(truncate_string_at_char_boundary(
            &partial_output,
            MAX_PARTIAL_OUTPUT_BYTES,
        ));
        self
    }

    pub fn with_touched_files(mut self, touched_files: Vec<String>) -> Self {
        self.touched_files = touched_files;
        self
    }

    pub fn with_artifact_path(mut self, artifact_path: impl Into<String>) -> Self {
        self.artifact_path = Some(artifact_path.into());
        self
    }

    pub fn with_checkpoint(mut self, checkpoint: EvidenceLedgerCheckpoint) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }
}

impl SessionEvidenceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&self, event: EvidenceLedgerEvent) -> EvidenceLedgerEvent {
        self.events_by_session
            .entry(event.session_id.clone())
            .or_default()
            .push(event.clone());
        event
    }

    pub fn events_for_turn(&self, session_id: &str, turn_id: &str) -> Vec<EvidenceLedgerEvent> {
        self.events_by_session
            .get(session_id)
            .map(|events| {
                events
                    .iter()
                    .filter(|event| event.turn_id == turn_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn summary_for_session(&self, session_id: &str, limit: usize) -> EvidenceLedgerSummary {
        let Some(events) = self.events_by_session.get(session_id) else {
            return EvidenceLedgerSummary::default();
        };

        let mut touched_files = Vec::new();
        let mut latest_failed_commands = Vec::new();
        let mut latest_verification_commands = Vec::new();
        let mut partial_subagent_results = Vec::new();
        let mut latest_checkpoints = Vec::new();

        for event in events.iter().rev() {
            for file in &event.touched_files {
                if !touched_files.contains(file) {
                    touched_files.push(file.clone());
                }
            }

            if event.target_kind == EvidenceLedgerTargetKind::Command
                && event.status == EvidenceLedgerEventStatus::Failed
                && latest_failed_commands.len() < limit
            {
                latest_failed_commands.push(event.into());
            }

            if event.target_kind == EvidenceLedgerTargetKind::Command
                && is_verification_command(&event.target)
                && latest_verification_commands.len() < limit
            {
                latest_verification_commands.push(event.into());
            }

            if event.target_kind == EvidenceLedgerTargetKind::Subagent
                && event.status == EvidenceLedgerEventStatus::PartialTimeout
                && partial_subagent_results.len() < limit
            {
                partial_subagent_results.push(event.into());
            }

            if event.target_kind == EvidenceLedgerTargetKind::Checkpoint
                && event.status == EvidenceLedgerEventStatus::Created
                && latest_checkpoints.len() < limit
            {
                latest_checkpoints.push(event.into());
            }
        }

        touched_files.truncate(limit);

        EvidenceLedgerSummary {
            touched_files,
            latest_failed_commands,
            latest_verification_commands,
            partial_subagent_results,
            latest_checkpoints,
        }
    }
}

impl From<&EvidenceLedgerEvent> for EvidenceLedgerSummaryItem {
    fn from(event: &EvidenceLedgerEvent) -> Self {
        Self {
            event_id: event.event_id.clone(),
            turn_id: event.turn_id.clone(),
            tool_name: event.tool_name.clone(),
            target_kind: event.target_kind.clone(),
            target: event.target.clone(),
            status: event.status.clone(),
            summary: event.summary.clone(),
            error_kind: event.exit_code_or_error_kind.clone(),
            partial_output: event.partial_output.clone(),
            checkpoint: event.checkpoint.clone(),
        }
    }
}

impl From<EvidenceLedgerSummary> for CompressionContract {
    fn from(summary: EvidenceLedgerSummary) -> Self {
        Self {
            touched_files: summary.touched_files,
            verification_commands: summary
                .latest_verification_commands
                .into_iter()
                .map(compression_contract_item_from_summary_item)
                .collect(),
            blocking_failures: summary
                .latest_failed_commands
                .into_iter()
                .map(compression_contract_item_from_summary_item)
                .collect(),
            subagent_statuses: summary
                .partial_subagent_results
                .into_iter()
                .map(compression_contract_item_from_summary_item)
                .collect(),
        }
    }
}

fn compression_contract_item_from_summary_item(
    item: EvidenceLedgerSummaryItem,
) -> CompressionContractItem {
    CompressionContractItem {
        target: item.target,
        status: event_status_label(&item.status).to_string(),
        summary: item.summary,
        error_kind: item.error_kind,
    }
}

fn event_status_label(status: &EvidenceLedgerEventStatus) -> &'static str {
    match status {
        EvidenceLedgerEventStatus::Created => "created",
        EvidenceLedgerEventStatus::Succeeded => "succeeded",
        EvidenceLedgerEventStatus::Failed => "failed",
        EvidenceLedgerEventStatus::PartialTimeout => "partial_timeout",
        EvidenceLedgerEventStatus::Cancelled => "cancelled",
        EvidenceLedgerEventStatus::Unknown => "unknown",
    }
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn is_verification_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    command.contains(" test")
        || command.starts_with("test")
        || command.contains("cargo test")
        || command.contains("pnpm")
        || command.contains("npm test")
        || command.contains("yarn test")
        || command.contains("vitest")
        || command.contains("type-check")
        || command.contains("lint")
}

fn truncate_string_at_char_boundary(value: &str, max_bytes: usize) -> String {
    crate::util::truncate_at_char_boundary(value, max_bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        EvidenceLedgerCheckpoint, EvidenceLedgerEvent, EvidenceLedgerEventStatus,
        EvidenceLedgerTargetKind, SessionEvidenceLedger,
    };

    #[test]
    fn ledger_reads_events_scoped_by_session_and_turn() {
        let ledger = SessionEvidenceLedger::new();
        let event = EvidenceLedgerEvent::new(
            "session-a",
            "turn-a",
            "Task",
            EvidenceLedgerTargetKind::Subagent,
            "ReviewSecurity",
            EvidenceLedgerEventStatus::PartialTimeout,
            "Security reviewer timed out after partial output.",
        )
        .with_error_kind("timeout")
        .with_partial_output("Found token logging before timeout.");

        let appended = ledger.append(event);

        assert!(!appended.event_id.is_empty());
        assert_eq!(
            ledger.events_for_turn("session-a", "turn-a"),
            vec![appended.clone()]
        );
        assert!(ledger.events_for_turn("session-a", "other-turn").is_empty());
        assert!(ledger.events_for_turn("other-session", "turn-a").is_empty());
    }

    #[test]
    fn checkpoint_created_event_preserves_recovery_boundary_metadata() {
        let checkpoint = EvidenceLedgerCheckpoint {
            current_branch: Some("feature/context".to_string()),
            dirty_state_summary: "staged=1, unstaged=2, untracked=3".to_string(),
            touched_files: vec!["src/lib.rs".to_string()],
            diff_hash: Some("abc123".to_string()),
        };

        let event = EvidenceLedgerEvent::checkpoint_created(
            "session-a",
            "turn-a",
            "Edit",
            "src/lib.rs",
            checkpoint.clone(),
        );

        assert_eq!(event.target_kind, EvidenceLedgerTargetKind::Checkpoint);
        assert_eq!(event.status, EvidenceLedgerEventStatus::Created);
        assert_eq!(event.touched_files, vec!["src/lib.rs"]);
        assert_eq!(event.checkpoint.as_ref(), Some(&checkpoint));
    }

    #[test]
    fn summary_projects_latest_checkpoints() {
        let ledger = SessionEvidenceLedger::new();
        ledger.append(EvidenceLedgerEvent::checkpoint_created(
            "session-a",
            "turn-a",
            "Delete",
            "src/old.rs",
            EvidenceLedgerCheckpoint {
                current_branch: Some("feature/context".to_string()),
                dirty_state_summary: "staged=0, unstaged=1, untracked=0".to_string(),
                touched_files: vec!["src/old.rs".to_string()],
                diff_hash: Some("def456".to_string()),
            },
        ));

        let summary = ledger.summary_for_session("session-a", 10);

        assert_eq!(summary.latest_checkpoints.len(), 1);
        assert_eq!(summary.latest_checkpoints[0].target, "src/old.rs");
        assert_eq!(
            summary.latest_checkpoints[0]
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.current_branch.as_deref()),
            Some("feature/context")
        );
    }

    #[test]
    fn summary_projects_partial_subagent_results() {
        let ledger = SessionEvidenceLedger::new();
        ledger.append(
            EvidenceLedgerEvent::new(
                "session-a",
                "turn-a",
                "Task",
                EvidenceLedgerTargetKind::Subagent,
                "ReviewSecurity",
                EvidenceLedgerEventStatus::PartialTimeout,
                "Security reviewer timed out after partial output.",
            )
            .with_error_kind("timeout")
            .with_partial_output("Found token logging before timeout."),
        );

        let summary = ledger.summary_for_session("session-a", 10);

        assert_eq!(summary.partial_subagent_results.len(), 1);
        assert_eq!(summary.partial_subagent_results[0].target, "ReviewSecurity");
        assert_eq!(
            summary.partial_subagent_results[0]
                .partial_output
                .as_deref(),
            Some("Found token logging before timeout.")
        );
    }

    #[test]
    fn partial_output_is_truncated_on_utf8_boundary() {
        let ledger = SessionEvidenceLedger::new();
        let output = format!("{}{}", "a".repeat(7_999), "测");
        ledger.append(
            EvidenceLedgerEvent::new(
                "session-a",
                "turn-a",
                "Task",
                EvidenceLedgerTargetKind::Subagent,
                "ReviewSecurity",
                EvidenceLedgerEventStatus::PartialTimeout,
                "Security reviewer timed out after partial output.",
            )
            .with_partial_output(output),
        );

        let summary = ledger.summary_for_session("session-a", 10);
        let partial_output = summary.partial_subagent_results[0]
            .partial_output
            .as_deref()
            .expect("partial output");

        assert_eq!(partial_output.len(), 7_999);
        assert!(partial_output.is_char_boundary(partial_output.len()));
    }

    #[test]
    fn summary_projects_into_compression_contract() {
        let ledger = SessionEvidenceLedger::new();
        ledger.append(
            EvidenceLedgerEvent::new(
                "session-a",
                "turn-a",
                "Edit",
                EvidenceLedgerTargetKind::File,
                "src/main.rs",
                EvidenceLedgerEventStatus::Succeeded,
                "Edited main file.",
            )
            .with_touched_files(vec!["src/main.rs".to_string()]),
        );
        ledger.append(
            EvidenceLedgerEvent::new(
                "session-a",
                "turn-a",
                "Bash",
                EvidenceLedgerTargetKind::Command,
                "cargo test",
                EvidenceLedgerEventStatus::Failed,
                "Tests failed before compression.",
            )
            .with_error_kind("exit_code:1"),
        );
        ledger.append(EvidenceLedgerEvent::new(
            "session-a",
            "turn-a",
            "Task",
            EvidenceLedgerTargetKind::Subagent,
            "ReviewSecurity",
            EvidenceLedgerEventStatus::PartialTimeout,
            "Security reviewer timed out after partial output.",
        ));

        let contract: crate::agentic::core::CompressionContract =
            ledger.summary_for_session("session-a", 10).into();

        assert_eq!(contract.touched_files, vec!["src/main.rs"]);
        assert_eq!(contract.verification_commands[0].target, "cargo test");
        assert_eq!(
            contract.blocking_failures[0].error_kind.as_deref(),
            Some("exit_code:1")
        );
        assert_eq!(contract.subagent_statuses[0].target, "ReviewSecurity");
        assert_eq!(contract.subagent_statuses[0].status, "partial_timeout");
    }
}
