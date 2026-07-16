//! Desktop host API for ecosystem-neutral external AI application sources.

use bitfun_core::external_sources::{
    external_source_snapshot, set_external_prompt_command_conflict_choice,
    set_external_source_enabled, ExternalSourceCatalogEntry, ExternalSourceCatalogSnapshot,
    ExternalSourceDiagnostic, PromptCommandAvailability,
};
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_product_domains::external_sources::{PromptCommandConflict, SourceQualifiedCommandId};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotRequest {
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSourceEnabledRequest {
    pub workspace_path: Option<String>,
    pub source_key: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSourceConflictChoiceRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotResponse {
    pub generation: u64,
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub commands: Vec<ExternalPromptCommandSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_conflicts: Vec<PromptCommandConflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalPromptCommandSummary {
    pub definition: ExternalPromptCommandDefinitionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalPromptCommandDefinitionSummary {
    pub id: SourceQualifiedCommandId,
    pub name: String,
    pub description: String,
    pub availability: PromptCommandAvailability,
    pub content_version: String,
}

impl From<ExternalSourceCatalogSnapshot> for ExternalSourceSnapshotResponse {
    fn from(snapshot: ExternalSourceCatalogSnapshot) -> Self {
        Self {
            generation: snapshot.generation,
            discovery_pending: snapshot.discovery_pending,
            sources: snapshot.sources,
            commands: snapshot
                .commands
                .into_iter()
                .map(|entry| ExternalPromptCommandSummary {
                    definition: ExternalPromptCommandDefinitionSummary {
                        id: entry.definition.id,
                        name: entry.definition.name,
                        description: entry.definition.description,
                        availability: entry.definition.availability,
                        content_version: entry.definition.content_version,
                    },
                })
                .collect(),
            command_conflicts: snapshot.command_conflicts,
            diagnostics: snapshot.diagnostics,
        }
    }
}

async fn require_local_workspace(workspace_path: Option<&str>) -> Result<Option<&Path>, String> {
    let Some(workspace_path) = workspace_path else {
        return Ok(None);
    };
    let path = Path::new(workspace_path);
    if !path.is_absolute() {
        return Err("External AI application sources require an absolute workspace path".into());
    }
    if is_remote_path(workspace_path).await {
        return Err(
            "External AI application sources are not available for remote workspaces yet".into(),
        );
    }
    Ok(Some(path))
}

#[tauri::command]
pub async fn get_external_source_snapshot(
    request: ExternalSourceSnapshotRequest,
) -> Result<ExternalSourceSnapshotResponse, String> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    external_source_snapshot(workspace, request.force_refresh)
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn set_external_source_enabled_command(
    request: SetExternalSourceEnabledRequest,
) -> Result<ExternalSourceSnapshotResponse, String> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_source_enabled(workspace, &request.source_key, request.enabled)
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn set_external_source_conflict_choice_command(
    request: SetExternalSourceConflictChoiceRequest,
) -> Result<ExternalSourceSnapshotResponse, String> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_prompt_command_conflict_choice(
        workspace,
        &request.conflict_key,
        &request.candidate_id,
    )
    .await
    .map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_snapshot_never_serializes_prompt_templates() {
        let snapshot: ExternalSourceCatalogSnapshot = serde_json::from_value(serde_json::json!({
            "generation": 1,
            "discoveryPending": false,
            "sources": [],
            "commands": [{
                "definition": {
                    "id": {
                        "source": { "providerId": "opencode.commands", "sourceId": "global" },
                        "localId": "review"
                    },
                    "name": "review",
                    "description": "Review changes",
                    "template": "sensitive prompt body",
                    "availability": { "state": "available" },
                    "contentVersion": "v1"
                }
            }],
            "commandConflicts": [],
            "diagnostics": []
        }))
        .unwrap();

        let value = serde_json::to_value(ExternalSourceSnapshotResponse::from(snapshot)).unwrap();

        assert_eq!(value["commands"][0]["definition"]["name"], "review");
        assert!(value["commands"][0]["definition"].get("template").is_none());
    }
}
