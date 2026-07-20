// Pure projections and review text derived from the external-source catalog.
fn native_command_conflict_key<'a>(
    execution_domain_id: &str,
    command_name: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    format!(
        "native:{}",
        prompt_command_conflict_key(execution_domain_id, command_name, candidates)
    )
}

fn external_command_projections(
    snapshot: &ExternalSourceCatalogSnapshot,
    conflict_choices: &BTreeMap<String, String>,
) -> Vec<ExternalCommandProjection> {
    let built_in_actions = slash_actions(ActionState::chat(false, false));
    let mut projections = snapshot
        .commands
        .iter()
        .map(|entry| {
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
                let conflict_key = native_command_conflict_key(
                    source.record.execution_domain_id.as_str(),
                    &entry.definition.name,
                    [
                        (native_candidate_id.as_str(), env!("CARGO_PKG_VERSION")),
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
            ExternalCommandProjection {
                action_id: format!("external-command:{}", entry.definition.name),
                command_name: entry.definition.name.clone(),
                invocation_alias: format!("/{}", entry.definition.name),
                candidate_id: entry.definition.id.stable_key(),
                content_version: entry.definition.content_version.clone(),
                description: format!("{} · {}", entry.definition.description, ecosystem),
                restricted,
                provider_conflict_key: None,
                native_collision,
            }
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
            candidates.push((native_candidate_id.as_str(), env!("CARGO_PKG_VERSION")));
            let conflict_key =
                native_command_conflict_key(execution_domain, &conflict.command_name, candidates);
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
                invocation_alias: format!(
                    "/external:{}:{}",
                    candidate.source.provider_id, conflict.command_name
                ),
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

struct ExternalToolMutationResult {
    action: ExternalToolReviewAction,
    result: std::result::Result<ExternalSourceCatalogSnapshot, String>,
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
        ExternalToolActivationState::ApprovalRequired => "approval required",
        ExternalToolActivationState::Disabled => "disabled",
        ExternalToolActivationState::Active => "active",
        ExternalToolActivationState::Conflict => "name conflict",
        ExternalToolActivationState::Unsupported { .. } => "unsupported",
        ExternalToolActivationState::RuntimeUnavailable { .. } => "runtime unavailable",
        ExternalToolActivationState::LoadFailed { .. } => "load failed",
        _ => "unknown",
    }
}

fn external_tool_scope_label(scope: impl std::fmt::Debug) -> &'static str {
    match format!("{scope:?}").as_str() {
        "UserGlobal" => "user global",
        "Project" => "project",
        "WorkspaceLocal" => "workspace local",
        "RemoteUser" => "remote user",
        "RemoteProject" => "remote project",
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
            "Review access, then enable the target or keep it disabled."
        }
        ExternalToolActivationState::Disabled => {
            "Enable the target after reviewing its source and access."
        }
        ExternalToolActivationState::Active => {
            "No action is required. Disable the target to stop exposing it."
        }
        ExternalToolActivationState::Conflict => "Choose a provider below, or disable this target.",
        ExternalToolActivationState::Unsupported { .. } => {
            "Convert the module to the supported standalone JavaScript subset, then refresh."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "Restore the required runtime, then refresh."
        }
        ExternalToolActivationState::LoadFailed { .. } => {
            "Refresh to retry. If it still fails, inspect or update the module, or disable this target."
        }
        _ => "Refresh to retrieve the current target state.",
    }
}

fn external_tool_default_reason(activation: &ExternalToolActivationState) -> &'static str {
    match activation {
        ExternalToolActivationState::ApprovalRequired => {
            "The module needs approval before BitFun loads it."
        }
        ExternalToolActivationState::Disabled => "The target was disabled by user choice.",
        ExternalToolActivationState::Active => {
            "The module loaded successfully and is exposed in this execution domain."
        }
        ExternalToolActivationState::Conflict => "Another implementation uses the same tool name.",
        ExternalToolActivationState::Unsupported { .. } => {
            "The module uses unsupported syntax or behavior."
        }
        ExternalToolActivationState::RuntimeUnavailable { .. } => {
            "The required JavaScript runtime is unavailable."
        }
        ExternalToolActivationState::LoadFailed { .. } => {
            "The tool process could not load this module."
        }
        _ => "The current state is unavailable.",
    }
}

fn external_tool_can_enable(activation: &ExternalToolActivationState) -> bool {
    matches!(
        activation,
        ExternalToolActivationState::ApprovalRequired | ExternalToolActivationState::Disabled
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
        return "External tools\n\nTool discovery has not completed. Run /external-tools refresh and try again."
            .to_string();
    };
    let mut lines = vec![
        "External tools".to_string(),
        String::new(),
        "No external code ran during discovery. Enabling a tool starts its external module with your user permissions and inherited environment variables; BitFun does not provide an OS sandbox or full descendant-process cleanup in this version."
            .to_string(),
    ];

    if snapshot.discovery_pending {
        lines.push(String::new());
        lines.push("Discovery is still running. Existing results remain usable.".to_string());
    }

    lines.push(String::new());
    lines.push("Targets".to_string());
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
                "     Source root: {}",
                source
                    .map(|source| source.record.location.as_str())
                    .unwrap_or("unknown")
            ));
            lines.push("     Module files:".to_string());
            let module_paths = target
                .tools
                .iter()
                .map(|tool| tool.definition.module_path.as_str())
                .collect::<BTreeSet<_>>();
            for module_path in module_paths {
                lines.push(format!("       - {module_path}"));
            }
            lines.push(format!(
                "     Scope: {}",
                source
                    .map(|source| external_tool_scope_label(source.record.scope))
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Execution domain: {}",
                source
                    .map(|source| source.record.execution_domain_id.as_str())
                    .unwrap_or("unknown")
            ));
            lines.push(format!(
                "     Working directory: {}",
                tool.definition.working_directory
            ));
            lines.push(format!(
                "     Runtime: {}",
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
                commands.push(format!("/external-tools enable {}", index + 1));
            }
            if external_tool_can_disable(target.activation()) {
                commands.push(format!("/external-tools disable {}", index + 1));
            }
            if !commands.is_empty() {
                lines.push(format!("     Commands: {}", commands.join("  or  ")));
            }
        }
    }

    lines.push(String::new());
    lines.push("Name conflicts".to_string());
    let pending_conflicts = snapshot
        .tool_conflicts
        .iter()
        .filter(|conflict| conflict.selected_candidate_id.is_none())
        .collect::<Vec<_>>();
    if pending_conflicts.is_empty() {
        lines.push("  None".to_string());
    } else {
        for (conflict_index, conflict) in pending_conflicts.iter().enumerate() {
            lines.push(format!(
                "  {}. Tool '{}' requires a provider choice:",
                conflict_index + 1,
                conflict.tool_name
            ));
            for (candidate_index, candidate) in conflict.candidates.iter().enumerate() {
                lines.push(format!(
                    "     {}. {} ({}) - /external-tools choose {} {}",
                    candidate_index + 1,
                    candidate.display_name,
                    candidate.provider_id,
                    conflict_index + 1,
                    candidate_index + 1
                ));
            }
            lines.push(
                "     The choice applies to matching candidates in this execution domain."
                    .to_string(),
            );
        }
    }

    lines.push(String::new());
    lines.push("Diagnostics".to_string());
    if snapshot.diagnostics.is_empty() {
        lines.push("  None".to_string());
    } else {
        for diagnostic in &snapshot.diagnostics {
            let severity = match diagnostic.severity {
                ExternalSourceDiagnosticSeverity::Info => "info",
                ExternalSourceDiagnosticSeverity::Warning => "warning",
                ExternalSourceDiagnosticSeverity::Error => "error",
                _ => "notice",
            };
            let source = diagnostic
                .source
                .as_ref()
                .map(|source| format!(" [{}]", source.stable_key()))
                .unwrap_or_default();
            lines.push(format!(
                "  - {severity} [{}]{source}: {}",
                diagnostic.code,
                external_tool_user_facing_reason(&diagnostic.message)
            ));
        }
    }

    lines.push(String::new());
    lines.push(
        "Use /external-tools refresh after editing, upgrading, or removing external tools."
            .to_string(),
    );
    lines.join("\n")
}
