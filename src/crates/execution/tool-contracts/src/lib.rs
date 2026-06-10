//! Agent tool contracts.
//!
//! Pure tool DTOs and helpers live here before the concrete tool framework and
//! tool packs are moved out of the core facade.

pub mod execution_gate;
pub mod file_guidance;
pub mod file_read_freshness;
pub mod framework;
pub mod input_validator;
pub mod tool_execution_presentation;
pub mod tool_result_storage;

pub use bitfun_core_types::ToolImageAttachment;
pub use bitfun_runtime_ports::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
pub use execution_gate::{
    build_tool_call_loop_block_message, validate_tool_execution_admission, ToolCallLoopBlock,
    ToolCallLoopDecision, ToolCallLoopHistory, ToolExecutionAdmissionRejection,
    ToolExecutionAdmissionRequest, TOOL_CALL_HISTORY_WINDOW, TOOL_CALL_LOOP_THRESHOLD,
};
pub use file_guidance::{
    file_tool_guidance_message, is_file_tool_guidance_message, FILE_TOOL_GUIDANCE_PREFIX,
};
pub use file_read_freshness::{
    file_read_facts_are_fresh, file_read_facts_content_matches, normalize_tool_file_content,
    FileReadFreshnessFacts,
};
pub use framework::{
    build_bitfun_runtime_uri, build_collapsed_tool_stub_definition,
    build_get_tool_spec_assistant_detail, build_get_tool_spec_detail_result,
    build_get_tool_spec_duplicate_load_hint, build_get_tool_spec_duplicate_load_result,
    build_prompt_visible_tool_manifest_definitions, build_tool_manifest_policy_tools,
    build_tool_path_policy_denial_message, build_tool_runtime_artifact_reference,
    build_tool_session_runtime_artifact_reference, collect_loaded_collapsed_tool_names,
    get_tool_spec_input_schema, get_tool_spec_is_concurrency_safe, get_tool_spec_is_readonly,
    get_tool_spec_needs_permissions, get_tool_spec_short_description, is_bitfun_runtime_uri,
    is_remote_posix_path_within_root, is_tool_path_allowed_by_resolved_roots,
    materialize_static_tool_provider_groups, normalize_absolute_posix_path, normalize_host_path,
    normalize_runtime_relative_path, parse_bitfun_runtime_uri, posix_resolve_path_with_workspace,
    posix_style_path_is_absolute, render_get_tool_spec_tool_use_message,
    resolve_contextual_tool_manifest, resolve_contextual_tool_manifest_from_provider,
    resolve_contextual_visible_tools, resolve_contextual_visible_tools_from_provider,
    resolve_get_tool_spec_detail, resolve_get_tool_spec_detail_from_provider,
    resolve_get_tool_spec_execution_plan, resolve_get_tool_spec_execution_result_from_provider,
    resolve_host_path, resolve_host_path_with_workspace, resolve_readonly_enabled_tools,
    resolve_tool_manifest_policy, resolve_tool_path_with_context, resolve_workspace_tool_path,
    sort_tool_manifest_definitions, summarize_get_tool_spec_collapsed_tools,
    tool_manifest_sort_rank, tool_path_is_effectively_absolute, validate_collapsed_tool_usage,
    validate_get_tool_spec_input, validate_tool_allowed_by_list, CollapsedToolUsageError,
    ContextualToolManifest, ContextualToolManifestItem, ContextualVisibleTools, DynamicMcpToolInfo,
    DynamicToolInfo, GetToolSpecCatalogProvider, GetToolSpecCollapsedToolSummary,
    GetToolSpecDetail, GetToolSpecExecutionError, GetToolSpecExecutionPlan,
    GetToolSpecLoadObservation, GetToolSpecRuntime, ParsedBitFunRuntimeUri,
    PortableToolContextProvider, PromptVisibleToolManifestItem, SnapshotToolDecorator,
    SnapshotToolWrapper, SnapshotToolWrapperRef, StaticToolMaterializationError,
    StaticToolProvider, StaticToolProviderFactory, StaticToolProviderGroup, StaticToolProviderPlan,
    ToolCatalogRuntime, ToolCatalogSnapshotProvider, ToolContextFacts, ToolDecoratorRef,
    ToolExecutionAccessError, ToolExposure, ToolManifestDefinition, ToolManifestPolicyResolution,
    ToolManifestPolicyTool, ToolPathBackend, ToolPathContractError, ToolPathOperation,
    ToolPathPolicy, ToolPathResolution, ToolRef, ToolRegistry, ToolRegistryItem, ToolRenderOptions,
    ToolRestrictionError, ToolResult, ToolRuntimeAssembly, ToolRuntimeRestrictions,
    ToolWorkspaceKind, ValidationResult, BITFUN_RUNTIME_URI_PREFIX, GET_TOOL_SPEC_TOOL_NAME,
};
pub use input_validator::InputValidator;
pub use tool_execution_presentation::{
    build_invalid_tool_call_error_message, build_tool_call_truncation_recovery_notice,
    build_tool_execution_error_presentation, build_user_steering_interrupted_presentation,
    is_write_like_tool_name, render_tool_result_for_assistant, truncate_raw_tool_arguments_preview,
    truncate_raw_tool_arguments_preview_to, truncate_tool_arguments_preview,
    ToolExecutionErrorPresentation, TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES,
    USER_STEERING_INTERRUPTED_MESSAGE,
};
pub use tool_result_storage::{
    build_persisted_tool_output_message, count_tool_result_lines, generate_tool_result_preview,
    sanitize_tool_result_file_component, select_tool_result_indices_for_persistence,
    tool_result_is_persisted_output, PersistedToolOutput, ToolResultPersistenceCandidate,
    ToolResultStoragePolicy, DEFAULT_MAX_TOOL_RESULT_CHARS, MAX_TOOL_RESULTS_PER_ROUND_CHARS,
    PERSISTED_OUTPUT_CLOSING_TAG, PERSISTED_OUTPUT_TAG, TOOL_RESULT_PREVIEW_CHARS,
};
