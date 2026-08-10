//! External-source App Server wire schemas.
//!
//! These DTOs reuse the stable product-domain projections. Executable source
//! definitions and provider-private configuration never cross this boundary.

use std::collections::{BTreeMap, BTreeSet};

use agent_client_protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use bitfun_product_domains::external_source_control::{
    ExternalApplicationControlRequestV2, ExternalApplicationControlResultV2,
    ExternalApplicationReviewPageRequestV2 as DomainExternalApplicationReviewPageRequestV2,
    ExternalApplicationReviewPageV2, ExternalApplicationSnapshotV2, ExternalSourceControlRequestV1,
    ExternalSourceControlSnapshotV1, ExternalSourceSurfaceSnapshotV1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourcePublicSnapshot,
    NativePromptCommandConflictSnapshot, NativePromptCommandDescriptor,
    PromptCommandInvocationOutcome, PromptCommandShellReviewDecision,
};
use bitfun_product_domains::external_subagents::ExternalSubagentModelBindingTarget;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceConflictPreferences {
    pub choices: BTreeMap<String, String>,
    pub lineage_current_keys: BTreeMap<String, String>,
    pub conflicted_candidate_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "externalSource/snapshot", response = ExternalSourceSnapshotResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotRequest {
    pub workspace_path: String,
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotResponse {
    pub control: ExternalSourceControlSnapshotV1,
    pub snapshot: ExternalSourcePublicSnapshot,
    pub preferences: ExternalSourceConflictPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(
    method = "externalSource/applicationSnapshotV2",
    response = ExternalApplicationSnapshotResponseV2
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationSnapshotRequestV2 {
    pub workspace_path: Option<String>,
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(transparent)]
pub struct ExternalApplicationSnapshotResponseV2(pub ExternalApplicationSnapshotV2);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(
    method = "externalSource/applicationReviewPageV2",
    response = ExternalApplicationReviewPageResponseV2
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewPageRequest {
    pub workspace_path: Option<String>,
    pub request: DomainExternalApplicationReviewPageRequestV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(transparent)]
pub struct ExternalApplicationReviewPageResponseV2(pub ExternalApplicationReviewPageV2);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(
    method = "externalSource/applicationActionV2",
    response = ExternalApplicationActionResponseV2
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationActionRequest {
    pub workspace_path: Option<String>,
    pub request: ExternalApplicationControlRequestV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(transparent)]
pub struct ExternalApplicationActionResponseV2(pub ExternalApplicationControlResultV2);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
#[notification(method = "externalSource/event")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceEventNotification {
    pub cursor: crate::event::EventCursor,
    pub workspace_path: String,
    pub snapshot: ExternalSourcePublicSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "externalSource/control", response = ExternalSourceControlResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlRequest {
    pub workspace_path: String,
    pub request: ExternalSourceControlRequestV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlResponse {
    pub surface: ExternalSourceSurfaceSnapshotV1,
    pub snapshot: ExternalSourceSnapshotResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "externalSource/review", response = ExternalSourceReviewResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceReviewRequest {
    pub workspace_path: String,
    pub operation_id: String,
    pub action: ExternalSourceReviewAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalSourceReviewAction {
    Refresh,
    SetPromptCommandConflictChoice {
        conflict_key: String,
        candidate_id: String,
        expected_preference_revision: u64,
    },
    SetToolTargetDecision {
        approval_key: String,
        decision_key: String,
        approved: bool,
        expected_preference_revision: u64,
    },
    SetToolConflictChoice {
        conflict_key: String,
        candidate_id: String,
        expected_preference_revision: u64,
    },
    SetSubagentActivation {
        candidate_id: String,
        approved: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
        decision_key: String,
    },
    SetSubagentModelBinding {
        binding_key: String,
        target: Option<ExternalSubagentModelBindingTarget>,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
    ChooseSubagentConflict {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ExternalSourceReviewResponse(pub ExternalSourceSnapshotResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(
    method = "externalSource/setNativeCommandChoice",
    response = SetNativeCommandChoiceResponse
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetNativeCommandChoiceRequest {
    pub workspace_path: String,
    pub operation_id: String,
    pub native_commands: Vec<NativePromptCommandDescriptor>,
    pub selected_candidate_id: String,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetNativeCommandChoiceResponse {
    pub conflicts: NativePromptCommandConflictSnapshot,
    pub preferences: ExternalSourceConflictPreferences,
}

#[derive(Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "externalSource/expandCommand", response = ExpandExternalCommandResponse)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpandExternalCommandRequest {
    pub workspace_path: String,
    pub operation_id: String,
    pub command_name: String,
    pub arguments: String,
    pub native_commands: Vec<NativePromptCommandDescriptor>,
    pub candidate_id: Option<String>,
    pub content_version: Option<String>,
    pub native_conflict_key: Option<String>,
    pub expected_preference_revision: Option<u64>,
    pub shell_review_decision: Option<PromptCommandShellReviewDecision>,
}

impl std::fmt::Debug for ExpandExternalCommandRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExpandExternalCommandRequest")
            .field("workspace_path", &"[REDACTED]")
            .field("operation_id", &self.operation_id)
            .field("command_name", &self.command_name)
            .field("arguments_bytes", &self.arguments.len())
            .field("native_command_count", &self.native_commands.len())
            .field("candidate_id", &self.candidate_id)
            .field("content_version", &self.content_version)
            .field("native_conflict_key", &self.native_conflict_key)
            .field(
                "expected_preference_revision",
                &self.expected_preference_revision,
            )
            .field(
                "shell_review_decision_configured",
                &self.shell_review_decision.is_some(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ExpandExternalCommandResponse(pub PromptCommandInvocationOutcome);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceErrorData {
    pub app: crate::error::AppServerErrorData,
    pub error: ExternalSourceOperationError,
}

pub fn validate_operation_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 160
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid external source operation id")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_identity_is_required_and_bounded() {
        assert!(validate_operation_id("tui-operation-1").is_ok());
        assert!(validate_operation_id("").is_err());
        assert!(validate_operation_id(" leading").is_err());
        assert!(validate_operation_id(&"x".repeat(161)).is_err());
    }

    #[test]
    fn command_debug_redacts_workspace_and_arguments() {
        let request = ExpandExternalCommandRequest {
            workspace_path: "C:/secret/project".to_string(),
            operation_id: "operation-1".to_string(),
            command_name: "review".to_string(),
            arguments: "--token secret".to_string(),
            native_commands: Vec::new(),
            candidate_id: None,
            content_version: None,
            native_conflict_key: None,
            expected_preference_revision: None,
            shell_review_decision: None,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("C:/secret/project"));
        assert!(!debug.contains("--token secret"));
    }

    #[test]
    fn application_v2_wire_requests_keep_workspace_binding_outside_domain_payloads() {
        let snapshot: ExternalApplicationSnapshotRequestV2 =
            serde_json::from_value(serde_json::json!({
                "workspacePath": null,
                "forceRefresh": true
            }))
            .unwrap();
        assert_eq!(snapshot.workspace_path, None);
        assert!(snapshot.force_refresh);

        let page: ExternalApplicationReviewPageRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": "C:/work/project",
                "request": {
                    "schemaVersion": 2,
                    "executionDomainId": "host-a",
                    "workspaceScopeId": "workspace-a",
                    "targetScope": "workspace_override",
                    "reviewId": "review-a",
                    "preferenceRevision": 4,
                    "expectedGenerations": [],
                    "pageSize": 64
                }
            }))
            .unwrap();
        assert_eq!(page.workspace_path.as_deref(), Some("C:/work/project"));
        assert_eq!(page.request.page_size, 64);

        let action: ExternalApplicationActionRequest = serde_json::from_value(serde_json::json!({
            "workspacePath": "C:/work/project",
            "request": {
                "schemaVersion": 2,
                "executionDomainId": "host-a",
                "workspaceScopeId": "workspace-a",
                "targetScope": "workspace_override",
                "operationId": "operation-a",
                "expectedPreferenceRevision": 4,
                "action": {
                    "type": "connect_application",
                    "applicationId": "opencode"
                }
            }
        }))
        .unwrap();
        assert_eq!(action.workspace_path.as_deref(), Some("C:/work/project"));
        assert_eq!(action.request.operation_id, "operation-a");
    }

    #[test]
    fn application_v2_snapshot_response_serializes_as_the_domain_object() {
        let domain_json = serde_json::json!({
            "schemaVersion": 2,
            "executionDomainId": "host-a",
            "effectiveConnectionScope": "user_default",
            "refreshGeneration": 7,
            "preferenceRevision": 4,
            "safeMode": false,
            "hostCapabilities": {
                "canReadSnapshot": true,
                "canReadReview": true,
                "canMutate": true,
                "canManageUserDefault": true,
                "canManageWorkspaceOverride": true,
                "canRefresh": true,
                "canSetSafeMode": true
            },
            "applications": []
        });
        let domain: ExternalApplicationSnapshotV2 =
            serde_json::from_value(domain_json.clone()).unwrap();

        assert_eq!(
            serde_json::to_value(ExternalApplicationSnapshotResponseV2(domain)).unwrap(),
            domain_json
        );

        let page_json = serde_json::json!({
            "schemaVersion": 2,
            "executionDomainId": "host-a",
            "targetScope": "user_default",
            "reviewId": "review-a",
            "preferenceRevision": 4,
            "expectedGenerations": [],
            "totalCount": 0,
            "items": []
        });
        let page: ExternalApplicationReviewPageV2 =
            serde_json::from_value(page_json.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(ExternalApplicationReviewPageResponseV2(page)).unwrap(),
            page_json
        );

        let action_json = serde_json::json!({
            "schemaVersion": 2,
            "operationId": "operation-a",
            "preferenceRevision": 5,
            "outcome": "applied",
            "itemResults": []
        });
        let action: ExternalApplicationControlResultV2 =
            serde_json::from_value(action_json.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(ExternalApplicationActionResponseV2(action)).unwrap(),
            action_json
        );
    }
}
