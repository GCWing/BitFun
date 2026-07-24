// Pure projections and review text derived from the external-source catalog.
use bitfun_product_domains::external_source_control::{
    ExternalSourceDesiredState, ExternalSourceEffectiveStatus, ExternalSourceRecoveryActionV1,
    ExternalSourceSupportState,
};

fn external_command_projections(
    snapshot: &ExternalSourceCatalogSnapshot,
    conflict_choices: &BTreeMap<String, String>,
) -> Vec<ExternalCommandProjection> {
    let built_in_actions = slash_actions(ActionState::chat(false, false));
    let mut projections = snapshot
        .commands
        .iter()
        .filter_map(|entry| {
            let ecosystem = snapshot
                .sources
                .iter()
                .find(|source| source.record.key == entry.definition.id.source)
                .map(|source| source.record.ecosystem_id.as_str())
                .unwrap_or("external");
            let restricted = !matches!(
                entry.definition.availability,
                PromptCommandAvailability::Available
            );
            let native_collision = built_in_actions.iter().find_map(|action| {
                if !action
                    .name
                    .trim_start_matches('/')
                    .eq_ignore_ascii_case(&entry.definition.name)
                {
                    return None;
                }
                let source = snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == entry.definition.id.source)?;
                let native_candidate_id = format!("bitfun.cli:{}", action.id);
                let external_candidate_id = entry.definition.id.stable_key();
                let conflict_key = native_prompt_command_conflict_key(
                    source.record.execution_domain_id.as_str(),
                    &entry.definition.name,
                    [
                        (
                            native_candidate_id.as_str(),
                            action_conflict_behavior_version(action.id),
                        ),
                        (
                            external_candidate_id.as_str(),
                            entry.definition.content_version.as_str(),
                        ),
                    ],
                );
                Some(NativeCommandCollisionProjection {
                    native_action_id: action.id.to_string(),
                    native_candidate_id,
                    external_candidate_id,
                    selected_candidate_id: conflict_choices.get(&conflict_key).cloned(),
                    conflict_key,
                })
            });
            Some(ExternalCommandProjection {
                action_id: format!("external-command:{}", entry.definition.name),
                command_name: entry.definition.name.clone(),
                invocation_alias: format!("/{}", entry.definition.name),
                candidate_id: entry.definition.id.stable_key(),
                content_version: entry.definition.content_version.clone(),
                description: format!("{} · {}", entry.definition.description, ecosystem),
                restricted,
                provider_conflict_key: None,
                native_collision,
            })
        })
        .collect::<Vec<_>>();

    for conflict in snapshot
        .command_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
    {
        let built_in = built_in_actions.iter().find(|action| {
            action
                .name
                .trim_start_matches('/')
                .eq_ignore_ascii_case(&conflict.command_name)
        });
        let native_group = built_in.and_then(|action| {
            let execution_domain = conflict.candidates.iter().find_map(|candidate| {
                snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == candidate.source)
                    .map(|source| source.record.execution_domain_id.as_str())
            })?;
            let native_candidate_id = format!("bitfun.cli:{}", action.id);
            let mut candidates = conflict
                .candidates
                .iter()
                .map(|candidate| {
                    (
                        candidate.candidate_id.as_str(),
                        candidate.content_version.as_str(),
                    )
                })
                .collect::<Vec<_>>();
            candidates.push((
                native_candidate_id.as_str(),
                action_conflict_behavior_version(action.id),
            ));
            let conflict_key = native_prompt_command_conflict_key(
                execution_domain,
                &conflict.command_name,
                candidates,
            );
            Some((action.id.to_string(), native_candidate_id, conflict_key))
        });
        projections.extend(conflict.candidates.iter().map(|candidate| {
            let native_collision = native_group.as_ref().map(
                |(native_action_id, native_candidate_id, conflict_key)| {
                    NativeCommandCollisionProjection {
                        native_action_id: native_action_id.clone(),
                        native_candidate_id: native_candidate_id.clone(),
                        external_candidate_id: candidate.candidate_id.clone(),
                        selected_candidate_id: conflict_choices.get(conflict_key).cloned(),
                        conflict_key: conflict_key.clone(),
                    }
                },
            );
            ExternalCommandProjection {
                action_id: format!("external-command-candidate:{}", candidate.candidate_id),
                command_name: conflict.command_name.clone(),
                invocation_alias: format!("/{}", conflict.command_name),
                candidate_id: candidate.candidate_id.clone(),
                content_version: candidate.content_version.clone(),
                description: format!(
                    "{} · {} · {}",
                    candidate.command_description,
                    candidate.source_display_name,
                    candidate.ecosystem_id
                ),
                restricted: !matches!(candidate.availability, PromptCommandAvailability::Available),
                provider_conflict_key: Some(conflict.conflict_key.clone()),
                native_collision,
            }
        }));
    }
    projections
}

fn external_command_counts(snapshot: &ExternalSourceCatalogSnapshot) -> (usize, usize) {
    snapshot
        .commands
        .iter()
        .fold((0, 0), |(available, restricted), entry| {
            if matches!(
                entry.definition.availability,
                PromptCommandAvailability::Available
            ) {
                (available + 1, restricted)
            } else {
                (available, restricted + 1)
            }
        })
}

