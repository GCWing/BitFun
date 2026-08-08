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
            return Err("usage: /tools refresh".to_string());
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
        "BitFun has not finished checking external tools; run /tools refresh".to_string()
    })?;
    if command.eq_ignore_ascii_case("enable") || command.eq_ignore_ascii_case("disable") {
        let index = parse_positive_index(parts.next(), "tool number")?;
        if parts.next().is_some() {
            return Err(format!("usage: /tools {command} <tool-number>"));
        }
        let targets = external_tool_target_summaries(snapshot);
        let target = targets
            .get(index)
            .ok_or_else(|| "that tool is no longer available; run /tools refresh".to_string())?;
        let approved = command.eq_ignore_ascii_case("enable");
        let allowed = if approved {
            external_tool_can_enable(target.activation())
        } else {
            external_tool_can_disable(target.activation())
        };
        if !allowed {
            return Err(format!(
                "tool {} is {}; run /tools refresh for its next step",
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
            return Err("usage: /tools choose <conflict-number> <choice-number>".to_string());
        }
        let conflict = snapshot
            .tool_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .chain(
                snapshot
                    .tool_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_some()),
            )
            .nth(conflict_index)
            .ok_or_else(|| {
                "that conflict is no longer available; run /tools refresh".to_string()
            })?;
        let candidate = conflict
            .candidates
            .get(candidate_index)
            .ok_or_else(|| "that choice is no longer available; run /tools refresh".to_string())?;
        return Ok(ExternalToolReviewAction::Choose {
            conflict_key: conflict.conflict_key.clone(),
            candidate_id: candidate.candidate_id.clone(),
        });
    }
    Err("usage: /tools [refresh | enable <number> | disable <number> | choose <conflict-number> <choice-number>]".to_string())
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
                "External tool confirmation saved; run /tools refresh to review the changed tool"
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
                "External tool confirmation saved; run /tools refresh to review its current state"
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
                "External tool choice saved; run /tools refresh to review the changed tool"
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
                "External tool choices changed; run /tools refresh before choosing".to_string()
            }
        }
        ExternalToolReviewAction::Show => "External tools".to_string(),
    }
}

struct BuiltinCommandReconfirmation {
    command_name: String,
    candidate_id: String,
    confirmed: bool,
}

