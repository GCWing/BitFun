enum SessionUpdateApplyOutcome {
    SessionUpdateFailed(String),
    OutcomeUnknown(String),
    Applied,
}

enum SessionUpdatePollOutcome {
    NoChange,
    Redraw,
    ExitAfterSave,
    ExitAfterUnknownOutcome(String),
}

fn previous_session_update_status(
    setting_name: &str,
    selected_id: &str,
    outcome: &SessionUpdateApplyOutcome,
) -> String {
    match outcome {
        SessionUpdateApplyOutcome::Applied => format!(
            "The previous session {setting_name} was changed to {selected_id}; the current session was not modified."
        ),
        SessionUpdateApplyOutcome::SessionUpdateFailed(error) => format!(
            "The previous session {setting_name} change to {selected_id} failed: {error}. Return to that session to retry."
        ),
        SessionUpdateApplyOutcome::OutcomeUnknown(error) => format!(
            "The previous session {setting_name} change to {selected_id} has an unknown outcome: {error}. This TUI is closing; reopen it, restore that session, and inspect its current {setting_name} before retrying."
        ),
    }
}

fn session_update_completion_should_exit(exit_requested: bool, applied: bool) -> bool {
    exit_requested && applied
}

fn apply_agent_mode_feedback(
    current_mode: &mut String,
    chat_state: &mut ChatState,
    selected_mode: &str,
    outcome: SessionUpdateApplyOutcome,
) -> bool {
    match outcome {
        SessionUpdateApplyOutcome::SessionUpdateFailed(error) => {
            tracing::error!(
                "Failed to switch agent mode to {}: {}",
                selected_mode,
                error
            );
            chat_state.add_system_message(format!(
                "Agent mode was not changed: {error}. Please retry."
            ));
            false
        }
        SessionUpdateApplyOutcome::OutcomeUnknown(error) => {
            tracing::error!(
                "Agent mode update outcome is unknown for {}: {}",
                selected_mode,
                error
            );
            chat_state.add_system_message(format!(
                "Agent mode update outcome is unknown: {error}. This TUI is closing; reopen it, restore this session, and inspect its current mode before retrying."
            ));
            false
        }
        SessionUpdateApplyOutcome::Applied => {
            *current_mode = selected_mode.to_string();
            chat_state.agent_type = selected_mode.to_string();
            tracing::info!("Agent mode switched to: {}", selected_mode);
            true
        }
    }
}

fn usage_report_metadata(report: &SessionUsageReport) -> Result<serde_json::Value> {
    let usage_report = serde_json::to_value(report)
        .map_err(|error| anyhow!("Failed to serialize usage report: {error}"))?;
    Ok(serde_json::json!({
        "localCommandKind": "usage_report",
        "reportId": report.report_id,
        "schemaVersion": report.schema_version,
        "generatedAt": report.generated_at,
        "modelVisible": false,
        "usageReport": usage_report,
        "usageReportStatus": "completed",
    }))
}

fn apply_model_selection_feedback(
    chat_state: &mut ChatState,
    selected_display_name: &str,
    selected_id: &str,
    outcome: SessionUpdateApplyOutcome,
) -> bool {
    match outcome {
        SessionUpdateApplyOutcome::SessionUpdateFailed(error) => {
            tracing::error!(
                "Failed to switch model to {} ({}): {}",
                selected_display_name,
                selected_id,
                error
            );
            chat_state.add_system_message(format!(
                "Current session model was not changed: {error}. Please retry."
            ));
            false
        }
        SessionUpdateApplyOutcome::OutcomeUnknown(error) => {
            tracing::error!(
                "Model update outcome is unknown for {} ({}): {}",
                selected_display_name,
                selected_id,
                error
            );
            chat_state.add_system_message(format!(
                "Model update outcome is unknown: {error}. This TUI is closing; reopen it, restore this session, and inspect its current model before retrying."
            ));
            false
        }
        SessionUpdateApplyOutcome::Applied => {
            chat_state.current_model_id = Some(selected_id.to_string());
            chat_state.current_model_name = selected_display_name.to_string();
            tracing::info!(
                "Model switched to: {} ({})",
                selected_display_name,
                selected_id
            );
            true
        }
    }
}