fn external_integration_policy_lines(snapshot: &ExternalSourceCatalogSnapshot) -> Vec<String> {
    let policy = &snapshot.integration_policy;
    if policy.status
        == bitfun_core::external_sources::ExternalIntegrationPolicyStatus::IncompatibleSchema
    {
        return vec![
            format!(
                "Access: safely off; unsupported policy schema {}",
                policy.schema_major
            ),
            "Recover: bitfun config external reset-incompatible".to_string(),
        ];
    }
    if !policy.status.is_compatible() {
        return vec![
            format!(
                "Access: safely off; unsupported policy status '{}'",
                policy.status.as_str()
            ),
            "Recover: upgrade BitFun or connect through a compatible workspace host".to_string(),
        ];
    }
    let scope = if policy.workspace_override.is_some() {
        "this project overrides global settings"
    } else {
        "this project inherits global settings"
    };
    if policy.registered_ecosystems.is_empty() {
        return vec![format!("Access: unavailable; {scope}")];
    }
    let mut lines = vec![format!(
        "Access: {}; {scope}",
        if policy.effective.enabled {
            "enabled"
        } else {
            "disabled"
        }
    )];
    for descriptor in &policy.registered_ecosystems {
        let Some(ecosystem) = policy.effective.ecosystems.get(&descriptor.ecosystem_id) else {
            lines.push(format!("{}: unavailable", descriptor.display_name));
            continue;
        };
        let mode = match ecosystem.mode.as_str() {
            "recommended" => "recommended",
            "discover_only" => "discover only",
            "disabled" => "off",
            "custom" => "custom",
            _ => "unsupported, safely off",
        };
        let capability_summary = descriptor
            .capabilities
            .iter()
            .filter_map(|capability| {
                ecosystem
                    .capabilities
                    .get(&capability.capability_id)
                    .map(|access| {
                        let access = match access.as_str() {
                            "disabled" => "off",
                            "discover_only" => "discover",
                            "ask_before_use" => "ask",
                            "auto" => "auto",
                            _ => "unsupported, safely off",
                        };
                        format!("{} {access}", capability.capability_id.as_str())
                    })
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{}: {mode}; {capability_summary}",
            descriptor.display_name
        ));
    }
    lines.push("Manage: bitfun config external --help".to_string());
    lines
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalToolReviewAction {
    Show,
    Refresh,
    Decide {
        approval_key: String,
        decision_key: String,
        approved: bool,
    },
    Choose {
        conflict_key: String,
        candidate_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalControlUiAction {
    Show,
    Refresh,
    SetSafeMode(bool),
    SetSourceEnabled { source_key: String, enabled: bool },
}

fn parse_external_control_action(arguments: &str) -> Result<ExternalControlUiAction, String> {
    match arguments.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] | ["status"] => Ok(ExternalControlUiAction::Show),
        ["refresh"] => Ok(ExternalControlUiAction::Refresh),
        ["safe-mode", "on"] => Ok(ExternalControlUiAction::SetSafeMode(true)),
        ["safe-mode", "off"] => Ok(ExternalControlUiAction::SetSafeMode(false)),
        ["source", "enable", source_key] => Ok(ExternalControlUiAction::SetSourceEnabled {
            source_key: (*source_key).to_string(),
            enabled: true,
        }),
        ["source", "disable", source_key] => Ok(ExternalControlUiAction::SetSourceEnabled {
            source_key: (*source_key).to_string(),
            enabled: false,
        }),
        _ => Err("usage: /extensions [status | refresh | safe-mode on | safe-mode off | source enable <source-key> | source disable <source-key>]".to_string()),
    }
}

fn external_control_review_text(
    control: &bitfun_core::external_sources::ExternalSourceControlSnapshotV1,
) -> String {
    use bitfun_core::external_sources::{ExternalCapabilityKindV1, ExternalSourceRuntimeState};

    let mut lines = vec![
        "External integrations".to_string(),
        String::new(),
        format!(
            "Safe Mode: {}",
            if control.safe_mode { "on" } else { "off" }
        ),
        format!("Execution domain: {}", control.execution_domain_id),
        format!("Generation: {}", control.refresh_generation),
        format!("Sources: {}", control.sources.len()),
    ];
    if control.safe_mode {
        lines.push(
            "New external Tool, Agent, and MCP calls are blocked; calls already in progress are not cancelled."
                .to_string(),
        );
        lines.push(
            "Safe Mode applies only to this Host process and execution domain; restarting the Host turns it off."
                .to_string(),
        );
    }
    for source in &control.sources {
        let desired = match source.desired {
            ExternalSourceDesiredState::Enabled => "enabled",
            ExternalSourceDesiredState::Disabled => "disabled",
        };
        let effective = match source.effective_status {
            ExternalSourceEffectiveStatus::Discovering => "discovering",
            ExternalSourceEffectiveStatus::Disabled => "disabled",
            ExternalSourceEffectiveStatus::ReviewRequired => "review required",
            ExternalSourceEffectiveStatus::Conflict => "conflict",
            ExternalSourceEffectiveStatus::Active => "active",
            ExternalSourceEffectiveStatus::Degraded => "degraded",
            ExternalSourceEffectiveStatus::Unsupported => "unsupported",
            ExternalSourceEffectiveStatus::Available => "available",
            ExternalSourceEffectiveStatus::Removed => "removed",
        };
        lines.push(format!(
            "Source {}: {} ({desired}, {effective})",
            source.stable_key, source.display_name
        ));
    }
    for capability in &control.capabilities {
        let label = match capability.kind {
            ExternalCapabilityKindV1::Command => "Commands",
            ExternalCapabilityKindV1::Tool => "Tools",
            ExternalCapabilityKindV1::Subagent => "Agents",
            ExternalCapabilityKindV1::Mcp => "MCP servers",
        };
        let runtime = match capability.runtime {
            ExternalSourceRuntimeState::NotApplicable => "not applicable",
            ExternalSourceRuntimeState::Inactive => "inactive",
            ExternalSourceRuntimeState::Starting => "starting",
            ExternalSourceRuntimeState::Active => "active",
            ExternalSourceRuntimeState::Degraded => "degraded",
            ExternalSourceRuntimeState::Quarantined => "quarantined",
            ExternalSourceRuntimeState::Unsupported => "unsupported",
        };
        let support = match capability.support {
            ExternalSourceSupportState::Supported => "",
            ExternalSourceSupportState::Partial => ", support: partial",
            ExternalSourceSupportState::Unsupported => ", support: unsupported",
            ExternalSourceSupportState::Unavailable => ", support: unavailable",
        };
        lines.push(format!(
            "{label}: {} items, {} review, {} conflicts, {runtime}{support}",
            capability.item_count,
            capability.pending_review_count,
            capability.unresolved_conflict_count,
        ));
    }
    const MAX_STATUS_DETAILS: usize = 4;
    if !control.diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("Issues".to_string());
        for diagnostic in control.diagnostics.iter().take(MAX_STATUS_DETAILS) {
            let severity = match diagnostic.severity {
                ExternalSourceDiagnosticSeverity::Info => "info",
                ExternalSourceDiagnosticSeverity::Warning => "warning",
                ExternalSourceDiagnosticSeverity::Error => "error",
                _ => "notice",
            };
            lines.push(format!(
                "  - {severity}: [{}] {}",
                diagnostic.code,
                external_source_diagnostic_summary(&diagnostic.code)
            ));
        }
        let hidden = control.diagnostics.len().saturating_sub(MAX_STATUS_DETAILS);
        if hidden > 0 {
            lines.push(format!(
                "  - {hidden} more; refresh after fixing the listed issue(s)."
            ));
        }
    }
    if !control.recovery_actions.is_empty() {
        lines.push(String::new());
        lines.push("Recovery".to_string());
        for action in control.recovery_actions.iter().take(MAX_STATUS_DETAILS) {
            lines.push(format!(
                "  - {}",
                external_recovery_action_label(action, "extensions")
            ));
        }
    }
    lines.push(String::new());
    lines.push("Refresh: /extensions refresh".to_string());
    lines.push(if control.safe_mode {
        "Exit Safe Mode: /extensions safe-mode off".to_string()
    } else {
        "Enter Safe Mode: /extensions safe-mode on".to_string()
    });
    lines.push("Enable source: /extensions source enable <source-key>".to_string());
    lines.push("Disable source: /extensions source disable <source-key>".to_string());
    lines.join("\n")
}

