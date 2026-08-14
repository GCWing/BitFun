use super::*;

#[derive(Debug, Clone)]
pub(crate) struct NativeHookHandlerSummary {
    pub command_summary: String,
    pub timeout_seconds: u64,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NativeHookRuleSummary {
    pub event: String,
    pub matcher: String,
    pub matcher_is_valid: bool,
    pub scope: String,
    pub handlers: Vec<NativeHookHandlerSummary>,
}
#[derive(Debug, Clone)]
pub(crate) struct NativeHookFileSummary {
    pub scope: String,
    pub location: String,
    pub exists: bool,
    pub loaded: bool,
}

#[derive(Clone)]
pub(crate) struct NativeHookOverviewView {
    pub enabled: bool,
    pub project_hooks_enabled: bool,
    pub files: Vec<NativeHookFileSummary>,
    pub rules: Vec<NativeHookRuleSummary>,
    pub total_handlers: usize,
    pub issues: Vec<String>,
}

impl std::fmt::Debug for NativeHookOverviewView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeHookOverviewView")
            .field("enabled", &self.enabled)
            .field("project_hooks_enabled", &self.project_hooks_enabled)
            .field("file_count", &self.files.len())
            .field("rule_count", &self.rules.len())
            .field("total_handlers", &self.total_handlers)
            .field("issue_count", &self.issues.len())
            .finish()
    }
}

pub(crate) struct NativeHookProvider;

impl NativeHookProvider {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn overview(
        &self,
        scope: &ManagementScope,
    ) -> ManagementResult<NativeHookOverviewView> {
        let workspace = scope.local_workspace("Native hook management")?;
        Ok(project_native_hook_overview(
            bitfun_core::native_hooks::overview_for_management(workspace).await,
        ))
    }
}

pub(crate) fn project_native_hook_overview(
    overview: bitfun_core::native_hooks::NativeHookManagementOverview,
) -> NativeHookOverviewView {
    NativeHookOverviewView {
        enabled: overview.enabled,
        project_hooks_enabled: overview.project_hooks_enabled,
        files: overview
            .files
            .into_iter()
            .map(|file| NativeHookFileSummary {
                scope: file.scope,
                location: file.location,
                exists: file.exists,
                loaded: file.loaded,
            })
            .collect(),
        rules: overview
            .rules
            .into_iter()
            .map(|rule| NativeHookRuleSummary {
                event: rule.event,
                matcher: rule.matcher,
                matcher_is_valid: rule.matcher_is_valid,
                scope: rule.scope,
                handlers: rule
                    .handlers
                    .into_iter()
                    .map(|handler| NativeHookHandlerSummary {
                        command_summary: handler.command_summary,
                        timeout_seconds: handler.timeout_seconds,
                        status_message: handler.status_message,
                    })
                    .collect(),
            })
            .collect(),
        total_handlers: overview.total_handlers,
        issues: overview.issues,
    }
}

pub(crate) struct ExternalHookProvider;

impl ExternalHookProvider {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn snapshot(
        &self,
        scope: &ManagementScope,
        refresh_updates: bool,
    ) -> ManagementResult<ExternalHookImportSnapshotV1> {
        let workspace = scope.local_workspace("External hook management")?;
        bitfun_core::external_hook_import::external_hook_import_snapshot(
            Some(workspace),
            refresh_updates,
        )
        .await
        .map_err(map_external_error)
    }

    pub(crate) async fn plan(
        &self,
        scope: &ManagementScope,
        source: SourceKey,
    ) -> ManagementResult<ExternalHookImportPlanV1> {
        let workspace = scope.local_workspace("External hook management")?;
        bitfun_core::external_hook_import::plan_external_hook_import(Some(workspace), source)
            .await
            .map_err(map_external_error)
    }

    pub(crate) async fn apply(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
        request: ExternalHookImportApplyRequestV1,
    ) -> ManagementResult<ExternalHookImportApplyResultV1> {
        let workspace = scope.local_workspace("External hook management")?;
        validate_external_operation_id(operation_id)?;
        bitfun_core::external_hook_import::apply_external_hook_import(Some(workspace), request)
            .await
            .map_err(map_external_error)
    }

    pub(crate) async fn mutate(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
        request: ExternalHookImportMutationRequestV1,
    ) -> ManagementResult<ExternalHookImportSnapshotV1> {
        let workspace = scope.local_workspace("External hook management")?;
        validate_external_operation_id(operation_id)?;
        bitfun_core::external_hook_import::mutate_external_hook_import(Some(workspace), request)
            .await
            .map_err(map_external_error)
    }
}