fn apply_session_rename_feedback(
    chat_state: &mut ChatState,
    session_name: &str,
    outcome: SessionUpdateApplyOutcome,
) -> bool {
    match outcome {
        SessionUpdateApplyOutcome::SessionUpdateFailed(error) => {
            tracing::error!("Failed to rename the current session: {}", error);
            chat_state.add_system_message(format!(
                "Current session name was not changed: {error}. Please retry."
            ));
            false
        }
        SessionUpdateApplyOutcome::OutcomeUnknown(error) => {
            tracing::error!("Session rename outcome is unknown: {}", error);
            chat_state.add_system_message(format!(
                "Session rename outcome is unknown: {error}. This TUI is closing; reopen it, restore this session, and inspect its current name before retrying."
            ));
            false
        }
        SessionUpdateApplyOutcome::Applied => {
            chat_state.session_name = session_name.to_string();
            tracing::info!("Current session renamed");
            true
        }
    }
}

fn apply_session_model_migration(
    chat_state: &mut ChatState,
    event_session_id: &str,
    previous_model_id: &str,
    new_model_id: &str,
    reason: &str,
) -> bool {
    if event_session_id != chat_state.core_session_id {
        tracing::debug!(
            "Ignoring model migration for another session: current_session_id={}, event_session_id={}",
            chat_state.core_session_id,
            event_session_id
        );
        return false;
    }
    if chat_state.current_model_id.as_deref() != Some(previous_model_id) {
        tracing::debug!(
            "Ignoring stale model migration: session_id={}, current_model_id={:?}, previous_model_id={}",
            event_session_id,
            chat_state.current_model_id,
            previous_model_id
        );
        return false;
    }
    chat_state.current_model_id = Some(new_model_id.to_string());
    chat_state.current_model_name = new_model_id.to_string();
    chat_state.add_system_message(format!(
        "The current session model changed from {previous_model_id} to {new_model_id} because {reason}."
    ));
    true
}

