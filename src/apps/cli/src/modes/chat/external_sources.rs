fn parse_positive_index(value: Option<&str>, label: &str) -> Result<usize, String> {
    let raw = value.ok_or_else(|| format!("missing {label}"))?;
    let index = raw
        .parse::<usize>()
        .map_err(|_| format!("{label} must be a positive number"))?;
    if index == 0 {
        return Err(format!("{label} must be a positive number"));
    }
    Ok(index - 1)
}

fn parse_external_tool_review_action(
    arguments: &str,
    current_snapshot: Option<&ExternalSourceCatalogSnapshot>,
    reviewed_snapshot: Option<&ExternalSourceCatalogSnapshot>,
) -> Result<ExternalToolReviewAction, String> {
    let mut parts = arguments.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(ExternalToolReviewAction::Show);
    };
    if command.eq_ignore_ascii_case("refresh") {
        if parts.next().is_some() {
            return Err("usage: /external-tools refresh".to_string());
        }
        return Ok(ExternalToolReviewAction::Refresh);
    }
    if command.eq_ignore_ascii_case("help") {
        return Ok(ExternalToolReviewAction::Show);
    }
    // Numbered commands refer to the immutable catalog that produced the
    // review popup. The backend still validates stable decision/conflict keys,
    // so a changed target fails closed instead of reusing the same number for
    // a different tool after a watcher refresh.
    let snapshot = reviewed_snapshot.or(current_snapshot).ok_or_else(|| {
        "BitFun has not finished checking external tools; run /external-tools refresh".to_string()
    })?;
    if command.eq_ignore_ascii_case("enable") || command.eq_ignore_ascii_case("disable") {
        let index = parse_positive_index(parts.next(), "tool number")?;
        if parts.next().is_some() {
            return Err(format!("usage: /external-tools {command} <tool-number>"));
        }
        let targets = external_tool_target_summaries(snapshot);
        let target = targets.get(index).ok_or_else(|| {
            "that tool is no longer available; reopen /external-tools".to_string()
        })?;
        let approved = command.eq_ignore_ascii_case("enable");
        let allowed = if approved {
            external_tool_can_enable(target.activation())
        } else {
            external_tool_can_disable(target.activation())
        };
        if !allowed {
            return Err(format!(
                "tool {} is {}; reopen /external-tools for its next step",
                index + 1,
                external_tool_activation_label(target.activation())
            ));
        }
        let tool = target.first();
        return Ok(ExternalToolReviewAction::Decide {
            approval_key: tool.approval_key.clone(),
            decision_key: tool.decision_key.clone(),
            approved,
        });
    }
    if command.eq_ignore_ascii_case("choose") {
        let conflict_index = parse_positive_index(parts.next(), "conflict number")?;
        let candidate_index = parse_positive_index(parts.next(), "choice number")?;
        if parts.next().is_some() {
            return Err(
                "usage: /external-tools choose <conflict-number> <choice-number>".to_string(),
            );
        }
        let conflict = snapshot
            .tool_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .nth(conflict_index)
            .ok_or_else(|| {
                "that conflict is no longer available; reopen /external-tools".to_string()
            })?;
        let candidate = conflict.candidates.get(candidate_index).ok_or_else(|| {
            "that choice is no longer available; reopen /external-tools".to_string()
        })?;
        return Ok(ExternalToolReviewAction::Choose {
            conflict_key: conflict.conflict_key.clone(),
            candidate_id: candidate.candidate_id.clone(),
        });
    }
    Err("usage: /external-tools [refresh | enable <number> | disable <number> | choose <conflict-number> <choice-number>]".to_string())
}