fn cli_native_prompt_command_descriptors(command_name: &str) -> Vec<NativePromptCommandDescriptor> {
    slash_actions(ActionState::chat(false, false))
        .into_iter()
        .filter(|action| {
            action
                .name
                .trim_start_matches('/')
                .eq_ignore_ascii_case(command_name)
        })
        .map(|action| NativePromptCommandDescriptor {
            command_name: command_name.to_ascii_lowercase(),
            candidate_id: format!("bitfun.cli:{}", action.id),
            behavior_version: action_conflict_behavior_version(action.id).to_string(),
        })
        .collect()
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

impl From<bitfun_app_server_protocol::external_source::ExternalSourceConflictPreferences>
    for ExternalSourceConflictPreferences
{
    fn from(
        preferences: bitfun_app_server_protocol::external_source::ExternalSourceConflictPreferences,
    ) -> Self {
        Self {
            choices: preferences.choices,
            lineage_current_keys: preferences.lineage_current_keys,
            conflicted_candidate_ids: preferences.conflicted_candidate_ids,
        }
    }
}

fn builtin_command_reconfirmation(
    action_id: &str,
    command_name: &str,
    preferences: &ExternalSourceConflictPreferences,
) -> Option<BuiltinCommandReconfirmation> {
    let candidate_id = format!("bitfun.cli:{action_id}");
    let participated_in_conflict = preferences.conflicted_candidate_ids.contains(&candidate_id);
    if !participated_in_conflict {
        return None;
    }
    let command_name = command_name.trim_start_matches('/');
    let conflict_key = native_prompt_command_conflict_key(
        "local-user",
        command_name,
        [(
            candidate_id.as_str(),
            action_conflict_behavior_version(action_id),
        )],
    );
    let confirmed = preferences.choices.get(&conflict_key) == Some(&candidate_id);
    Some(BuiltinCommandReconfirmation {
        command_name: command_name.to_ascii_lowercase(),
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
enum CommandRoute {
    Builtin,
    External,
    AskForCollisionChoice,
    WaitForDiscovery,
}

fn command_route(
    has_builtin: bool,
    external: Option<&ExternalCommandProjection>,
    discovery_pending: bool,
    builtin_reconfirmation_required: bool,
) -> CommandRoute {
    if builtin_reconfirmation_required {
        return CommandRoute::AskForCollisionChoice;
    }
    if let Some(collision) = external.and_then(|command| command.native_collision.as_ref()) {
        return match collision.selected_candidate_id.as_deref() {
            Some(selected) if selected == collision.external_candidate_id => CommandRoute::External,
            Some(selected) if selected == collision.native_candidate_id => CommandRoute::Builtin,
            _ => CommandRoute::AskForCollisionChoice,
        };
    }
    if has_builtin {
        CommandRoute::Builtin
    } else if discovery_pending {
        CommandRoute::WaitForDiscovery
    } else {
        CommandRoute::External
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
                "Tools from external AI applications need attention: {approvals} approvals, {conflicts} name conflicts, {diagnostics} diagnostics - run /tools refresh"
            ))
        }
    }

    fn handle_external_control(
        &mut self,
        arguments: &str,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let legacy_command = arguments.split_whitespace().next().is_some_and(|command| {
            command.eq_ignore_ascii_case("safe-mode") || command.eq_ignore_ascii_case("source")
        });
        if !legacy_command {
            self.handle_external_application(arguments, chat_view, rt_handle);
            return;
        }
        self.handle_legacy_external_control(arguments, chat_view, chat_state, rt_handle);
    }

    fn handle_external_application(
        &mut self,
        arguments: &str,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let action =
            match parse_external_application_action(arguments, &self.external_application_ui) {
                Ok(action) => action,
                Err(error) => {
                    chat_view.set_status(Some(error));
                    return;
                }
            };
        if self.external_application_ui.pending_rx.is_some() {
            chat_view.set_status(Some(
                "An external application update is already running; input remains available."
                    .to_string(),
            ));
            return;
        }

        if let ExternalApplicationUiAction::SetReviewItem { item_ref, selected } = &action {
            let index = self
                .external_application_ui
                .review
                .as_ref()
                .and_then(|review| {
                    review
                        .page
                        .items
                        .iter()
                        .position(|item| item.item_ref == *item_ref)
                });
            let result = index
                .ok_or_else(|| "That review item is stale; reopen /extensions review.".to_string())
                .and_then(|index| {
                    self.external_application_ui
                        .set_review_item_selected(index, *selected)
                });
            match result
                .and_then(|()| external_application_review_text(&self.external_application_ui))
            {
                Ok(text) => {
                    chat_view.show_info_popup(text);
                    chat_view.set_status(Some(
                        "Review selection updated locally; run /extensions review apply to save."
                            .to_string(),
                    ));
                }
                Err(error) => chat_view.set_status(Some(error)),
            }
            return;
        }
        if matches!(action, ExternalApplicationUiAction::DeferReview) {
            self.external_application_ui.review = None;
            chat_view.set_status(Some(
                "Review closed with no changes applied; it remains pending.".to_string(),
            ));
            return;
        }

        let pending = match &action {
            ExternalApplicationUiAction::Show => ExternalApplicationPendingRequest::Snapshot {
                force_refresh: false,
            },
            ExternalApplicationUiAction::Refresh => ExternalApplicationPendingRequest::Snapshot {
                force_refresh: true,
            },
            ExternalApplicationUiAction::OpenReview => {
                let request = match self.external_application_ui.open_review_page_request() {
                    Ok(request) => request,
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                };
                ExternalApplicationPendingRequest::ReviewPage {
                    request,
                    navigation: ExternalReviewNavigation::Open,
                }
            }
            ExternalApplicationUiAction::ReviewNext => {
                let (request, navigation) = match self
                    .external_application_ui
                    .review_page_request(ExternalReviewDirection::Next)
                {
                    Ok(request) => request,
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                };
                ExternalApplicationPendingRequest::ReviewPage {
                    request,
                    navigation,
                }
            }
            ExternalApplicationUiAction::ReviewPrevious => {
                let (request, navigation) = match self
                    .external_application_ui
                    .review_page_request(ExternalReviewDirection::Previous)
                {
                    Ok(request) => request,
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                };
                ExternalApplicationPendingRequest::ReviewPage {
                    request,
                    navigation,
                }
            }
            ExternalApplicationUiAction::ConnectApplication { application_id } => {
                let request = self.external_application_ui.control_request(
                    &format!("tui-{}", uuid::Uuid::new_v4()),
                    ExternalApplicationControlActionV2::ConnectApplication {
                        application_id: application_id.clone(),
                    },
                );
                match request {
                    Ok(request) => ExternalApplicationPendingRequest::Mutation(request),
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                }
            }
            ExternalApplicationUiAction::DisconnectApplication { application_id } => {
                let request = self.external_application_ui.control_request(
                    &format!("tui-{}", uuid::Uuid::new_v4()),
                    ExternalApplicationControlActionV2::DisconnectApplication {
                        application_id: application_id.clone(),
                    },
                );
                match request {
                    Ok(request) => ExternalApplicationPendingRequest::Mutation(request),
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                }
            }
            ExternalApplicationUiAction::DeferApplication { application_id } => {
                let request = self.external_application_ui.control_request(
                    &format!("tui-{}", uuid::Uuid::new_v4()),
                    ExternalApplicationControlActionV2::SetApplicationDeferred {
                        application_id: application_id.clone(),
                    },
                );
                match request {
                    Ok(request) => ExternalApplicationPendingRequest::Mutation(request),
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                }
            }
            ExternalApplicationUiAction::ApplyReview => {
                match self
                    .external_application_ui
                    .review_apply_request(&format!("tui-{}", uuid::Uuid::new_v4()))
                {
                    Ok(request) => ExternalApplicationPendingRequest::Mutation(request),
                    Err(error) => {
                        chat_view.set_status(Some(error));
                        return;
                    }
                }
            }
            ExternalApplicationUiAction::SetReviewItem { .. }
            | ExternalApplicationUiAction::DeferReview => unreachable!(),
        };

        let agent = self.agent.clone();
        let shared = agent.is_shared();
        let task_action = action.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = match pending {
                ExternalApplicationPendingRequest::Snapshot { force_refresh } => {
                    match agent.external_application_snapshot_v2(force_refresh).await {
                        Ok(snapshot) => Ok(ExternalApplicationAsyncResult::Snapshot(snapshot)),
                        Err(error) if should_fallback_to_legacy_external_status(shared, &error) => {
                            agent
                                .external_source_snapshot(force_refresh)
                                .await
                                .map(ExternalApplicationAsyncResult::LegacySnapshot)
                        }
                        Err(error) => Err(error),
                    }
                }
                ExternalApplicationPendingRequest::ReviewPage {
                    request,
                    navigation,
                } => agent
                    .external_application_review_page_v2(request)
                    .await
                    .map(|page| ExternalApplicationAsyncResult::ReviewPage { page, navigation }),
                ExternalApplicationPendingRequest::Mutation(request) => {
                    let result = agent.apply_external_application_action_v2(request).await;
                    match result {
                        Ok(result) => {
                            agent
                                .external_application_snapshot_v2(false)
                                .await
                                .map(|snapshot| ExternalApplicationAsyncResult::Mutation {
                                    result,
                                    snapshot,
                                })
                        }
                        Err(error) => Err(error),
                    }
                }
            };
            let _ = sender.send(ExternalApplicationMutationResult {
                action: task_action,
                result,
            });
        });
        self.external_application_ui.pending_rx = Some(receiver);
        let status = match action {
            ExternalApplicationUiAction::Show => "Reading external applications",
            ExternalApplicationUiAction::Refresh => "Refreshing external applications",
            ExternalApplicationUiAction::OpenReview
            | ExternalApplicationUiAction::ReviewNext
            | ExternalApplicationUiAction::ReviewPrevious => "Reading external application review",
            ExternalApplicationUiAction::ConnectApplication { .. } => {
                "Connecting external application"
            }
            ExternalApplicationUiAction::DisconnectApplication { .. } => {
                "Disconnecting external application"
            }
            ExternalApplicationUiAction::DeferApplication { .. } => {
                "Deferring external application decision"
            }
            ExternalApplicationUiAction::ApplyReview => "Applying external application review",
            ExternalApplicationUiAction::SetReviewItem { .. }
            | ExternalApplicationUiAction::DeferReview => unreachable!(),
        };
        chat_view.set_status(Some(format!(
            "{status}; you can continue typing or cancel other UI work"
        )));
    }

    fn handle_legacy_external_control(
        &mut self,
        arguments: &str,
        chat_view: &mut ChatView,
        _chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let action = match parse_external_control_action(arguments) {
            Ok(action) => action,
            Err(error) => {
                chat_view.set_status(Some(error));
                return;
            }
        };
        if self.external_control_mutation_rx.is_some() {
            chat_view.set_status(Some(
                "An external integration update is already running; input remains available."
                    .to_string(),
            ));
            return;
        }

        let expected_preference_revision = self
            .external_source_snapshot
            .as_ref()
            .map(|snapshot| snapshot.preference_revision);
        let task_action = action.clone();
        let agent = self.agent.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = async {
                if matches!(&task_action, ExternalControlUiAction::Show) {
                    let response = agent.external_source_snapshot(false).await?;
                    return Ok((
                        response.control,
                        Some(response.snapshot),
                        Some(response.preferences.into()),
                    ));
                }

                let action = match &task_action {
                    ExternalControlUiAction::Refresh => ExternalSourceControlActionV1::Refresh,
                    ExternalControlUiAction::SetSafeMode(enabled) => {
                        ExternalSourceControlActionV1::SetSafeMode { enabled: *enabled }
                    }
                    ExternalControlUiAction::SetSourceEnabled {
                        source_key,
                        enabled,
                    } => ExternalSourceControlActionV1::SetSourceEnabled {
                        source_key: source_key.clone(),
                        enabled: *enabled,
                    },
                    ExternalControlUiAction::Show => unreachable!(),
                };
                let response = agent
                    .external_source_control(ExternalSourceControlRequestV1 {
                        schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
                        operation_id: format!("tui-{}", uuid::Uuid::new_v4()),
                        expected_preference_revision,
                        action,
                    })
                    .await?;
                Ok((
                    response.surface.control,
                    Some(response.snapshot.snapshot),
                    Some(response.snapshot.preferences.into()),
                ))
            }
            .await;
            let _ = sender.send(ExternalControlMutationResult {
                action: task_action,
                result,
            });
        });
        self.external_control_mutation_rx = Some(receiver);
        let status = match action {
            ExternalControlUiAction::Show => "Reading external integration status",
            ExternalControlUiAction::Refresh => "Refreshing external integrations",
            ExternalControlUiAction::SetSafeMode(true) => "Entering External Safe Mode",
            ExternalControlUiAction::SetSafeMode(false) => "Exiting External Safe Mode",
            ExternalControlUiAction::SetSourceEnabled { enabled: true, .. } => {
                "Enabling external source"
            }
            ExternalControlUiAction::SetSourceEnabled { enabled: false, .. } => {
                "Disabling external source"
            }
        };
        chat_view.set_status(Some(format!(
            "{status}; you can continue typing or cancel other UI work"
        )));
    }

    fn poll_external_application_mutation(&mut self, chat_view: &mut ChatView) -> bool {
        let outcome = match self
            .external_application_ui
            .pending_rx
            .as_ref()
            .map(Receiver::try_recv)
        {
            Some(Ok(outcome)) => outcome,
            Some(Err(MpscTryRecvError::Empty)) | None => return false,
            Some(Err(MpscTryRecvError::Disconnected)) => {
                self.external_application_ui.pending_rx = None;
                chat_view.set_status(Some(
                    "External application status stopped before returning a result; retry /extensions status."
                        .to_string(),
                ));
                return true;
            }
        };
        self.external_application_ui.pending_rx = None;
        match outcome.result {
            Ok(ExternalApplicationAsyncResult::Snapshot(snapshot)) => {
                match self.external_application_ui.replace_snapshot(snapshot) {
                    Ok(()) => {
                        if let Ok(snapshot) = self.external_application_ui.snapshot() {
                            chat_view.show_info_popup(external_application_overview_text(snapshot));
                            chat_view.set_status(Some(
                                if matches!(outcome.action, ExternalApplicationUiAction::Refresh) {
                                    "External applications refreshed"
                                } else {
                                    "External application status updated"
                                }
                                .to_string(),
                            ));
                        }
                    }
                    Err(error) => chat_view.set_status(Some(error)),
                }
            }
            Ok(ExternalApplicationAsyncResult::LegacySnapshot(response)) => {
                self.replace_external_conflict_preferences(response.preferences.into());
                self.update_external_source_view(chat_view, &response.snapshot);
                self.external_source_snapshot = Some(response.snapshot);
                self.external_application_ui.snapshot = None;
                self.external_application_ui.review = None;
                chat_view
                    .show_info_popup(external_control_read_only_review_text(&response.control));
                chat_view.set_status(Some(
                    "External application status is shown in read-only compatibility mode"
                        .to_string(),
                ));
            }
            Ok(ExternalApplicationAsyncResult::ReviewPage { page, navigation }) => {
                match self
                    .external_application_ui
                    .replace_review_page(page, navigation)
                    .and_then(|()| external_application_review_text(&self.external_application_ui))
                {
                    Ok(text) => {
                        chat_view.show_info_popup(text);
                        chat_view.set_status(Some(
                            "External application review page updated".to_string(),
                        ));
                    }
                    Err(error) => {
                        self.external_application_ui.review = None;
                        chat_view.set_status(Some(error));
                    }
                }
            }
            Ok(ExternalApplicationAsyncResult::Mutation { result, snapshot }) => {
                if let Err(error) = result.validate() {
                    chat_view.set_status(Some(format!(
                        "The external application response was invalid: {error}"
                    )));
                    return true;
                }
                let operation_outcome = result.outcome;
                let partial = result
                    .item_results
                    .iter()
                    .any(|item| item.outcome != ExternalApplicationOperationOutcomeV2::Applied);
                if let Err(error) = self.external_application_ui.replace_snapshot(snapshot) {
                    chat_view.set_status(Some(error));
                    return true;
                }
                if let Ok(snapshot) = self.external_application_ui.snapshot() {
                    chat_view.show_info_popup(external_application_overview_text(snapshot));
                }
                let status = match operation_outcome {
                    ExternalApplicationOperationOutcomeV2::Applied if partial => {
                        "External application changes were partially applied; review the refreshed status"
                    }
                    ExternalApplicationOperationOutcomeV2::Applied => match outcome.action {
                        ExternalApplicationUiAction::ConnectApplication { .. } => {
                            "External application connected"
                        }
                        ExternalApplicationUiAction::DisconnectApplication { .. } => {
                            "External application disconnected"
                        }
                        ExternalApplicationUiAction::DeferApplication { .. } => {
                            "External application decision deferred"
                        }
                        ExternalApplicationUiAction::ApplyReview => {
                            "External application review applied"
                        }
                        _ => "External application change applied",
                    },
                    ExternalApplicationOperationOutcomeV2::Stale => {
                        "Nothing was applied because the external application data changed; review the refreshed status"
                    }
                    ExternalApplicationOperationOutcomeV2::Blocked => {
                        "External application change was blocked; review the refreshed status"
                    }
                    ExternalApplicationOperationOutcomeV2::Rejected => {
                        "External application change was rejected; review the refreshed status"
                    }
                    ExternalApplicationOperationOutcomeV2::Failed => {
                        "External application change failed; review the refreshed status"
                    }
                };
                chat_view.set_status(Some(status.to_string()));
            }
            Err(error) => {
                if matches!(
                    error.code,
                    ExternalSourceOperationErrorCode::HostCapabilityUnavailable
                        | ExternalSourceOperationErrorCode::Unsupported
                        | ExternalSourceOperationErrorCode::IncompatibleVersion
                ) {
                    self.external_application_ui.snapshot = None;
                    self.external_application_ui.review = None;
                } else if matches!(error.code, ExternalSourceOperationErrorCode::StaleRevision) {
                    self.external_application_ui.review = None;
                }
                tracing::warn!(
                    error_code = error.code.as_str(),
                    correlation_id = error.correlation_id.as_deref().unwrap_or("none"),
                    operation_stage = ?error.stage,
                    "External application action failed"
                );
                chat_view.set_status(Some(external_operation_error_status("extensions", &error)));
            }
        }
        true
    }

    fn poll_external_control_mutation(&mut self, chat_view: &mut ChatView) -> bool {
        let application_changed = self.poll_external_application_mutation(chat_view);
        let outcome = match self
            .external_control_mutation_rx
            .as_ref()
            .map(Receiver::try_recv)
        {
            Some(Ok(outcome)) => outcome,
            Some(Err(MpscTryRecvError::Empty)) | None => return application_changed,
            Some(Err(MpscTryRecvError::Disconnected)) => {
                self.external_control_mutation_rx = None;
                chat_view.set_status(Some(
                    "External integration status stopped before returning a result; retry /extensions status."
                        .to_string(),
                ));
                return true;
            }
        };
        self.external_control_mutation_rx = None;
        match outcome.result {
            Ok((control, catalog, preferences)) => {
                if let Some(preferences) = preferences {
                    self.replace_external_conflict_preferences(preferences);
                }
                if let Some(catalog) = catalog {
                    self.update_external_source_view(chat_view, &catalog);
                    self.external_source_snapshot = Some(catalog);
                }
                chat_view.show_info_popup(external_control_review_text(&control));
                let status = match outcome.action {
                    ExternalControlUiAction::Show => "External integration status updated",
                    ExternalControlUiAction::Refresh => "External integrations refreshed",
                    ExternalControlUiAction::SetSafeMode(true) => "External Safe Mode is active",
                    ExternalControlUiAction::SetSafeMode(false) => {
                        "External Safe Mode is off; eligible integrations were reconciled"
                    }
                    ExternalControlUiAction::SetSourceEnabled { enabled: true, .. } => {
                        "External source enabled"
                    }
                    ExternalControlUiAction::SetSourceEnabled { enabled: false, .. } => {
                        "External source disabled"
                    }
                };
                chat_view.set_status(Some(status.to_string()));
            }
            Err(error) => {
                tracing::warn!(
                    error_code = error.code.as_str(),
                    correlation_id = error.correlation_id.as_deref().unwrap_or("none"),
                    operation_stage = ?error.stage,
                    "External integration control action failed"
                );
                chat_view.set_status(Some(external_operation_error_status("extensions", &error)));
            }
        }
        true
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

        let _ = chat_state;
        let expected_preference_revision = self
            .external_source_snapshot
            .as_ref()
            .map(|snapshot| snapshot.preference_revision)
            .unwrap_or(0);
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
        let agent = self.agent.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = match &task_action {
                ExternalToolReviewAction::Refresh => {
                    agent
                        .external_source_review(ExternalSourceReviewAction::Refresh)
                        .await
                }
                ExternalToolReviewAction::Decide {
                    approval_key,
                    decision_key,
                    approved,
                } => {
                    agent
                        .external_source_review(ExternalSourceReviewAction::SetToolTargetDecision {
                            approval_key: approval_key.clone(),
                            decision_key: decision_key.clone(),
                            approved: *approved,
                            expected_preference_revision,
                        })
                        .await
                }
                ExternalToolReviewAction::Choose {
                    conflict_key,
                    candidate_id,
                } => {
                    agent
                        .external_source_review(ExternalSourceReviewAction::SetToolConflictChoice {
                            conflict_key: conflict_key.clone(),
                            candidate_id: candidate_id.clone(),
                            expected_preference_revision,
                        })
                        .await
                }
                ExternalToolReviewAction::Show => unreachable!(),
            }
            .map(|response| response.snapshot);
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
                    "External tool update stopped before returning a result; run /tools refresh and retry."
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
                        "{result_label}; {approvals} approvals and {conflicts} conflicts remain - run /tools refresh"
                    )));
                }
            }
            Err(error) => {
                tracing::warn!(
                    error_code = error.code.as_str(),
                    correlation_id = error.correlation_id.as_deref().unwrap_or("none"),
                    "External tool review action failed"
                );
                chat_view.set_status(Some(external_operation_error_status("tools", &error)));
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
        if attention.bindings
            + attention.confirmations
            + attention.conflicts
            + attention.unavailable
            + attention.diagnostics
            == 0
        {
            None
        } else {
            let mut details = Vec::new();
            if attention.bindings > 0 {
                details.push(format!("{} model bindings", attention.bindings));
            }
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
                "Agents from external AI applications need attention: {} - run /agent refresh",
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

        let _ = chat_state;
        let pending_status = match &action {
            ExternalAgentReviewAction::Refresh => "Refreshing external agents",
            ExternalAgentReviewAction::Decide { approved: true, .. } => "Enabling external agent",
            ExternalAgentReviewAction::Decide {
                approved: false, ..
            } => "Disabling external agent",
            ExternalAgentReviewAction::Choose { .. } => "Selecting agent source",
            ExternalAgentReviewAction::Bind { .. } => "Saving agent model binding",
            ExternalAgentReviewAction::Show => unreachable!(),
        };
        let task_action = action.clone();
        let agent = self.agent.clone();
        let (sender, receiver) = mpsc::channel();
        rt_handle.spawn(async move {
            let result = match &task_action {
                ExternalAgentReviewAction::Refresh => {
                    agent
                        .external_source_review(ExternalSourceReviewAction::Refresh)
                        .await
                }
                ExternalAgentReviewAction::Decide {
                    candidate_id,
                    decision_key,
                    approved,
                    expected_subagent_generation,
                    expected_preference_revision,
                } => {
                    agent
                        .external_source_review(ExternalSourceReviewAction::SetSubagentActivation {
                            candidate_id: candidate_id.clone(),
                            approved: *approved,
                            expected_subagent_generation: *expected_subagent_generation,
                            expected_preference_revision: *expected_preference_revision,
                            decision_key: decision_key.clone(),
                        })
                        .await
                }
                ExternalAgentReviewAction::Choose {
                    conflict_key,
                    candidate_id,
                    approve_external,
                    expected_subagent_generation,
                    expected_preference_revision,
                } => {
                    agent
                        .external_source_review(
                            ExternalSourceReviewAction::ChooseSubagentConflict {
                                conflict_key: conflict_key.clone(),
                                candidate_id: candidate_id.clone(),
                                approve_external: *approve_external,
                                expected_subagent_generation: *expected_subagent_generation,
                                expected_preference_revision: *expected_preference_revision,
                            },
                        )
                        .await
                }
                ExternalAgentReviewAction::Bind {
                    binding_key,
                    target,
                    expected_subagent_generation,
                    expected_preference_revision,
                } => {
                    agent
                        .external_source_review(
                            ExternalSourceReviewAction::SetSubagentModelBinding {
                                binding_key: binding_key.clone(),
                                target: target.clone(),
                                expected_subagent_generation: *expected_subagent_generation,
                                expected_preference_revision: *expected_preference_revision,
                            },
                        )
                        .await
                }
                ExternalAgentReviewAction::Show => unreachable!(),
            }
            .map(|response| response.snapshot);
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
                    "External agent update stopped before returning a result; run /agent refresh."
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
                let bindings = snapshot
                    .subagent_model_binding_groups
                    .iter()
                    .filter(|binding| {
                        matches!(
                            binding.method,
                            ExternalSubagentModelBindingMethod::BindingRequired
                                | ExternalSubagentModelBindingMethod::BindingUnavailable
                        )
                    })
                    .count();
                let conflicts = snapshot
                    .subagent_conflicts
                    .iter()
                    .filter(|conflict| conflict.selected_candidate_id.is_none())
                    .count();
                let result_label = external_agent_mutation_result_label(&outcome.action, &snapshot);
                self.external_source_snapshot = Some(snapshot);
                if bindings + confirmations + conflicts == 0 {
                    chat_view.set_status(Some(result_label));
                } else {
                    chat_view.set_status(Some(format!(
                        "{result_label}; {bindings} model bindings, {confirmations} confirmations, and {conflicts} conflicts remain - run /agent refresh"
                    )));
                }
            }
            Err(error) => {
                tracing::warn!(
                    error_code = error.code.as_str(),
                    correlation_id = error.correlation_id.as_deref().unwrap_or("none"),
                    "External agent review action failed"
                );
                chat_view.set_status(Some(external_operation_error_status("agents", &error)));
            }
        }
        true
    }
}