struct ExternalControlMutationResult {
    action: ExternalControlUiAction,
    result: std::result::Result<
        (
            bitfun_core::external_sources::ExternalSourceSurfaceSnapshotV1,
            Option<ExternalSourceCatalogSnapshot>,
        ),
        ExternalSourceOperationError,
    >,
}

struct ExternalToolMutationResult {
    action: ExternalToolReviewAction,
    result: std::result::Result<ExternalSourceCatalogSnapshot, ExternalSourceOperationError>,
}

fn external_operation_error_status(surface: &str, error: &ExternalSourceOperationError) -> String {
    let reason = match error.code {
        ExternalSourceOperationErrorCode::InvalidRequest => {
            "The requested change is no longer valid."
        }
        ExternalSourceOperationErrorCode::HostUnavailable => "The workspace host is not available.",
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable => {
            "This workspace host is read-only for external integrations."
        }
        ExternalSourceOperationErrorCode::TrustRequired => {
            "This external integration requires review before it can run."
        }
        ExternalSourceOperationErrorCode::PolicyIncompatible => {
            "Compatibility settings were written by a newer BitFun version."
        }
        ExternalSourceOperationErrorCode::PolicyLimited => {
            "The current safety policy does not allow this change."
        }
        ExternalSourceOperationErrorCode::StaleRevision => {
            "Compatibility settings changed before the update completed."
        }
        ExternalSourceOperationErrorCode::Conflict => {
            "The available choices changed before the update completed."
        }
        ExternalSourceOperationErrorCode::NotFound => "That external item is no longer available.",
        ExternalSourceOperationErrorCode::Unavailable => {
            "The external integration is temporarily unavailable."
        }
        ExternalSourceOperationErrorCode::RuntimeUnavailable
        | ExternalSourceOperationErrorCode::DependencyFailed
        | ExternalSourceOperationErrorCode::ProcessLost => {
            "The external integration runtime is unavailable."
        }
        ExternalSourceOperationErrorCode::Unsupported
        | ExternalSourceOperationErrorCode::IncompatibleVersion => {
            "This external integration is not supported by the current BitFun version."
        }
        ExternalSourceOperationErrorCode::Timeout
        | ExternalSourceOperationErrorCode::Overloaded
        | ExternalSourceOperationErrorCode::TemporarilyUnavailable => {
            "The external integration is temporarily unavailable."
        }
        ExternalSourceOperationErrorCode::Cancelled => {
            "The external integration operation was cancelled."
        }
        ExternalSourceOperationErrorCode::InvalidResponse
        | ExternalSourceOperationErrorCode::Internal => {
            "BitFun could not complete the external integration update."
        }
    };
    let next_steps = error
        .recovery_actions
        .iter()
        .map(|action| external_recovery_action_label(action, surface))
        .collect::<Vec<_>>();
    let next_step = if next_steps.is_empty() {
        format!(" Run /{surface} refresh to review the current state.")
    } else {
        format!(" Next: {}.", next_steps.join("; "))
    };
    let reference = error
        .correlation_id
        .as_deref()
        .map(|id| format!(" Reference: {id}."))
        .unwrap_or_default();
    format!("{reason}{next_step}{reference}")
}

fn external_recovery_action_label(
    action: &ExternalSourceRecoveryActionV1,
    surface: &str,
) -> String {
    match action {
        ExternalSourceRecoveryActionV1::Refresh => format!("/{surface} refresh"),
        ExternalSourceRecoveryActionV1::Retry => "retry the operation".to_string(),
        ExternalSourceRecoveryActionV1::Review => "review the listed external items".to_string(),
        ExternalSourceRecoveryActionV1::ResolveConflict => {
            "resolve the listed conflict".to_string()
        }
        ExternalSourceRecoveryActionV1::InstallRuntime => {
            "install or repair the required runtime".to_string()
        }
        ExternalSourceRecoveryActionV1::ReconnectHost => {
            "reconnect or upgrade the execution Host".to_string()
        }
        ExternalSourceRecoveryActionV1::ExitSafeMode => "/extensions safe-mode off".to_string(),
    }
}

struct ExternalToolTargetSummary<'a> {
    tools: Vec<&'a ExternalToolCatalogEntry>,
}

impl<'a> ExternalToolTargetSummary<'a> {
    fn first(&self) -> &'a ExternalToolCatalogEntry {
        self.tools[0]
    }

    fn activation(&self) -> &'a ExternalToolActivationState {
        &self.first().activation
    }

    fn names(&self) -> String {
        let mut names = self
            .tools
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }
}

fn external_tool_target_summaries(
    snapshot: &ExternalSourceCatalogSnapshot,
) -> Vec<ExternalToolTargetSummary<'_>> {
    let mut summaries: Vec<ExternalToolTargetSummary<'_>> = Vec::new();
    for tool in &snapshot.tools {
        if let Some(summary) = summaries
            .iter_mut()
            .find(|summary| summary.first().definition.id.target == tool.definition.id.target)
        {
            summary.tools.push(tool);
        } else {
            summaries.push(ExternalToolTargetSummary { tools: vec![tool] });
        }
    }
    summaries
}

fn external_tool_activation_label(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => "confirmation required",
        ExternalToolActivationState::Declined => "kept disabled",
        ExternalToolActivationState::Disabled => "disabled",
        ExternalToolActivationState::Active => "enabled",
        ExternalToolActivationState::Conflict => "choose between same-name tools",
        ExternalToolActivationState::Unsupported { .. } => "not supported",
        ExternalToolActivationState::RuntimeUnavailable { .. } => "run environment unavailable",
        ExternalToolActivationState::LoadFailed { .. } => "could not load",
        _ => "unknown",
    }
}

fn external_tool_scope_label(scope: impl std::fmt::Debug) -> &'static str {
    match format!("{scope:?}").as_str() {
        "UserGlobal" => "all workspaces",
        "Project" | "WorkspaceLocal" => "current workspace",
        "RemoteUser" => "all remote workspaces",
        "RemoteProject" => "current remote workspace",
        _ => "unknown",
    }
}

fn external_tool_user_facing_reason(reason: &str) -> String {
    reason
        .replace("PR2 worker", "Tool process")
        .replace("PR2", "This version")
}

