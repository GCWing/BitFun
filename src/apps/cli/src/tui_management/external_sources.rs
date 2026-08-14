use super::*;

#[derive(Debug, Clone)]
pub(crate) struct NativeCommandChoiceView {
    pub conflicts: NativePromptCommandConflictSnapshot,
    pub preferences: ExternalSourcePreferences,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalSourcePreferences {
    pub choices: BTreeMap<String, String>,
    pub lineage_current_keys: BTreeMap<String, String>,
    pub conflicted_candidate_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalSourceSnapshotView {
    pub surface: ExternalSourceSurfaceSnapshotV1,
    pub preferences: ExternalSourcePreferences,
}

pub(crate) use bitfun_core::external_sources::ExternalSourceReviewAction;

pub(crate) struct ExternalSourceProvider;

impl ExternalSourceProvider {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn snapshot(
        &self,
        scope: &ManagementScope,
        force_refresh: bool,
    ) -> ManagementResult<ExternalSourceSnapshotView> {
        let workspace = scope.local_workspace("External source management")?;
        let surface = bitfun_core::external_sources::get_external_source_control_snapshot(
            Some(workspace),
            force_refresh,
            ExternalSourceHostCapabilities::read_write(),
        )
        .await
        .map_err(map_external_error)?;
        Ok(ExternalSourceSnapshotView {
            surface,
            preferences: external_source_preferences().await?,
        })
    }

    pub(crate) async fn control(
        &self,
        scope: &ManagementScope,
        request: ExternalSourceControlRequestV1,
    ) -> ManagementResult<ExternalSourceSnapshotView> {
        let workspace = scope.local_workspace("External source management")?;
        request
            .validate()
            .map_err(ManagementError::invalid_request)?;
        bitfun_core::external_sources::apply_external_source_control_action(
            Some(workspace),
            request,
        )
        .await
        .map_err(map_external_error)?;
        self.snapshot(scope, false).await
    }

    pub(crate) async fn review(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
        action: ExternalSourceReviewAction,
    ) -> ManagementResult<ExternalSourceSnapshotView> {
        let workspace = scope.local_workspace("External source management")?;
        validate_external_operation_id(operation_id)?;
        let result =
            bitfun_core::external_sources::review_external_source(Some(workspace), action).await;
        result.map_err(|error| map_external_string_error_with_id(error, operation_id))?;
        self.snapshot(scope, false).await
    }
}

async fn external_source_preferences() -> ManagementResult<ExternalSourcePreferences> {
    bitfun_core::external_sources::external_source_conflict_choices()
        .await
        .map(
            |(choices, lineage_current_keys, conflicted_candidate_ids)| ExternalSourcePreferences {
                choices,
                lineage_current_keys,
                conflicted_candidate_ids,
            },
        )
        .map_err(map_external_string_error)
}

#[derive(Clone)]
pub(crate) struct ExternalCommandExpansionRequest {
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

pub(crate) struct ExternalCommandProvider;

impl ExternalCommandProvider {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn set_native_choice(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
        native_commands: Vec<NativePromptCommandDescriptor>,
        selected_candidate_id: &str,
        expected_preference_revision: u64,
    ) -> ManagementResult<(
        NativePromptCommandConflictSnapshot,
        ExternalSourcePreferences,
    )> {
        let workspace = scope.local_workspace("External command management")?;
        validate_external_operation_id(operation_id)?;
        let conflicts = bitfun_core::external_sources::set_native_prompt_command_conflict_choice(
            Some(workspace),
            native_commands,
            selected_candidate_id,
            expected_preference_revision,
        )
        .await
        .map_err(|error| map_external_string_error_with_id(error, operation_id))?;
        Ok((conflicts, external_source_preferences().await?))
    }

    pub(crate) async fn expand(
        &self,
        scope: &ManagementScope,
        request: ExternalCommandExpansionRequest,
    ) -> ManagementResult<PromptCommandInvocationOutcome> {
        let workspace = scope.local_workspace("External command management")?;
        validate_external_operation_id(&request.operation_id)?;
        let operation_id = request.operation_id.clone();
        bitfun_core::external_sources::expand_external_prompt_command(
            Some(workspace),
            &request.command_name,
            &request.arguments,
            request.native_commands,
            request.candidate_id.as_deref(),
            request.content_version.as_deref(),
            request.native_conflict_key.as_deref(),
            request.expected_preference_revision,
            request.shell_review_decision.as_ref(),
        )
        .await
        .map_err(|error| map_external_string_error_with_id(error, &operation_id))
    }
}