impl ChatMode {
    fn logout(&self, chat_state: &mut ChatState, rt_handle: &tokio::runtime::Handle) {
        let logged_in =
            tokio::task::block_in_place(|| rt_handle.block_on(crate::account::is_logged_in()));
        if !logged_in {
            chat_state.add_system_message("Not logged in.".to_string());
            return;
        }
        match tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout())) {
            Ok(()) => chat_state.add_system_message("Logged out.".to_string()),
            Err(error) => chat_state.add_system_message(format!("Logout failed: {error}")),
        }
    }

    fn show_usage_report(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if chat_state.is_processing {
            chat_view.set_status(Some(
                "Wait until the session is idle before using /usage.".to_string(),
            ));
            return;
        }

        let session_id = chat_state.core_session_id.clone();
        let workspace_path = chat_state
            .workspace
            .clone()
            .or_else(|| self.workspace.clone())
            .or_else(|| Some(self.agent.workspace_path_string()));
        let agent = self.agent.clone();

        let report_result: Result<bitfun_core::service::session_usage::SessionUsageReport> =
            tokio::task::block_in_place(|| {
                let session_id = session_id.clone();
                let workspace_path = workspace_path.clone();
                let agent = agent.clone();
                rt_handle.block_on(async move {
                    let workspace_path = workspace_path
                        .filter(|path| !path.trim().is_empty())
                        .ok_or_else(|| anyhow!("Workspace path is required for usage reports"))?;

                    let report = agent
                        .generate_session_usage_report(AgentSessionUsageRequest {
                            session_id: session_id.clone(),
                            workspace_path: Some(workspace_path),
                            remote_connection_id: None,
                            remote_ssh_host: None,
                            include_hidden_subagents: true,
                        })
                        .await?;

                    let markdown = render_usage_report_markdown(&report);
                    let generated_at = u64::try_from(report.generated_at).unwrap_or_default();
                    let metadata = usage_report_metadata(&report)?;
                    agent
                        .record_completed_local_command_turn(AgentLocalCommandTurnRecordRequest {
                            session_id,
                            content: markdown,
                            turn_id: Some(format!("local-usage-{}", report.report_id)),
                            timestamp_ms: Some(generated_at),
                            metadata: metadata.as_object().cloned().ok_or_else(|| {
                                anyhow!("Usage report metadata must be an object")
                            })?,
                        })
                        .await?;

                    Ok(report)
                })
            });

        match report_result {
            Ok(report) => {
                let markdown = render_usage_report_markdown(&report);
                chat_state.add_assistant_message(markdown);
                chat_view.set_status(Some("Usage report added to conversation".to_string()));
            }
            Err(error) => {
                chat_state
                    .add_system_message(format!("Failed to generate usage report: {}", error));
            }
        }
    }

    fn list_available_themes(&self) -> Vec<ThemeItem> {
        let mut themes = Vec::new();
        for id in builtin_theme_ids() {
            themes.push(ThemeItem { id });
        }

        themes.sort_by_cached_key(|theme| theme.id.to_ascii_lowercase());
        themes.dedup_by(|a, b| a.id == b.id);
        themes
    }

    fn resolve_configured_theme(
        &self,
        base: Theme,
        appearance: Appearance,
        scheme: EffectiveColorScheme,
    ) -> Theme {
        self.resolve_theme_by_id(base, appearance, scheme, self.config.ui.theme_id.trim())
    }

    fn resolve_theme_by_id(
        &self,
        base: Theme,
        appearance: Appearance,
        scheme: EffectiveColorScheme,
        id: &str,
    ) -> Theme {
        if scheme == EffectiveColorScheme::Monochrome {
            return Theme::monochrome();
        }

        if id.is_empty() {
            return base;
        }

        if let Some(json) = builtin_theme_json(id) {
            return base
                .apply_opencode_theme_json(json, appearance)
                .unwrap_or(base)
                .with_effective_scheme(scheme);
        }

        base
    }

    fn preview_theme_selection(&mut self, theme: &ThemeItem, chat_view: &mut ChatView) {
        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };

        let resolved = self.resolve_theme_by_id(base, appearance, scheme, theme.id.trim());
        chat_view.set_theme(resolved);
        chat_view.set_status(Some(format!(
            "Preview theme: {} (Enter apply, Esc cancel)",
            theme.id
        )));
    }

    fn apply_theme_selection(&mut self, theme: &ThemeItem, chat_view: &mut ChatView) {
        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };

        self.config.ui.theme_id = theme.id.clone();
        if let Err(e) = self.config.save() {
            chat_view.set_status(Some(format!("Failed to save config: {}", e)));
        }

        let resolved = self.resolve_theme_by_id(base, appearance, scheme, theme.id.trim());
        chat_view.set_theme(resolved);
        chat_view.set_status(Some(format!("Theme set to: {}", theme.id)));
    }

    fn get_mode_agents(&self, rt_handle: &tokio::runtime::Handle) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        let modes = tokio::task::block_in_place(|| rt_handle.block_on(registry.get_modes_info()));
        modes
    }

    fn cycle_agent(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.switch_agent_by_offset(1, chat_view, chat_state, rt_handle);
    }

    fn cycle_agent_reverse(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.switch_agent_by_offset(-1, chat_view, chat_state, rt_handle);
    }

    fn switch_agent_by_offset(
        &mut self,
        offset: isize,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if !session_update_allowed(
            chat_state.is_processing,
            self.pending_session_update.is_some(),
        ) {
            chat_view.set_status(Some(session_update_unavailable_message(
                "Agent mode",
                chat_state.is_processing,
            )));
            return;
        }
        let modes = self.get_mode_agents(rt_handle);
        if modes.len() <= 1 {
            return;
        }

        let current_idx = modes
            .iter()
            .position(|m| m.id == self.agent_type)
            .unwrap_or(0);

        let len = modes.len() as isize;
        let next_idx = ((current_idx as isize + offset) % len + len) % len;
        let next = &modes[next_idx as usize];

        let selected = AgentItem {
            id: next.id.clone(),
            description: next.description.clone(),
        };
        self.apply_agent_selection(&selected, chat_view, chat_state, rt_handle);
    }

    /// Resolve the Runtime-owned Session model through the local product catalog.
    fn load_current_model_name(
        &self,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let session_model_id = chat_state.current_model_id.clone();
        let result: Option<String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                let model_id = crate::model_selection::resolve_session_model_display_id(
                    &global_config.ai,
                    session_model_id.as_deref(),
                )?;

                fn provider_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    let raw_name = model.name.trim();
                    let model_name = model.model_name.trim();
                    if !raw_name.is_empty() && !model_name.is_empty() {
                        let dashed_suffix = format!(" - {}", model_name);
                        let slash_suffix = format!("/{}", model_name);
                        if let Some(provider) = raw_name.strip_suffix(&dashed_suffix) {
                            return provider.trim().to_string();
                        }
                        if let Some(provider) = raw_name.strip_suffix(&slash_suffix) {
                            return provider.trim().to_string();
                        }
                    }
                    if raw_name.is_empty() {
                        model.provider.clone()
                    } else {
                        raw_name.to_string()
                    }
                }

                fn model_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    format!("{} / {}", model.model_name, provider_display_name(model))
                }

                let model_name = models
                    .iter()
                    .find(|model| model.id == model_id)
                    .map(model_display_name)
                    .unwrap_or_else(|| model_id.clone());

                Some(model_name)
            })
        });

        if let Some(name) = result {
            chat_state.current_model_name = name;
        } else if let Some(model_id) = chat_state.current_model_id.as_ref() {
            chat_state.current_model_name = model_id.clone();
        }
    }

    /// Show model selector popup with all available models
    fn show_model_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return None;
                    }
                };

                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;
                let current_model_id = crate::model_selection::resolve_session_model_display_id(
                    &global_config.ai,
                    chat_state.current_model_id.as_deref(),
                );

                // Convert to ModelItem list (only enabled models)
                let model_items: Vec<ModelItem> = models
                    .into_iter()
                    .filter(|m| m.enabled)
                    .map(|m| ModelItem {
                        id: m.id,
                        name: m.name,
                        provider: m.provider,
                        model_name: m.model_name,
                    })
                    .collect();

                Some((model_items, current_model_id))
            })
        });

        match result {
            Some((models, current_id)) if !models.is_empty() => {
                chat_view.show_model_selector(models, current_id, !self.agent.is_shared(), true);
            }
            _ => {
                chat_state.add_system_message(
                    "No available models found. Please configure models first.".to_string(),
                );
            }
        }
    }

    /// Apply only the current Session model through the Runtime owner.
    fn apply_model_selection(
        &mut self,
        selected: &ModelItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let selected_id = selected.id.clone();
        let selected_display_name = format!("{} / {}", selected.model_name, selected.name);
        if chat_state.current_model_id.as_deref() == Some(selected_id.as_str()) {
            chat_view.set_status(Some(format!(
                "Current session already uses {selected_display_name}"
            )));
            return;
        }
        if !session_update_allowed(
            chat_state.is_processing,
            self.pending_session_update.is_some(),
        ) {
            chat_view.set_status(Some(session_update_unavailable_message(
                "Model",
                chat_state.is_processing,
            )));
            return;
        }
        let session_id = chat_state.core_session_id.clone();
        let task_session_id = session_id.clone();
        let task_model_id = selected_id.clone();
        let agent = self.agent.clone();
        chat_view.set_status(Some(format!(
            "Changing current session model to {selected_display_name}..."
        )));
        let handle = rt_handle.spawn(async move {
            agent
                .update_session_model(&task_session_id, &task_model_id)
                .await
        });
        self.pending_session_update = Some(PendingSessionUpdate {
            session_id,
            kind: PendingSessionUpdateKind::Model {
                model_id: selected_id,
                display_name: selected_display_name,
            },
            started_at: Instant::now(),
            slow_notice_shown: false,
            exit_warning_shown: false,
            handle,
        });
    }

    /// Show agent selector popup with all available agent modes
    fn show_agent_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let modes = self.get_mode_agents(rt_handle);
        if modes.is_empty() {
            let message = if self.agent.is_shared() {
                "Main agent modes are unavailable."
            } else {
                "Main agent modes are unavailable; agent management remains available."
            };
            chat_view.set_status(Some(message.to_string()));
            if self.agent.is_shared() {
                return;
            }
        }

        let agent_items: Vec<AgentItem> = modes
            .into_iter()
            .map(|m| AgentItem {
                id: m.id,
                description: m.description,
            })
            .collect();

        let allow_mode_switch = session_update_allowed(
            chat_state.is_processing,
            self.pending_session_update.is_some(),
        );
        if self.agent.is_shared() {
            chat_view.show_agent_modes_only(
                agent_items,
                Some(self.agent_type.clone()),
                allow_mode_switch,
            );
        } else {
            chat_view.show_agent_selector(
                agent_items,
                Some(self.agent_type.clone()),
                true,
                allow_mode_switch,
            );
        }
    }

    fn handle_agent_selector_action(
        &mut self,
        action: AgentSelectorAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match action {
            AgentSelectorAction::SwitchMode(selected) => {
                if !session_update_allowed(
                    chat_state.is_processing,
                    self.pending_session_update.is_some(),
                ) {
                    chat_view.set_status(Some(session_update_unavailable_message(
                        "Agent mode",
                        chat_state.is_processing,
                    )));
                    return;
                }
                chat_view.hide_agent_selector();
                self.apply_agent_selection(&selected, chat_view, chat_state, rt_handle);
            }
            AgentSelectorAction::ManageSubagents => {
                self.show_subagent_selector(chat_view, chat_state, rt_handle);
            }
            AgentSelectorAction::ReviewExternalSources => {
                chat_view.hide_agent_selector();
                self.handle_external_agent_review("", chat_view, chat_state, rt_handle);
            }
        }
    }

    /// Apply agent selection: switch agent type
    fn apply_agent_selection(
        &mut self,
        selected: &AgentItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if selected.id == self.agent_type {
            return;
        }

        if self.pending_session_update.is_some() {
            chat_view.set_status(Some(
                "A current session update is already in progress. Please wait.".to_string(),
            ));
            return;
        }

        let session_id = chat_state.core_session_id.clone();
        let mode_id = selected.id.clone();
        let task_mode_id = mode_id.clone();
        let agent = self.agent.clone();
        chat_view.set_status(Some(format!("Switching agent mode to {mode_id}...")));
        let task_session_id = session_id.clone();
        let handle = rt_handle.spawn(async move {
            agent
                .update_session_mode(&task_session_id, &task_mode_id)
                .await
        });
        self.pending_session_update = Some(PendingSessionUpdate {
            session_id,
            kind: PendingSessionUpdateKind::Mode { mode_id },
            started_at: Instant::now(),
            slow_notice_shown: false,
            exit_warning_shown: false,
            handle,
        });
    }

    fn poll_session_update_completion(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> SessionUpdatePollOutcome {
        let Some(pending) = self.pending_session_update.as_mut() else {
            return SessionUpdatePollOutcome::NoChange;
        };
        if !pending.handle.is_finished() {
            if !pending.slow_notice_shown
                && pending.started_at.elapsed() >= SESSION_UPDATE_SLOW_NOTICE
            {
                pending.slow_notice_shown = true;
                if !pending.exit_warning_shown {
                    let message = if self.agent.is_shared() {
                        "The current session update is still being saved. You can keep editing; changing sessions and sending wait for the result."
                    } else {
                        "The current session update is still being saved. You can edit or switch sessions; sending in this session waits."
                    };
                    chat_view.set_status(Some(message.to_string()));
                }
                return SessionUpdatePollOutcome::Redraw;
            }
            return SessionUpdatePollOutcome::NoChange;
        }
        let pending = self
            .pending_session_update
            .take()
            .expect("finished session update should remain present");
        let outcome = match tokio::task::block_in_place(|| rt_handle.block_on(pending.handle)) {
            Ok(Ok(())) => SessionUpdateApplyOutcome::Applied,
            Ok(Err(error)) if error.outcome_unknown() => {
                SessionUpdateApplyOutcome::OutcomeUnknown(error.to_string())
            }
            Ok(Err(error)) => SessionUpdateApplyOutcome::SessionUpdateFailed(error.to_string()),
            Err(error) => SessionUpdateApplyOutcome::SessionUpdateFailed(format!(
                "session update task failed: {error}"
            )),
        };
        let unknown_outcome = matches!(&outcome, SessionUpdateApplyOutcome::OutcomeUnknown(_));
        if chat_state.core_session_id != pending.session_id {
            if let SessionUpdateApplyOutcome::SessionUpdateFailed(error) = &outcome {
                tracing::error!(
                    "Failed to change previous session {} {} to {}: {}",
                    pending.session_id,
                    pending.kind.name(),
                    pending.kind.selected_id(),
                    error
                );
            }
            let status = previous_session_update_status(
                pending.kind.name(),
                pending.kind.selected_id(),
                &outcome,
            );
            chat_view.set_status(Some(status.clone()));
            if unknown_outcome {
                return SessionUpdatePollOutcome::ExitAfterUnknownOutcome(status);
            }
            return SessionUpdatePollOutcome::Redraw;
        }
        let applied = match &pending.kind {
            PendingSessionUpdateKind::Mode { mode_id } => {
                apply_agent_mode_feedback(&mut self.agent_type, chat_state, mode_id, outcome)
            }
            PendingSessionUpdateKind::Model {
                model_id,
                display_name,
            } => apply_model_selection_feedback(chat_state, display_name, model_id, outcome),
            PendingSessionUpdateKind::Rename { session_name } => {
                apply_session_rename_feedback(chat_state, session_name, outcome)
            }
        };
        if applied {
            chat_view.set_status(Some(format!(
                "Current session {} set to {}",
                pending.kind.name(),
                pending.kind.selected_id()
            )));
        } else if unknown_outcome {
            let message = format!(
                "Current session {} update outcome is unknown. This TUI is closing; reopen it, restore the session, and inspect its current {} before retrying.",
                pending.kind.name(),
                pending.kind.name()
            );
            chat_view.set_status(Some(message.clone()));
            return SessionUpdatePollOutcome::ExitAfterUnknownOutcome(message);
        } else {
            chat_view.set_status(Some(format!(
                "Current session {} change failed. Please retry.",
                pending.kind.name()
            )));
        }

        if applied
            && matches!(
                &pending.kind,
                PendingSessionUpdateKind::Mode { mode_id } if mode_id == "HarmonyOSDev"
            )
        {
            let deveco_home = std::env::var("DEVECO_HOME").ok();
            let missing = deveco_home
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if missing {
                chat_state.add_system_message(
                    "HarmonyOSDev tip: HmosCompilation requires DEVECO_HOME (DevEco Studio install path). If compilation fails, set DEVECO_HOME and restart the terminal."
                        .to_string(),
                );
            }
        }
        if session_update_completion_should_exit(pending.exit_warning_shown, applied) {
            SessionUpdatePollOutcome::ExitAfterSave
        } else {
            SessionUpdatePollOutcome::Redraw
        }
    }

    // ============ MCP management ============
}