fn external_tool_reason(summary: &ExternalToolTargetSummary<'_>) -> Option<String> {
    match summary.activation() {
        ExternalToolActivationState::Unsupported { reason }
        | ExternalToolActivationState::RuntimeUnavailable { reason }
        | ExternalToolActivationState::LoadFailed { reason } => {
            Some(external_tool_user_facing_reason(reason))
        }
        _ => None,
    }
}

fn external_tool_next_step(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => {
            "Review the code source and access, then enable it or keep it disabled."
        }
        ExternalToolActivationState::Declined => {
            "Enable these tools after reviewing their code source and access."
        }
        ExternalToolActivationState::Disabled => {
            "Enable this source or its tool capability before using these tools."
        }
        ExternalToolActivationState::Active => {
            "No action is needed. Disable these tools to stop using this source's tools."
        }
        ExternalToolActivationState::Conflict => {
            "Choose which tool to use below, or leave this name disabled."
        }
        ExternalToolActivationState::Unsupported { .. } => {
            "Change the code to a single JavaScript file supported by BitFun, then refresh."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "Install or repair Node.js, then refresh. You can continue without external JavaScript tools while the run environment is unavailable."
        }
        ExternalToolActivationState::LoadFailed { .. } => {
            "Refresh to retry. If it still fails, fix the source code or keep these tools disabled."
        }
        _ => "Refresh to check the current state.",
    }
}

fn external_tool_default_reason(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => {
            "Review this tool file's access before enabling it."
        }
        ExternalToolActivationState::Declined => "You chose to keep these tools disabled.",
        ExternalToolActivationState::Disabled => {
            "The source or its tool capability is disabled by policy."
        }
        ExternalToolActivationState::Active => "The tool code is loaded and ready to use.",
        ExternalToolActivationState::Conflict => "Another tool uses the same name.",
        ExternalToolActivationState::Unsupported { .. } => {
            "This tool file contains code or operations that BitFun does not support."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "The required JavaScript run environment is unavailable."
        }
        ExternalToolActivationState::LoadFailed { .. } => "BitFun could not load this tool file.",
        _ => "The current state is unavailable.",
    }
}

fn external_tool_can_enable(activation: &ExternalToolActivationState) -> bool {
    matches!(
        activation,
        ExternalToolActivationState::ApprovalRequired | ExternalToolActivationState::Declined
    )
}

fn external_tool_can_disable(activation: &ExternalToolActivationState) -> bool {
    matches!(
        activation,
        ExternalToolActivationState::ApprovalRequired
            | ExternalToolActivationState::Active
            | ExternalToolActivationState::Conflict
            | ExternalToolActivationState::LoadFailed { .. }
    )
}

fn external_tool_result_is_stale(
    current: Option<&ExternalSourceCatalogSnapshot>,
    incoming: &ExternalSourceCatalogSnapshot,
) -> bool {
    current.is_some_and(|current| current.generation > incoming.generation)
}

fn external_tool_pending_notice_key(snapshot: &ExternalSourceCatalogSnapshot) -> Option<String> {
    let mut decisions = snapshot
        .tool_approval_requests
        .iter()
        .map(|request| format!("approval:{}", request.decision_key))
        .chain(
            snapshot
                .tool_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_none())
                .map(|conflict| format!("conflict:{}", conflict.conflict_key)),
        )
        .collect::<Vec<_>>();
    decisions.extend(snapshot.diagnostics.iter().filter_map(|diagnostic| {
        matches!(
            diagnostic.severity,
            ExternalSourceDiagnosticSeverity::Warning | ExternalSourceDiagnosticSeverity::Error
        )
        .then(|| {
            format!(
                "diagnostic:{:?}:{}:{}:{}",
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message,
                diagnostic
                    .source
                    .as_ref()
                    .map(|source| source.stable_key())
                    .unwrap_or_default()
            )
        })
    }));
    if decisions.is_empty() {
        return None;
    }
    decisions.sort_unstable();
    Some(decisions.join("\n"))
}

fn external_tool_capability_label(capability: ExternalToolCapability) -> &'static str {
    match capability {
        ExternalToolCapability::FileSystem => "filesystem",
        ExternalToolCapability::Network => "network",
        ExternalToolCapability::Process => "process",
        ExternalToolCapability::Environment => "environment variables",
        _ => "other",
    }
}

fn external_tool_runtime_label(runtime: ExternalToolRuntimeKind) -> &'static str {
    match runtime {
        ExternalToolRuntimeKind::JavaScript => "JavaScript",
        ExternalToolRuntimeKind::TypeScript => "TypeScript",
        _ => "unknown runtime",
    }
}

