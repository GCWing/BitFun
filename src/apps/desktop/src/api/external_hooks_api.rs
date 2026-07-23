//! Desktop transport for the runtime-free external Hook catalog.

use bitfun_core::external_hooks::{
    local_external_hook_catalog_snapshot, ExternalHookCatalogSnapshotV1,
};
use bitfun_core::external_sources::ExternalSourceOperationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookCatalogRequest {
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

pub type ExternalHookCatalogResponse = ExternalHookCatalogSnapshotV1;

#[tauri::command]
pub async fn get_external_hook_catalog(
    request: ExternalHookCatalogRequest,
) -> ExternalSourceOperationResult<ExternalHookCatalogResponse> {
    let workspace =
        super::external_sources_api::require_local_workspace(request.workspace_path.as_deref())
            .await?;
    local_external_hook_catalog_snapshot(workspace, request.force_refresh).await
}

#[cfg(test)]
mod tests {
    use super::ExternalHookCatalogRequest;

    #[test]
    fn request_uses_the_structured_camel_case_desktop_contract() {
        let request: ExternalHookCatalogRequest = serde_json::from_value(serde_json::json!({
            "workspacePath": "D:/workspace/project",
            "forceRefresh": true
        }))
        .unwrap();
        assert_eq!(
            request.workspace_path.as_deref(),
            Some("D:/workspace/project")
        );
        assert!(request.force_refresh);

        assert!(
            serde_json::from_value::<ExternalHookCatalogRequest>(serde_json::json!({
                "workspacePath": "D:/workspace/project",
                "forceRefresh": true,
                "unexpected": true
            }))
            .is_err()
        );
    }
}