fn session_update_allowed(is_processing: bool, update_pending: bool) -> bool {
    !is_processing && !update_pending
}

fn session_update_unavailable_message(setting_name: &str, is_processing: bool) -> String {
    if is_processing {
        format!("{setting_name} cannot be changed during the current turn.")
    } else {
        "A current session update is already in progress. Please wait.".to_string()
    }
}

#[cfg(test)]
mod usage_metadata_tests {
    use super::{
        session_update_allowed, session_update_unavailable_message, usage_report_metadata,
        SessionUsageReport,
    };

    #[test]
    fn session_update_is_rechecked_when_an_idle_popup_outlives_turn_start() {
        assert!(session_update_allowed(false, false));
        assert!(!session_update_allowed(true, false));
        assert!(!session_update_allowed(false, true));
    }

    #[test]
    fn active_turn_message_does_not_advertise_hidden_management() {
        let message = session_update_unavailable_message("Agent mode", true);

        assert_eq!(
            message,
            "Agent mode cannot be changed during the current turn."
        );
    }

    #[test]
    fn usage_metadata_preserves_the_existing_tui_transcript_schema() {
        let mut report = SessionUsageReport::partial_unavailable("session-1", 1_778_347_200_000);
        report.report_id = "usage-session-1-1778347200000".to_string();

        let metadata = usage_report_metadata(&report).expect("usage metadata");

        assert_eq!(metadata["localCommandKind"], "usage_report");
        assert_eq!(metadata["reportId"], report.report_id);
        assert_eq!(metadata["schemaVersion"], report.schema_version);
        assert_eq!(metadata["generatedAt"], report.generated_at);
        assert_eq!(metadata["modelVisible"], false);
        assert_eq!(metadata["usageReportStatus"], "completed");
        assert_eq!(metadata["usageReport"]["sessionId"], "session-1");
        assert_eq!(metadata.as_object().map(serde_json::Map::len), Some(7));
    }
}
