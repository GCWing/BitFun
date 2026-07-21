enum ModelSelectionApplyOutcome {
    SessionUpdateFailed(String),
    Applied {
        default_persist_error: Option<String>,
    },
}

enum ModeSelectionApplyOutcome {
    SessionUpdateFailed(String),
    Applied,
}

enum ModeChangePollOutcome {
    NoChange,
    Redraw,
    ExitAfterSave,
}

fn previous_session_mode_change_status(
    mode_id: &str,
    outcome: &ModeSelectionApplyOutcome,
) -> String {
    match outcome {
        ModeSelectionApplyOutcome::Applied => format!(
            "The previous session mode was changed to {mode_id}; the current session was not modified."
        ),
        ModeSelectionApplyOutcome::SessionUpdateFailed(error) => format!(
            "The previous session mode change to {mode_id} failed: {error}. Return to that session to retry."
        ),
    }
}

fn mode_change_completion_should_exit(exit_requested: bool, applied: bool) -> bool {
    exit_requested && applied
}

fn apply_agent_mode_feedback(
    current_mode: &mut String,
    chat_state: &mut ChatState,
    selected_mode: &str,
    outcome: ModeSelectionApplyOutcome,
) -> bool {
    match outcome {
        ModeSelectionApplyOutcome::SessionUpdateFailed(error) => {
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
        ModeSelectionApplyOutcome::Applied => {
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
    outcome: ModelSelectionApplyOutcome,
) {
    match outcome {
        ModelSelectionApplyOutcome::SessionUpdateFailed(error) => {
            tracing::error!(
                "Failed to switch model to {} ({}): {}",
                selected_display_name,
                selected_id,
                error
            );
            chat_state.add_system_message(format!(
                "Current session model was not changed: {error}. Please retry."
            ));
        }
        ModelSelectionApplyOutcome::Applied {
            default_persist_error,
        } => {
            chat_state.current_model_name = selected_display_name.to_string();
            tracing::info!(
                "Model switched to: {} ({})",
                selected_display_name,
                selected_id
            );
            if let Some(error) = default_persist_error {
                tracing::warn!(
                    "Current session model changed, but the future default could not be saved: {}",
                    error
                );
                chat_state.add_system_message(
                    "Model switched for the current session, but the default for future sessions could not be saved. Check configuration storage and retry if needed."
                        .to_string(),
                );
            }
        }
    }
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
        let runtime = Arc::clone(&self.runtime);

        let report_result: Result<bitfun_core::service::session_usage::SessionUsageReport> =
            tokio::task::block_in_place(|| {
                let session_id = session_id.clone();
                let workspace_path = workspace_path.clone();
                let agent = agent.clone();
                let runtime = Arc::clone(&runtime);
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
                    runtime
                        .agent_runtime()
                        .record_completed_local_command_turn(AgentLocalCommandTurnRecordRequest {
                            session_id,
                            content: markdown,
                            turn_id: Some(format!("local-usage-{}", report.report_id)),
                            timestamp_ms: Some(generated_at),
                            metadata: metadata.as_object().cloned().ok_or_else(|| {
                                anyhow!("Usage report metadata must be an object")
                            })?,
                        })
                        .await
                        .map_err(|error| anyhow!(error.into_message()))?;

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
        if !agent_mode_switch_allowed(chat_state.is_processing, self.pending_mode_change.is_some())
        {
            chat_view.set_status(Some(mode_switch_unavailable_message(
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

    /// Load current model name from global config for display
    fn load_current_model_name(
        &self,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let result: Option<String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                let model_id = crate::model_selection::resolve_mode_model_id(&global_config.ai)?;

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
                    .map(model_display_name);

                model_name
            })
        });

        if let Some(name) = result {
            chat_state.current_model_name = name;
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

                let current_model_id =
                    crate::model_selection::resolve_mode_model_id(&global_config.ai);

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
                chat_view.show_model_selector(models, current_id);
            }
            _ => {
                chat_state.add_system_message(
                    "No available models found. Please configure models first.".to_string(),
                );
            }
        }
    }

    /// Apply the current-session model and best-effort future-session default.
    fn apply_model_selection(
        &self,
        selected: &ModelItem,
        _chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let selected_id = selected.id.clone();
        let selected_display_name = format!("{} / {}", selected.model_name, selected.name);
        let session_id = chat_state.core_session_id.clone();

        let outcome = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                if let Err(e) = self
                    .agent
                    .update_session_model(&session_id, &selected_id)
                    .await
                {
                    return ModelSelectionApplyOutcome::SessionUpdateFailed(e.to_string());
                }

                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        return ModelSelectionApplyOutcome::Applied {
                            default_persist_error: Some(e.to_string()),
                        };
                    }
                };

                if let Err(e) = config_service
                    .set_config("ai.agent_model_defaults.mode", &selected_id)
                    .await
                {
                    return ModelSelectionApplyOutcome::Applied {
                        default_persist_error: Some(e.to_string()),
                    };
                }

                crate::account_sync::notify_local_settings_changed();

                ModelSelectionApplyOutcome::Applied {
                    default_persist_error: None,
                }
            })
        });

        apply_model_selection_feedback(chat_state, &selected_display_name, &selected_id, outcome);
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
            chat_view.set_status(Some(
                "Main agent modes are unavailable; agent management remains available.".to_string(),
            ));
        }

        let agent_items: Vec<AgentItem> = modes
            .into_iter()
            .map(|m| AgentItem {
                id: m.id,
                description: m.description,
            })
            .collect();

        chat_view.show_agent_selector(
            agent_items,
            Some(self.agent_type.clone()),
            true,
            agent_mode_switch_allowed(chat_state.is_processing, self.pending_mode_change.is_some()),
        );
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
                if !agent_mode_switch_allowed(
                    chat_state.is_processing,
                    self.pending_mode_change.is_some(),
                ) {
                    chat_view.set_status(Some(mode_switch_unavailable_message(
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

        if self.pending_mode_change.is_some() {
            chat_view.set_status(Some(
                "An agent mode change is already in progress. Please wait.".to_string(),
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
        self.pending_mode_change = Some(PendingModeChange {
            session_id,
            mode_id,
            started_at: Instant::now(),
            slow_notice_shown: false,
            exit_warning_shown: false,
            handle,
        });
    }

    fn poll_mode_change_completion(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> ModeChangePollOutcome {
        let Some(pending) = self.pending_mode_change.as_mut() else {
            return ModeChangePollOutcome::NoChange;
        };
        if !pending.handle.is_finished() {
            if !pending.slow_notice_shown && pending.started_at.elapsed() >= MODE_CHANGE_SLOW_NOTICE
            {
                pending.slow_notice_shown = true;
                if !pending.exit_warning_shown {
                    chat_view.set_status(Some(
                        "The agent mode change is still being saved. You can edit or switch sessions; sending in this session waits."
                            .to_string(),
                    ));
                }
                return ModeChangePollOutcome::Redraw;
            }
            return ModeChangePollOutcome::NoChange;
        }
        let pending = self
            .pending_mode_change
            .take()
            .expect("finished mode task should remain present");
        let outcome = match tokio::task::block_in_place(|| rt_handle.block_on(pending.handle)) {
            Ok(Ok(())) => ModeSelectionApplyOutcome::Applied,
            Ok(Err(error)) => ModeSelectionApplyOutcome::SessionUpdateFailed(error.to_string()),
            Err(error) => ModeSelectionApplyOutcome::SessionUpdateFailed(format!(
                "mode update task failed: {error}"
            )),
        };
        if chat_state.core_session_id != pending.session_id {
            if let ModeSelectionApplyOutcome::SessionUpdateFailed(error) = &outcome {
                tracing::error!(
                    "Failed to switch previous session {} to agent mode {}: {}",
                    pending.session_id,
                    pending.mode_id,
                    error
                );
            }
            chat_view.set_status(Some(previous_session_mode_change_status(
                &pending.mode_id,
                &outcome,
            )));
            return ModeChangePollOutcome::Redraw;
        }
        let applied =
            apply_agent_mode_feedback(&mut self.agent_type, chat_state, &pending.mode_id, outcome);
        if applied {
            chat_view.set_status(Some(format!("Agent mode set to {}", pending.mode_id)));
        } else {
            chat_view.set_status(Some("Agent mode change failed. Please retry.".to_string()));
        }

        if applied && pending.mode_id == "HarmonyOSDev" {
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
        if mode_change_completion_should_exit(pending.exit_warning_shown, applied) {
            ModeChangePollOutcome::ExitAfterSave
        } else {
            ModeChangePollOutcome::Redraw
        }
    }

    // ============ MCP management ============
}

fn agent_mode_switch_allowed(is_processing: bool, mode_change_pending: bool) -> bool {
    !is_processing && !mode_change_pending
}

fn mode_switch_unavailable_message(is_processing: bool) -> String {
    if is_processing {
        "Agent mode cannot be changed during the current turn. Subagent and external source management remain available."
            .to_string()
    } else {
        "An agent mode change is already in progress. Please wait.".to_string()
    }
}

#[cfg(test)]
mod usage_metadata_tests {
    use super::{agent_mode_switch_allowed, usage_report_metadata, SessionUsageReport};

    #[test]
    fn mode_switch_is_rechecked_when_an_idle_popup_outlives_turn_start() {
        assert!(agent_mode_switch_allowed(false, false));
        assert!(!agent_mode_switch_allowed(true, false));
        assert!(!agent_mode_switch_allowed(false, true));
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