fn external_tool_review_text(snapshot: Option<&ExternalSourceCatalogSnapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Tools\n\nBitFun and MCP\nBuilt-in tools are provided by BitFun. Use /mcp to manage MCP servers.\n\nExternal AI applications\nBitFun has not finished checking imported tools. Run /tools refresh and try again."
            .to_string();
    };
    let mut lines = vec![
        "Tools".to_string(),
        String::new(),
        "BitFun and MCP".to_string(),
        "Built-in tools are provided by BitFun. Use /mcp to manage MCP servers.".to_string(),
        String::new(),
        "External AI applications".to_string(),
        "BitFun does not run external code while checking sources. Enabling tools runs their code with your user permissions and inherited environment variables. The code is not isolated by an OS sandbox, and processes it starts may keep running after cancellation."
            .to_string(),
    ];
    lines.push(String::new());
    lines.extend(external_integration_policy_lines(snapshot));

    if snapshot.discovery_pending {
        lines.push(String::new());
        lines.push(
            "BitFun is still checking for changes. Existing tools remain usable.".to_string(),
        );
    }

    lines.push(String::new());
    lines.push("Tool sources".to_string());
    let targets = external_tool_target_summaries(snapshot);
    if targets.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (index, target) in targets.iter().enumerate() {
            let tool = target.first();
            let source = snapshot
                .sources
                .iter()
                .find(|source| source.record.key == tool.definition.id.target.source);
            let capabilities = target
                .tools
                .iter()
                .flat_map(|tool| tool.definition.capabilities.iter().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(external_tool_capability_label)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "  {}. {} - {}",
                index + 1,
                target.names(),
                external_tool_activation_label(target.activation())
            ));
            lines.push(format!(
                "     Source folder: {}",
                source
                    .map(|source| source.record.location.as_str())
                    .unwrap_or("unknown")
            ));
            lines.push("     Code files:".to_string());
            let module_paths = target
                .tools
                .iter()
                .map(|tool| tool.definition.module_path.as_str())
                .collect::<BTreeSet<_>>();
            for module_path in module_paths {
                lines.push(format!("       - {module_path}"));
            }
            lines.push(format!(
                "     Applies to: {}",
                source
                    .map(|source| external_tool_scope_label(source.record.scope))
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Runs in: {}",
                source
                    .map(|source| external_tool_run_location_label(
                        source.record.execution_domain_id.as_str(),
                    ))
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Starts in folder: {}",
                tool.definition.working_directory
            ));
            lines.push(format!(
                "     Runs with: {}",
                external_tool_runtime_label(tool.definition.runtime_kind)
            ));
            lines.push(format!("     Access: {capabilities}"));
            if let Some(reason) = external_tool_reason(target) {
                lines.push(format!("     Reason: {reason}"));
            } else {
                lines.push(format!(
                    "     Reason: {}",
                    external_tool_default_reason(target.activation())
                ));
            }
            lines.push(format!(
                "     Next step: {}",
                external_tool_next_step(target.activation())
            ));
            let mut commands = Vec::new();
            if external_tool_can_enable(target.activation()) {
                commands.push(format!("/tools enable {}", index + 1));
            }
            if external_tool_can_disable(target.activation()) {
                commands.push(format!("/tools disable {}", index + 1));
            }
            if !commands.is_empty() {
                lines.push(format!("     Commands: {}", commands.join("  or  ")));
            }
        }
    }

    lines.push(String::new());
    lines.push("Name conflicts - needs a choice".to_string());
    let conflicts = snapshot
        .tool_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .chain(
            snapshot
                .tool_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_some()),
        )
        .collect::<Vec<_>>();
    let pending_count = conflicts
        .iter()
        .take_while(|conflict| conflict.selected_candidate_id.is_none())
        .count();
    let pending_conflicts = &conflicts[..pending_count];
    if pending_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (conflict_index, conflict) in pending_conflicts.iter().enumerate() {
            lines.push(format!(
                "  {}. Multiple tools are named '{}':",
                conflict_index + 1,
                conflict.tool_name
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                lines.push(format!(
                    "     {}. {} - /tools choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            lines.push(
                "     Choose which tool BitFun should use for this name. The choice is remembered until one of these tools changes."
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push("Current choices".to_string());
    let resolved_conflicts = &conflicts[pending_count..];
    if resolved_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (resolved_index, conflict) in resolved_conflicts.iter().enumerate() {
            let conflict_index = pending_count + resolved_index;
            lines.push(format!(
                "  {}. Tools named '{}':",
                conflict_index + 1,
                conflict.tool_name
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let status = if conflict.selected_candidate_id.as_deref()
                    == Some(candidate.candidate_id.as_str())
                {
                    let selected_external_unavailable = candidate.source.is_some()
                        && !snapshot.tools.iter().any(|tool| {
                            tool.definition.candidate_id() == candidate.candidate_id
                                && tool.activation == ExternalToolActivationState::Active
                        });
                    if selected_external_unavailable {
                        "selected, currently unavailable"
                    } else {
                        "selected"
                    }
                } else {
                    "not selected"
                };
                lines.push(format!(
                    "     {}. {} [{}] - /tools choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    status,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            lines.push(
                "     This choice is remembered until one of these tools changes. Choose another entry above to change it."
                    .to_string(),
            );
        }
    }

    append_external_source_issues(&mut lines, snapshot, ExternalIssueSurface::Tools);

    lines.push(String::new());
    lines.push(
        "Use /tools refresh after editing, upgrading, or removing tools from an external AI application."
            .to_string(),
    );
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalAgentReviewAction {
    Show,
    Refresh,
    Decide {
        candidate_id: String,
        decision_key: String,
        approved: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
    Choose {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    },
}

struct ExternalAgentMutationResult {
    action: ExternalAgentReviewAction,
    result: std::result::Result<ExternalSourceCatalogSnapshot, ExternalSourceOperationError>,
}

fn external_tool_run_location_label(execution_domain_id: &str) -> &'static str {
    if execution_domain_id.starts_with("local") {
        "this computer"
    } else if execution_domain_id.starts_with("remote") {
        "current remote environment"
    } else {
        "unknown"
    }
}

fn external_source_diagnostic_summary(code: &str) -> &'static str {
    if code.contains("preference_read_failed") {
        "BitFun could not verify saved tool confirmations. Affected tools remain disabled; check BitFun settings storage, then refresh."
    } else if code.contains("conflict_history_write_failed") {
        "BitFun could not save conflict information. Affected names remain unavailable; check BitFun settings storage, then refresh."
    } else if code.contains("discovery_in_progress") {
        "One source is still being checked. Existing content remains available."
    } else if code.contains("timeout") {
        "Checking one source took too long. Other content remains available; refresh to try again."
    } else if code.contains("trust_required") {
        "A source needs your confirmation before BitFun can use it."
    } else if code.contains("too_large")
        || code.contains("file_limit")
        || code.contains("bytes_limit")
    {
        "Some files were skipped because the source is too large. Reduce its size, then refresh."
    } else if code.contains("invalid")
        || code.contains("parse")
        || code.contains("definition")
        || code.contains("export_missing")
        || code.contains("name_unsupported")
    {
        "Some settings could not be read and were skipped. Fix the source, then refresh."
    } else if code.contains("unreadable")
        || code.contains("read_failed")
        || code.contains("metadata_failed")
        || code.contains("directory_")
    {
        "BitFun could not read part of a source. Check file access, then refresh."
    } else if code.contains("projection_only")
        || code.contains("unsupported")
        || code.contains("restricted")
    {
        "This type of external content is not supported yet, so BitFun did not load or run it."
    } else if code.contains("failed") {
        "BitFun could not check one source. Other sources remain available; refresh to retry."
    } else {
        "BitFun found an issue in one source. The affected content was not enabled."
    }
}

#[derive(Clone, Copy)]
enum ExternalIssueSurface {
    Tools,
    Agents,
}

fn is_external_agent_diagnostic(
    diagnostic: &bitfun_core::external_sources::ExternalSourceDiagnostic,
) -> bool {
    matches!(diagnostic.asset_kind, ExternalSourceAssetKind::Subagent)
}

fn append_external_source_issues(
    lines: &mut Vec<String>,
    snapshot: &ExternalSourceCatalogSnapshot,
    surface: ExternalIssueSurface,
) {
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| match surface {
            ExternalIssueSurface::Tools => !is_external_agent_diagnostic(diagnostic),
            ExternalIssueSurface::Agents => is_external_agent_diagnostic(diagnostic),
        })
        .collect::<Vec<_>>();
    lines.push(String::new());
    lines.push("Issues".to_string());
    if diagnostics.is_empty() {
        lines.push("  None".to_string());
        return;
    }
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            ExternalSourceDiagnosticSeverity::Info => "info",
            ExternalSourceDiagnosticSeverity::Warning => "warning",
            ExternalSourceDiagnosticSeverity::Error => "error",
            _ => "notice",
        };
        let source = diagnostic
            .source
            .as_ref()
            .and_then(|key| {
                snapshot
                    .sources
                    .iter()
                    .find(|source| source.record.key == *key)
            })
            .map(|source| source.record.display_name.as_str());
        lines.push(format!(
            "  - {severity}: {}",
            external_source_diagnostic_summary(&diagnostic.code)
        ));
        if let Some(source) = source {
            lines.push(format!("    Affected source: {source}"));
        }
        lines.push(format!(
            "    Technical details: [{}] {}",
            diagnostic.code,
            external_tool_user_facing_reason(&diagnostic.message)
        ));
    }
}

const DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE: &str = "__bitfun_disabled__";

fn external_agent_activation_label(state: &ExternalSubagentActivationState) -> &'static str {
    match state {
        ExternalSubagentActivationState::ApprovalRequired => "confirmation required",
        ExternalSubagentActivationState::Declined => "kept disabled",
        ExternalSubagentActivationState::Disabled => "disabled by source",
        ExternalSubagentActivationState::Active => "enabled",
        ExternalSubagentActivationState::Conflict => "choose between same-name agents",
        ExternalSubagentActivationState::Blocked => "not supported",
        ExternalSubagentActivationState::Unavailable => "temporarily unavailable",
    }
}

fn external_agent_compatibility_label(state: ExternalSubagentCompatibilityState) -> &'static str {
    match state {
        ExternalSubagentCompatibilityState::Ready => "supported",
        ExternalSubagentCompatibilityState::ReadyWithDegradation => {
            "supported, but some settings will not apply"
        }
        ExternalSubagentCompatibilityState::Blocked => "not supported",
        ExternalSubagentCompatibilityState::Invalid => "configuration error",
    }
}

fn external_agent_model_label(model: Option<&str>) -> &str {
    model.unwrap_or("unavailable")
}

fn external_agent_review_text(snapshot: Option<&ExternalSourceCatalogSnapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Agents\n\nExternal AI applications\nBitFun has not finished checking imported agents. Run /agent refresh and try again."
            .to_string();
    };
    let mut lines = vec![
        "Agents".to_string(),
        String::new(),
        "External AI applications".to_string(),
        "BitFun only reads supported settings while checking sources. Agent instructions stay hidden and are not added to the current agent. Once enabled, those instructions guide the selected model and may call the tools listed below. Before enabling, review the model, tools, and where the agent runs. BitFun asks again if the instructions, model, tools, or configuration sources change. Each use starts a new task; follow-up is not supported in this version."
            .to_string(),
    ];
    lines.push(String::new());
    lines.extend(external_integration_policy_lines(snapshot));
    if snapshot.discovery_pending {
        lines.push(String::new());
        lines.push(
            "BitFun is still checking for changes. Previously enabled agents remain usable."
                .to_string(),
        );
    }

    append_external_source_issues(&mut lines, snapshot, ExternalIssueSurface::Agents);

    lines.push(String::new());
    lines.push("Agents".to_string());
    if snapshot.subagents.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (index, agent) in snapshot.subagents.iter().enumerate() {
            lines.push(format!(
                "  {}. {} ({}) - {}",
                index + 1,
                agent.display_name,
                agent.logical_id,
                external_agent_activation_label(&agent.activation_state)
            ));
            lines.push(format!("     Source application: {}", agent.provider_label));
            lines.push(format!(
                "     Applies to: {}",
                external_tool_scope_label(agent.scope)
            ));
            if !agent.source_location_labels.is_empty() {
                lines.push(format!(
                    "     Configuration sources: {}",
                    agent.source_location_labels.join(", ")
                ));
            }
            lines.push(format!(
                "     Model: {}",
                external_agent_model_label(agent.effective_model_label.as_deref())
            ));
            lines.push(format!(
                "     Tools: {}",
                if agent.effective_tool_labels.is_empty() {
                    "none".to_string()
                } else {
                    agent.effective_tool_labels.join(", ")
                }
            ));
            lines.push(format!(
                "     Support: {}",
                external_agent_compatibility_label(agent.compatibility_state)
            ));
            lines.push("     Run behavior: one run only; no follow-up".to_string());
            lines.push("     Runs on: this computer in the current workspace".to_string());
            if !agent.diagnostics.is_empty() {
                lines.push("     Compatibility notes:".to_string());
                for diagnostic in &agent.diagnostics {
                    lines.extend(external_agent_diagnostic_lines(
                        &diagnostic.code,
                        diagnostic.blocks_activation,
                        "       ",
                    ));
                }
            }
            match agent.activation_state {
                ExternalSubagentActivationState::ApprovalRequired
                | ExternalSubagentActivationState::Declined => {
                    lines.push(format!("     Command: /agent enable {}", index + 1))
                }
                ExternalSubagentActivationState::Active => {
                    lines.push(format!("     Command: /agent disable {}", index + 1))
                }
                _ => {}
            }
        }
    }

    lines.push(String::new());
    lines.push("Name conflicts - needs a choice".to_string());
    let conflicts = snapshot
        .subagent_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .chain(
            snapshot
                .subagent_conflicts
                .iter()
                .filter(|conflict| conflict.selected_candidate_id.is_some()),
        )
        .collect::<Vec<_>>();
    let pending_count = conflicts
        .iter()
        .take_while(|conflict| conflict.selected_candidate_id.is_none())
        .count();
    let pending_conflicts = &conflicts[..pending_count];
    if pending_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (conflict_index, conflict) in pending_conflicts.iter().enumerate() {
            lines.push(format!(
                "  {}. Multiple agents are named '{}'. Choose one:",
                conflict_index + 1,
                conflict.logical_id
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let kind = if candidate.external {
                    "external"
                } else {
                    "BitFun/local"
                };
                lines.push(format!(
                    "     {}. {} ({}, {}) - /agent choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    candidate.source_label,
                    kind,
                    conflict_index + 1,
                    candidate_index + 1
                ));
                if candidate.external {
                    if let Some(agent) = snapshot
                        .subagents
                        .iter()
                        .find(|agent| agent.candidate_id == candidate.candidate_id)
                    {
                        lines.push(format!(
                            "        Model: {}",
                            external_agent_model_label(agent.effective_model_label.as_deref())
                        ));
                        lines.push(format!(
                            "        Tools: {}",
                            if agent.effective_tool_labels.is_empty() {
                                "none".to_string()
                            } else {
                                agent.effective_tool_labels.join(", ")
                            }
                        ));
                        lines.push(
                            "        Runs on: this computer in the current workspace".to_string(),
                        );
                        lines.push(format!(
                            "        Support: {}",
                            external_agent_compatibility_label(agent.compatibility_state)
                        ));
                        for location in &agent.source_location_labels {
                            lines.push(format!("        Source: {location}"));
                        }
                        for diagnostic in &agent.diagnostics {
                            lines.extend(external_agent_diagnostic_lines(
                                &diagnostic.code,
                                diagnostic.blocks_activation,
                                "        ",
                            ));
                        }
                        lines.push(
                            "        This choice also confirms the model, tools, run location, and configuration sources shown above."
                                .to_string(),
                        );
                    }
                }
            }
            lines.push(format!(
                "     Keep unavailable: /agent choose {} 0",
                conflict_index + 1
            ));
            lines.push(
                "     The choice is remembered until one of these agents changes.".to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push("Current choices".to_string());
    let resolved_conflicts = &conflicts[pending_count..];
    if resolved_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (resolved_index, conflict) in resolved_conflicts.iter().enumerate() {
            let conflict_index = pending_count + resolved_index;
            lines.push(format!(
                "  {}. Agents named '{}':",
                conflict_index + 1,
                conflict.logical_id
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                let kind = if candidate.external {
                    "external"
                } else {
                    "BitFun/local"
                };
                let status = if conflict.selected_candidate_id.as_deref()
                    == Some(candidate.candidate_id.as_str())
                {
                    if candidate.external
                        && snapshot.subagents.iter().any(|agent| {
                            agent.candidate_id == candidate.candidate_id
                                && agent.activation_state != ExternalSubagentActivationState::Active
                        })
                    {
                        "selected, currently unavailable"
                    } else {
                        "selected"
                    }
                } else {
                    "not selected"
                };
                lines.push(format!(
                    "     {}. {} ({}, {}) [{}] - /agent choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    candidate.source_label,
                    kind,
                    status,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            let disabled = conflict.selected_candidate_id.as_deref()
                == Some(DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE);
            lines.push(format!(
                "     Keep unavailable{}: /agent choose {} 0",
                if disabled {
                    " [selected]"
                } else {
                    " [not selected]"
                },
                conflict_index + 1
            ));
            lines.push(
                "     This choice is remembered until one of these agents changes. Choose another entry above to change it."
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push(
        "Run /agent refresh after editing, upgrading, or removing agent configuration in an external AI application."
            .to_string(),
    );
    lines.join("\n")
}

fn external_agent_diagnostic_lines(
    code: &str,
    blocks_activation: bool,
    indent: &str,
) -> Vec<String> {
    let (reason, next_step) = if code.contains("configuration_unavailable") {
        (
            "BitFun could not read its model settings.",
            "Open BitFun model settings, check that BitFun can read and save its settings, then refresh.",
        )
    } else if code.contains("model_unavailable") {
        (
            "The requested model is not available in BitFun.",
            "Choose an available model in the source application, or set a fixed Sub-Agent model in BitFun, then refresh.",
        )
    } else if code.contains("tool_unavailable") {
        (
            "One or more requested tools are not available in BitFun.",
            "Remove or replace the unsupported tools in the source application, then refresh.",
        )
    } else if code.contains("type_invalid")
        || code.contains("definition_invalid")
        || code.ends_with("_invalid")
    {
        (
            "The agent settings have an invalid or missing required value.",
            "Correct the agent settings in the source application, then refresh.",
        )
    } else if blocks_activation {
        (
            "This agent requires behavior or settings that BitFun does not support.",
            "Update the agent in the source application to use supported settings and include all required content, then refresh.",
        )
    } else {
        (
            "BitFun does not use this setting.",
            "Before enabling, review the model and tools that will actually be used, and confirm that this setting will not apply.",
        )
    };
    let impact = if blocks_activation {
        "This agent cannot be enabled."
    } else {
        "Some settings will not apply. Review the resulting behavior before enabling."
    };
    vec![
        format!("{indent}Reason: {reason}"),
        format!("{indent}Impact: {impact}"),
        format!("{indent}Next step: {next_step}"),
        format!("{indent}Technical code: {code}"),
    ]
}

fn external_agent_result_is_stale(
    current: Option<&ExternalSourceCatalogSnapshot>,
    result: &ExternalSourceCatalogSnapshot,
) -> bool {
    current.is_some_and(|current| {
        current.subagent_generation > result.subagent_generation
            || current.preference_revision > result.preference_revision
    })
}

fn merge_external_agent_mutation_snapshot(
    current: Option<&ExternalSourceCatalogSnapshot>,
    mut result: ExternalSourceCatalogSnapshot,
) -> ExternalSourceCatalogSnapshot {
    let Some(current) = current else {
        return result;
    };
    if current.generation <= result.generation {
        return result;
    }

    // Agent decisions have an independent generation/revision. Preserve a
    // newer unrelated command/tool catalog while applying only the returned
    // agent partition, so a successful review action cannot roll the TUI back.
    let mut merged = current.clone();
    merged.subagent_generation = result.subagent_generation;
    merged.preference_revision = result.preference_revision;
    merged.subagents = std::mem::take(&mut result.subagents);
    merged.subagent_conflicts = std::mem::take(&mut result.subagent_conflicts);
    merged.pending_subagent_approvals = std::mem::take(&mut result.pending_subagent_approvals);
    merged
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalAgentAttention {
    confirmations: usize,
    conflicts: usize,
    unavailable: usize,
    diagnostics: usize,
    key: Option<String>,
}

fn external_agent_attention(
    previous: Option<&ExternalSourceCatalogSnapshot>,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> ExternalAgentAttention {
    let unresolved = snapshot
        .subagent_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .map(|conflict| conflict.conflict_key.clone())
        .collect::<Vec<_>>();
    let pending_decisions = snapshot
        .pending_subagent_approvals
        .iter()
        .map(|candidate_id| {
            snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == *candidate_id)
                .map(|agent| format!("{}:{}", agent.candidate_id, agent.decision_key))
                .unwrap_or_else(|| candidate_id.clone())
        })
        .collect::<Vec<_>>();
    let unavailable = previous
        .into_iter()
        .flat_map(|previous| previous.subagents.iter())
        .filter(|agent| agent.activation_state == ExternalSubagentActivationState::Active)
        .filter_map(|previous_agent| {
            match snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == previous_agent.candidate_id)
                .map(|agent| &agent.activation_state)
            {
                None => Some(format!("removed:{}", previous_agent.candidate_id)),
                Some(ExternalSubagentActivationState::Active) => None,
                Some(state) => Some(format!("{state:?}:{}", previous_agent.candidate_id)),
            }
        })
        .collect::<BTreeSet<_>>();
    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.severity,
                ExternalSourceDiagnosticSeverity::Warning | ExternalSourceDiagnosticSeverity::Error
            ) && is_external_agent_diagnostic(diagnostic)
        })
        .map(|diagnostic| {
            format!(
                "{:?}:{}:{}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            )
        })
        .collect::<Vec<_>>();
    let confirmations = snapshot.pending_subagent_approvals.len();
    let conflicts = unresolved.len();
    let unavailable_count = unavailable.len();
    let diagnostic_count = diagnostics.len();
    let key = if confirmations + conflicts + unavailable_count + diagnostic_count == 0 {
        None
    } else {
        Some(format!(
            "approvals={};conflicts={};unavailable={};diagnostics={}",
            pending_decisions.join(","),
            unresolved.join(","),
            unavailable.into_iter().collect::<Vec<_>>().join(","),
            diagnostics.join(",")
        ))
    };
    ExternalAgentAttention {
        confirmations,
        conflicts,
        unavailable: unavailable_count,
        diagnostics: diagnostic_count,
        key,
    }
}

fn external_agent_pending_notice_key(
    previous: Option<&ExternalSourceCatalogSnapshot>,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> Option<String> {
    external_agent_attention(previous, snapshot).key
}

fn parse_external_agent_review_action(
    arguments: &str,
    current_snapshot: Option<&ExternalSourceCatalogSnapshot>,
    reviewed_snapshot: Option<&ExternalSourceCatalogSnapshot>,
) -> Result<ExternalAgentReviewAction, String> {
    let mut parts = arguments.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(ExternalAgentReviewAction::Show);
    };
    if command.eq_ignore_ascii_case("refresh") {
        if parts.next().is_some() {
            return Err("usage: /agent refresh".to_string());
        }
        return Ok(ExternalAgentReviewAction::Refresh);
    }
    if command.eq_ignore_ascii_case("help") {
        return Ok(ExternalAgentReviewAction::Show);
    }
    let snapshot = reviewed_snapshot.or(current_snapshot).ok_or_else(|| {
        "BitFun has not finished checking agents from external AI applications; run /agent refresh".to_string()
    })?;
    if command.eq_ignore_ascii_case("enable") || command.eq_ignore_ascii_case("disable") {
        let index = parse_positive_index(parts.next(), "agent number")?;
        if parts.next().is_some() {
            return Err(format!("usage: /agent {command} <agent-number>"));
        }
        let agent = snapshot
            .subagents
            .get(index)
            .ok_or_else(|| "that agent is no longer available; run /agent refresh".to_string())?;
        let approved = command.eq_ignore_ascii_case("enable");
        let allowed = if approved {
            matches!(
                agent.activation_state,
                ExternalSubagentActivationState::ApprovalRequired
                    | ExternalSubagentActivationState::Declined
            )
        } else {
            matches!(
                agent.activation_state,
                ExternalSubagentActivationState::Active
            )
        };
        if !allowed {
            return Err(format!(
                "agent {} is {}; run /agent refresh for its next step",
                index + 1,
                external_agent_activation_label(&agent.activation_state)
            ));
        }
        return Ok(ExternalAgentReviewAction::Decide {
            candidate_id: agent.candidate_id.clone(),
            decision_key: agent.decision_key.clone(),
            approved,
            expected_subagent_generation: snapshot.subagent_generation,
            expected_preference_revision: snapshot.preference_revision,
        });
    }
    if command.eq_ignore_ascii_case("choose") {
        let conflict_index = parse_positive_index(parts.next(), "conflict number")?;
        let raw_candidate = parts
            .next()
            .ok_or_else(|| "missing choice number".to_string())?;
        let candidate_number = raw_candidate
            .parse::<usize>()
            .map_err(|_| "choice number must be zero or a positive number".to_string())?;
        if parts.next().is_some() {
            return Err("usage: /agent choose <conflict-number> <choice-number>".to_string());
        }
        let conflict = snapshot
            .subagent_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .chain(
                snapshot
                    .subagent_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_some()),
            )
            .nth(conflict_index)
            .ok_or_else(|| {
                "that conflict is no longer available; run /agent refresh".to_string()
            })?;
        let (candidate_id, approve_external) = if candidate_number == 0 {
            (DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE.to_string(), false)
        } else {
            let candidate = conflict
                .candidates
                .get(candidate_number - 1)
                .ok_or_else(|| {
                    "that choice is no longer available; run /agent refresh".to_string()
                })?;
            (candidate.candidate_id.clone(), candidate.external)
        };
        return Ok(ExternalAgentReviewAction::Choose {
            conflict_key: conflict.conflict_key.clone(),
            candidate_id,
            approve_external,
            expected_subagent_generation: snapshot.subagent_generation,
            expected_preference_revision: snapshot.preference_revision,
        });
    }
    Err("usage: /agent [refresh | enable <number> | disable <number> | choose <conflict-number> <choice-number>]".to_string())
}

fn external_agent_mutation_result_label(
    action: &ExternalAgentReviewAction,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> String {
    match action {
        ExternalAgentReviewAction::Refresh => "External agents refreshed".to_string(),
        ExternalAgentReviewAction::Decide {
            candidate_id,
            approved,
            ..
        } => {
            let active = snapshot
                .subagents
                .iter()
                .find(|agent| agent.candidate_id == *candidate_id);
            match (approved, active.map(|agent| &agent.activation_state)) {
                (true, Some(ExternalSubagentActivationState::Active)) => {
                    "External agent enabled".to_string()
                }
                (false, Some(ExternalSubagentActivationState::Declined)) => {
                    "External agent disabled".to_string()
                }
                _ => {
                    "External agent decision saved; run /agent refresh to review its current state"
                        .to_string()
                }
            }
        }
        ExternalAgentReviewAction::Choose {
            conflict_key,
            candidate_id,
            ..
        } => {
            let selected = snapshot.subagent_conflicts.iter().any(|conflict| {
                conflict.conflict_key == *conflict_key
                    && conflict.selected_candidate_id.as_deref() == Some(candidate_id.as_str())
            });
            if selected {
                if candidate_id == DISABLED_EXTERNAL_AGENT_CONFLICT_CHOICE {
                    "Conflicting agent kept unavailable".to_string()
                } else {
                    "Agent source selected".to_string()
                }
            } else {
                "Agent choices changed; run /agent refresh before choosing".to_string()
            }
        }
        ExternalAgentReviewAction::Show => "External agents".to_string(),
    }
}
