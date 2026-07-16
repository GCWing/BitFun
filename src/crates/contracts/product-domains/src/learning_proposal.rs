use serde::{Deserialize, Serialize};

pub const LEARNING_PROPOSAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningProposalStatus {
    Analyzing,
    Ready,
    Applying,
    Applied,
    Rejected,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningProposalSourceKind {
    UserMessage,
    AssistantText,
    AssistantThinking,
    Tool,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningProposalTargetKind {
    Memory,
    Skill,
    AgentsMd,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningProposalApplyMode {
    MemoryNote,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposalSelection {
    pub selected_text: String,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub source_kind: LearningProposalSourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLearningProposalRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    pub source: LearningProposalSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposalSource {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    pub selected_text: String,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub source_kind: LearningProposalSourceKind,
}

impl From<CreateLearningProposalRequest> for LearningProposalSource {
    fn from(request: CreateLearningProposalRequest) -> Self {
        Self {
            session_id: request.session_id,
            workspace_path: request.workspace_path,
            remote_connection_id: request.remote_connection_id,
            remote_ssh_host: request.remote_ssh_host,
            selected_text: request.source.selected_text,
            turn_id: request.source.turn_id,
            round_id: request.source.round_id,
            item_id: request.source.item_id,
            source_kind: request.source.source_kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposalTarget {
    pub kind: LearningProposalTargetKind,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub apply_mode: LearningProposalApplyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposalPreview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub original_content: String,
    pub proposed_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposalError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningProposal {
    pub schema_version: u32,
    pub proposal_id: String,
    pub status: LearningProposalStatus,
    pub source: LearningProposalSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<LearningProposalTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub future_use: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<LearningProposalPreview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_hash: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<LearningProposalError>,
}

impl LearningProposal {
    pub fn new_analyzing(
        proposal_id: String,
        source: LearningProposalSource,
        timestamp: u64,
    ) -> Self {
        Self {
            schema_version: LEARNING_PROPOSAL_SCHEMA_VERSION,
            proposal_id,
            status: LearningProposalStatus::Analyzing,
            source,
            target: None,
            rationale: None,
            future_use: None,
            preview: None,
            base_hash: None,
            diff_hash: None,
            created_at: timestamp,
            updated_at: timestamp,
            error: None,
        }
    }

    pub fn can_refresh(&self) -> bool {
        matches!(
            self.status,
            LearningProposalStatus::Analyzing
                | LearningProposalStatus::Ready
                | LearningProposalStatus::Stale
                | LearningProposalStatus::Failed
        )
    }

    pub fn can_approve(&self) -> bool {
        self.status == LearningProposalStatus::Ready
            && self
                .target
                .as_ref()
                .is_some_and(|target| target.apply_mode == LearningProposalApplyMode::MemoryNote)
            && self.preview.is_some()
            && self.base_hash.is_some()
            && self.diff_hash.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLearningProposalRequest {
    pub proposal_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

pub type RefreshLearningProposalRequest = GetLearningProposalRequest;
pub type RejectLearningProposalRequest = GetLearningProposalRequest;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLearningProposalsRequest {
    #[serde(default)]
    pub include_resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveLearningProposalRequest {
    pub proposal_id: String,
    pub base_hash: String,
    pub diff_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_proposal_starts_analyzing_and_is_not_approvable() {
        let source = LearningProposalSource {
            session_id: "session-1".to_string(),
            workspace_path: "C:/repo".to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
            selected_text: "important".to_string(),
            turn_id: "turn-1".to_string(),
            round_id: None,
            item_id: None,
            source_kind: LearningProposalSourceKind::AssistantText,
        };
        let proposal =
            LearningProposal::new_analyzing("proposal-1".to_string(), source, 1_752_537_600_000);

        assert_eq!(proposal.status, LearningProposalStatus::Analyzing);
        assert!(proposal.can_refresh());
        assert!(!proposal.can_approve());
    }

    #[test]
    fn request_and_proposal_use_camel_case_wire_fields() {
        let request: CreateLearningProposalRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1",
            "workspacePath": "C:/repo",
            "source": {
                "selectedText": "important",
                "turnId": "turn-1",
                "roundId": "round-1",
                "itemId": "item-1",
                "sourceKind": "assistant_text"
            }
        }))
        .unwrap();
        assert_eq!(request.source.round_id.as_deref(), Some("round-1"));

        let proposal = LearningProposal::new_analyzing(
            "proposal-1".to_string(),
            request.into(),
            1_752_537_600_000,
        );
        let encoded = serde_json::to_value(proposal).unwrap();
        assert_eq!(encoded["createdAt"], 1_752_537_600_000_u64);
        assert_eq!(encoded["source"]["selectedText"], "important");
        assert!(encoded.get("created_at").is_none());
    }
}