fn external_tool_mutation_result_label(
    action: &ExternalToolReviewAction,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> String {
    match action {
        ExternalToolReviewAction::Refresh => "External tools refreshed".to_string(),
        ExternalToolReviewAction::Decide {
            approval_key,
            decision_key,
            approved: true,
        } => {
            let activations = snapshot
                .tools
                .iter()
                .filter(|tool| {
                    tool.approval_key == *approval_key && tool.decision_key == *decision_key
                })
                .map(|tool| &tool.activation)
                .collect::<Vec<_>>();
            if activations.is_empty() {
                "External tool confirmation saved; reopen /external-tools to review the changed tool"
                    .to_string()
            } else if activations
                .iter()
                .any(|state| matches!(state, ExternalToolActivationState::LoadFailed { .. }))
            {
                "External tool enabled, but loading failed".to_string()
            } else if activations.iter().any(|state| {
                matches!(
                    state,
                    ExternalToolActivationState::RuntimeUnavailable { .. }
                )
            }) {
                "External tool enabled, but its run environment is unavailable".to_string()
            } else if activations
                .iter()
                .any(|state| matches!(state, ExternalToolActivationState::Conflict))
            {
                "External tool enabled; choose a source before every tool is available".to_string()
            } else if activations
                .iter()
                .all(|state| matches!(state, ExternalToolActivationState::Active))
            {
                "External tool enabled".to_string()
            } else {
                "External tool confirmation saved; reopen /external-tools to review its current state"
                    .to_string()
            }
        }
        ExternalToolReviewAction::Decide {
            approval_key,
            decision_key,
            approved: false,
        } => {
            let disabled = snapshot.tools.iter().any(|tool| {
                tool.approval_key == *approval_key
                    && tool.decision_key == *decision_key
                    && matches!(tool.activation, ExternalToolActivationState::Disabled)
            });
            if disabled {
                "External tool disabled".to_string()
            } else {
                "External tool choice saved; reopen /external-tools to review the changed tool"
                    .to_string()
            }
        }
        ExternalToolReviewAction::Choose {
            conflict_key,
            candidate_id,
        } => {
            let selected = snapshot.tool_conflicts.iter().any(|conflict| {
                conflict.conflict_key == *conflict_key
                    && conflict.selected_candidate_id.as_deref() == Some(candidate_id.as_str())
            });
            if selected {
                "External tool source selected".to_string()
            } else {
                "External tool choices changed; reopen /external-tools before choosing".to_string()
            }
        }
        ExternalToolReviewAction::Show => "External tools".to_string(),
    }
}

struct BuiltinCommandReconfirmation {
    conflict_key: String,
    candidate_id: String,
    confirmed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExternalSourceConflictPreferences {
    choices: BTreeMap<String, String>,
    lineage_current_keys: BTreeMap<String, String>,
    conflicted_candidate_ids: BTreeSet<String>,
}

impl
    From<(
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        BTreeSet<String>,
    )> for ExternalSourceConflictPreferences
{
    fn from(
        (choices, lineage_current_keys, conflicted_candidate_ids): (
            BTreeMap<String, String>,
            BTreeMap<String, String>,
            BTreeSet<String>,
        ),
    ) -> Self {
        Self {
            choices,
            lineage_current_keys,
            conflicted_candidate_ids,
        }
    }
}

fn builtin_command_reconfirmation(
    action_id: &str,
    action_name: &str,
    preferences: &ExternalSourceConflictPreferences,
) -> Option<BuiltinCommandReconfirmation> {
    let candidate_id = format!("bitfun.cli:{action_id}");
    let participated_in_conflict = preferences.conflicted_candidate_ids.contains(&candidate_id);
    if !participated_in_conflict {
        return None;
    }
    let command_name = action_name.trim_start_matches('/');
    let conflict_key = native_command_conflict_key(
        "local-user",
        command_name,
        [(candidate_id.as_str(), env!("CARGO_PKG_VERSION"))],
    );
    let confirmed = preferences.choices.get(&conflict_key) == Some(&candidate_id);
    Some(BuiltinCommandReconfirmation {
        conflict_key,
        candidate_id,
        confirmed,
    })
}

fn builtin_reconfirmation_names(
    preferences: &ExternalSourceConflictPreferences,
) -> BTreeSet<String> {
    slash_actions(ActionState::chat(false, false))
        .into_iter()
        .filter(|action| {
            builtin_command_reconfirmation(action.id, action.name, preferences)
                .is_some_and(|reconfirmation| !reconfirmation.confirmed)
        })
        .map(|action| action.name.trim_start_matches('/').to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandQualifier {
    Unqualified,
    Builtin,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandRoute {
    Builtin,
    External,
    AskForCollisionChoice,
    WaitForDiscovery,
    UnknownBuiltin,
}

fn parse_command_token(token: &str) -> (CommandQualifier, &str) {
    let requested_name = token.trim_start_matches('/');
    let Some((qualifier, command_name)) = requested_name.split_once(':') else {
        return (CommandQualifier::Unqualified, requested_name);
    };
    if qualifier.eq_ignore_ascii_case("builtin") {
        (CommandQualifier::Builtin, command_name)
    } else if qualifier.eq_ignore_ascii_case("external") {
        (CommandQualifier::External, command_name)
    } else {
        (CommandQualifier::Unqualified, requested_name)
    }
}

fn command_route(
    qualifier: CommandQualifier,
    has_builtin: bool,
    external: Option<&ExternalCommandProjection>,
    discovery_pending: bool,
    builtin_reconfirmation_required: bool,
) -> CommandRoute {
    match qualifier {
        CommandQualifier::Builtin => {
            if has_builtin {
                CommandRoute::Builtin
            } else {
                CommandRoute::UnknownBuiltin
            }
        }
        CommandQualifier::External => CommandRoute::External,
        CommandQualifier::Unqualified => {
            if builtin_reconfirmation_required {
                return CommandRoute::AskForCollisionChoice;
            }
            if discovery_pending {
                return CommandRoute::WaitForDiscovery;
            }
            if let Some(collision) = external.and_then(|command| command.native_collision.as_ref())
            {
                return match collision.selected_candidate_id.as_deref() {
                    Some(selected) if selected == collision.external_candidate_id => {
                        CommandRoute::External
                    }
                    Some(selected) if selected == collision.native_candidate_id => {
                        CommandRoute::Builtin
                    }
                    _ => CommandRoute::AskForCollisionChoice,
                };
            }
            if has_builtin {
                CommandRoute::Builtin
            } else {
                CommandRoute::External
            }
        }
    }
}

impl ChatMode {
    fn external_conflict_preferences(&self) -> ExternalSourceConflictPreferences {
        ExternalSourceConflictPreferences {
            choices: self.external_source_conflict_choices.clone(),
            lineage_current_keys: self.external_source_conflict_lineage_current_keys.clone(),
            conflicted_candidate_ids: self.external_source_conflicted_candidate_ids.clone(),
        }
    }

    fn update_external_source_view(
        &self,
        chat_view: &mut ChatView,
        snapshot: &ExternalSourceCatalogSnapshot,
    ) {
        let preferences = self.external_conflict_preferences();
        chat_view.set_external_source_state(
            external_command_projections(snapshot, &preferences.choices),
            snapshot.discovery_pending,
            builtin_reconfirmation_names(&preferences),
        );
    }

    fn take_external_tool_notice(
        &mut self,
        snapshot: &ExternalSourceCatalogSnapshot,
    ) -> Option<String> {
        let next_key = external_tool_pending_notice_key(snapshot);
        if next_key == self.external_tool_notice_key {
            return None;
        }
        self.external_tool_notice_key = next_key;
        let approvals = snapshot.tool_approval_requests.len();
        let conflicts = snapshot
            .tool_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .count();
        let diagnostics = snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.severity,
                    ExternalSourceDiagnosticSeverity::Warning
                        | ExternalSourceDiagnosticSeverity::Error
                )
            })
            .count();
        if approvals + conflicts + diagnostics == 0 {
            None
        } else {
            Some(format!(
                "External tools need attention: {approvals} approvals, {conflicts} name conflicts, {diagnostics} diagnostics - run /external-tools"
            ))
        }
    }

    fn handle_external_tool_review(
        &mut self,
        arguments: &str,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let action = match parse_external_tool_review_action(
            arguments,
            self.external_source_snapshot.as_ref(),
            self.external_tool_review_snapshot.as_ref(),
        ) {
            Ok(action) => action,
            Err(error) => {
                chat_view.set_status(Some(error));
                return;
            }
        };
        if matches!(action, ExternalToolReviewAction::Show) {
            self.external_tool_review_snapshot = self.external_source_snapshot.clone();
            chat_view.show_info_popup(external_tool_review_text(
                self.external_tool_review_snapshot.as_ref(),
            ));
            return;
        }

        if self.external_tool_mutation_rx.is_some() {
            chat_view.set_status(Some(
                "An external tool update is already running; input and cancellation remain available."
                    .to_string(),
            ));
            return;
        }

        let workspace = self.workspace_path_for_sync(chat_state);
        let pending_status = match &action {
            ExternalToolReviewAction::Refresh => "Refreshing external tools",
            ExternalToolReviewAction::Decide { approved: true, .. } => "Enabling external tool",
            ExternalToolReviewAction::Decide {
                approved: false, ..
            } => "Disabling external tool",
            ExternalToolReviewAction::Choose { .. } => "Selecting external tool provider",
            ExternalToolReviewAction::Show => unreachable!(),
        };
        let task_action = action.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = match &task_action {
                ExternalToolReviewAction::Refresh => {
                    external_source_snapshot(Some(&workspace), true).await
                }
                ExternalToolReviewAction::Decide {
                    approval_key,
                    decision_key,
                    approved,
                } => {
                    set_external_tool_target_decision(
                        Some(&workspace),
                        approval_key,
                        decision_key,
                        *approved,
                    )
                    .await
                }
                ExternalToolReviewAction::Choose {
                    conflict_key,
                    candidate_id,
                } => {
                    set_external_tool_conflict_choice(Some(&workspace), conflict_key, candidate_id)
                        .await
                }
                ExternalToolReviewAction::Show => unreachable!(),
            }
            .map_err(|error| error.to_string());
            let _ = sender.send(ExternalToolMutationResult {
                action: task_action,
                result,
            });
        });
        self.external_tool_mutation_rx = Some(receiver);
        chat_view.set_status(Some(format!(
            "{pending_status}; you can continue typing or cancel other UI work"
        )));
    }

    fn poll_external_tool_mutation(&mut self, chat_view: &mut ChatView) -> bool {
        let outcome = match self
            .external_tool_mutation_rx
            .as_ref()
            .map(Receiver::try_recv)
        {
            Some(Ok(outcome)) => outcome,
            Some(Err(MpscTryRecvError::Empty)) | None => return false,
            Some(Err(MpscTryRecvError::Disconnected)) => {
                self.external_tool_mutation_rx = None;
                chat_view.set_status(Some(
                    "External tool update stopped before returning a result; reopen /external-tools and retry."
                        .to_string(),
                ));
                return true;
            }
        };
        self.external_tool_mutation_rx = None;
        match outcome.result {
            Ok(snapshot) => {
                if external_tool_result_is_stale(self.external_source_snapshot.as_ref(), &snapshot)
                {
                    chat_view.set_status(Some(
                        "External tool update completed; a newer catalog result is already displayed."
                            .to_string(),
                    ));
                    return true;
                }
                self.update_external_source_view(chat_view, &snapshot);
                self.external_tool_notice_key = external_tool_pending_notice_key(&snapshot);
                let approvals = snapshot.tool_approval_requests.len();
                let conflicts = snapshot
                    .tool_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_none())
                    .count();
                let result_label = external_tool_mutation_result_label(&outcome.action, &snapshot);
                self.external_source_snapshot = Some(snapshot);
                if approvals + conflicts == 0 {
                    chat_view.set_status(Some(result_label));
                } else {
                    chat_view.set_status(Some(format!(
                        "{result_label}; {approvals} approvals and {conflicts} conflicts remain - run /external-tools"
                    )));
                }
            }
            Err(error) => {
                tracing::warn!("External tool review action failed: {}", error);
                chat_view.set_status(Some(format!(
                    "External tool choice was not applied: {error}. Reopen /external-tools to review current results."
                )));
            }
        }
        true
    }

    fn take_external_agent_notice(
        &mut self,
        snapshot: &ExternalSourceCatalogSnapshot,
    ) -> Option<String> {
        let attention = external_agent_attention(self.external_source_snapshot.as_ref(), snapshot);
        let next_key = attention.key.clone();
        if next_key == self.external_agent_notice_key {
            return None;
        }
        self.external_agent_notice_key = next_key;
        if attention.confirmations
            + attention.conflicts
            + attention.unavailable
            + attention.diagnostics
            == 0
        {
            None
        } else {
            let mut details = Vec::new();
            if attention.confirmations > 0 {
                details.push(format!("{} confirmations", attention.confirmations));
            }
            if attention.conflicts > 0 {
                details.push(format!("{} name conflicts", attention.conflicts));
            }
            if attention.unavailable > 0 {
                details.push(format!(
                    "{} enabled agents unavailable",
                    attention.unavailable
                ));
            }
            if attention.diagnostics > 0 {
                details.push(format!("{} issues", attention.diagnostics));
            }
            Some(format!(
                "External agents need attention: {} - run /external-agents",
                details.join(", ")
            ))
        }
    }

    fn handle_external_agent_review(
        &mut self,
        arguments: &str,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let action = match parse_external_agent_review_action(
            arguments,
            self.external_source_snapshot.as_ref(),
            self.external_agent_review_snapshot.as_ref(),
        ) {
            Ok(action) => action,
            Err(error) => {
                chat_view.set_status(Some(error));
                return;
            }
        };
        if matches!(action, ExternalAgentReviewAction::Show) {
            self.external_agent_review_snapshot = self.external_source_snapshot.clone();
            chat_view.show_info_popup(external_agent_review_text(
                self.external_agent_review_snapshot.as_ref(),
            ));
            return;
        }
        if self.external_agent_mutation_rx.is_some() {
            chat_view.set_status(Some(
                "An external agent update is already running; input and cancellation remain available."
                    .to_string(),
            ));
            return;
        }

        let workspace = self.workspace_path_for_sync(chat_state);
        let pending_status = match &action {
            ExternalAgentReviewAction::Refresh => "Refreshing external agents",
            ExternalAgentReviewAction::Decide { approved: true, .. } => "Enabling external agent",
            ExternalAgentReviewAction::Decide {
                approved: false, ..
            } => "Disabling external agent",
            ExternalAgentReviewAction::Choose { .. } => "Selecting agent source",
            ExternalAgentReviewAction::Show => unreachable!(),
        };
        let task_action = action.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = match &task_action {
                ExternalAgentReviewAction::Refresh => {
                    external_source_snapshot(Some(&workspace), true).await
                }
                ExternalAgentReviewAction::Decide {
                    candidate_id,
                    decision_key,
                    approved,
                    expected_subagent_generation,
                    expected_preference_revision,
                } => {
                    set_external_subagent_activation(
                        Some(&workspace),
                        candidate_id,
                        *approved,
                        *expected_subagent_generation,
                        *expected_preference_revision,
                        decision_key,
                    )
                    .await
                }
                ExternalAgentReviewAction::Choose {
                    conflict_key,
                    candidate_id,
                    approve_external,
                    expected_subagent_generation,
                    expected_preference_revision,
                } => {
                    choose_external_subagent_conflict(
                        Some(&workspace),
                        conflict_key,
                        candidate_id,
                        *approve_external,
                        *expected_subagent_generation,
                        *expected_preference_revision,
                    )
                    .await
                }
                ExternalAgentReviewAction::Show => unreachable!(),
            }
            .map_err(|error| error.to_string());
            let _ = sender.send(ExternalAgentMutationResult {
                action: task_action,
                result,
            });
        });
        self.external_agent_mutation_rx = Some(receiver);
        chat_view.set_status(Some(format!(
            "{pending_status}; you can continue typing or cancel other UI work"
        )));
    }

    fn poll_external_agent_mutation(&mut self, chat_view: &mut ChatView) -> bool {
        let outcome = match self
            .external_agent_mutation_rx
            .as_ref()
            .map(Receiver::try_recv)
        {
            Some(Ok(outcome)) => outcome,
            Some(Err(MpscTryRecvError::Empty)) | None => return false,
            Some(Err(MpscTryRecvError::Disconnected)) => {
                self.external_agent_mutation_rx = None;
                chat_view.set_status(Some(
                    "External agent update stopped before returning a result; reopen /external-agents and retry."
                        .to_string(),
                ));
                return true;
            }
        };
        self.external_agent_mutation_rx = None;
        match outcome.result {
            Ok(snapshot) => {
                if external_agent_result_is_stale(self.external_source_snapshot.as_ref(), &snapshot)
                {
                    chat_view.set_status(Some(
                        "External agent update completed; newer agent results are already displayed."
                            .to_string(),
                    ));
                    return true;
                }
                let snapshot = merge_external_agent_mutation_snapshot(
                    self.external_source_snapshot.as_ref(),
                    snapshot,
                );
                self.update_external_source_view(chat_view, &snapshot);
                self.external_agent_notice_key =
                    external_agent_pending_notice_key(Some(&snapshot), &snapshot);
                let confirmations = snapshot.pending_subagent_approvals.len();
                let conflicts = snapshot
                    .subagent_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_none())
                    .count();
                let result_label = external_agent_mutation_result_label(&outcome.action, &snapshot);
                self.external_source_snapshot = Some(snapshot);
                if confirmations + conflicts == 0 {
                    chat_view.set_status(Some(result_label));
                } else {
                    chat_view.set_status(Some(format!(
                        "{result_label}; {confirmations} confirmations and {conflicts} conflicts remain - run /external-agents"
                    )));
                }
            }
            Err(error) => {
                tracing::warn!("External agent review action failed: {}", error);
                let reason = error
                    .split_once(':')
                    .map_or(error.as_str(), |(_, reason)| reason.trim());
                chat_view.set_status(Some(format!(
                    "External agent choice was not applied: {reason}. Reopen /external-agents to review current results."
                )));
            }
        }
        true
    }
}
