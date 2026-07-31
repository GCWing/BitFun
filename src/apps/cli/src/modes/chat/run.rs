fn primary_model_usage_for_active_turn(
    event: &AgenticEvent,
    chat_state: &ChatState,
) -> Option<ModelTokenUsageSnapshot> {
    let AgenticEvent::TokenUsageUpdated {
        session_id,
        turn_id,
        model_config_id,
        effective_model_name,
        input_tokens,
        output_tokens,
        total_tokens,
        max_context_tokens,
        is_subagent,
        cached_tokens,
        ..
    } = event
    else {
        return None;
    };
    if *is_subagent
        || session_id != &chat_state.core_session_id
        || chat_state.current_turn_id() != Some(turn_id.as_str())
    {
        return None;
    }

    Some(ModelTokenUsageSnapshot {
        model_config_id: model_config_id.clone(),
        effective_model_name: effective_model_name.clone(),
        input_tokens: *input_tokens,
        output_tokens: *output_tokens,
        total_tokens: *total_tokens,
        max_context_tokens: *max_context_tokens,
        cached_tokens: *cached_tokens,
    })
}

fn context_compression_tool_event(
    event: &AgenticEvent,
    chat_state: &ChatState,
) -> Option<ToolEventData> {
    let (session_id, turn_id) = match event {
        AgenticEvent::ContextCompressionStarted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ContextCompressionCompleted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ContextCompressionFailed {
            session_id,
            turn_id,
            ..
        } => (session_id, turn_id),
        _ => return None,
    };
    if session_id != &chat_state.core_session_id
        || chat_state.current_turn_id() != Some(turn_id.as_str())
    {
        return None;
    }

    match event {
        AgenticEvent::ContextCompressionStarted {
            compression_id,
            trigger,
            tokens_before,
            context_window,
            ..
        } => Some(ToolEventData::Started {
            identity: ToolEventIdentity::direct(compression_id, "ContextCompression"),
            params: serde_json::json!({
                "trigger": trigger,
                "tokens_before": tokens_before,
                "context_window": context_window,
            }),
            timeout_seconds: None,
        }),
        AgenticEvent::ContextCompressionCompleted {
            compression_id,
            compression_count,
            tokens_before,
            tokens_after,
            compression_ratio,
            duration_ms,
            has_summary,
            summary_source,
            applied,
            ..
        } => Some(ToolEventData::Completed {
            identity: ToolEventIdentity::direct(compression_id, "ContextCompression"),
            result: serde_json::json!({
                "compression_count": compression_count,
                "tokens_before": tokens_before,
                "tokens_after": tokens_after,
                "compression_ratio": compression_ratio,
                "duration": duration_ms,
                "applied": applied,
                "has_summary": has_summary,
                "summary_source": summary_source,
            }),
            result_for_assistant: None,
            image_attachments: None,
            duration_ms: *duration_ms,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: Some(*duration_ms),
        }),
        AgenticEvent::ContextCompressionFailed {
            compression_id,
            error,
            ..
        } => Some(ToolEventData::Failed {
            identity: ToolEventIdentity::direct(compression_id, "ContextCompression"),
            error: error.clone(),
            duration_ms: None,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
        }),
        _ => None,
    }
}

impl ChatMode {
    pub(crate) fn run(
        &mut self,
        existing_terminal: Option<TerminalGuard>,
    ) -> Result<ChatExitReason> {
        tracing::info!("Starting Chat mode, Agent: {}", self.agent_type);
        if let Some(ws) = &self.workspace {
            tracing::info!("Workspace: {}", ws);
        }

        let mut terminal = match existing_terminal {
            Some(t) => t,
            None => init_terminal()?,
        };

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
        let theme = self.resolve_configured_theme(base, appearance, scheme);
        let shortcut_hints = self.keymap.compact_hints(self.action_state(false, false));
        let mut chat_view = ChatView::new(theme, shortcut_hints);

        // Create or restore core session
        let rt_handle = tokio::runtime::Handle::current();
        self.auto_approve_ask_default = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let Ok(service) = bitfun_core::service::config::get_global_config_service().await
                else {
                    return false;
                };
                service
                    .get_config::<bitfun_core::service::config::types::GlobalConfig>(None)
                    .await
                    .map(|config| config.tool_permissions.interaction.auto_approve_ask)
                    .unwrap_or(false)
            })
        });

        let (mut session_id, mut chat_state, migration_notices) =
            if let Some(ref restore_id) = self.restore_session_id {
                // Restore existing session
                tracing::info!("Restoring session: {}", restore_id);
                let agent = self.agent.clone();
                let rid = restore_id.clone();

                tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        // Restore session in core (loads metadata, messages, managers)
                        let (summary, workspace_binding, migration_notices, transcript) =
                            agent.restore_session_in_current_workspace(&rid).await?;
                        let effective_workspace = Some(workspace_binding.workspace_path.clone());

                        let mut state = ChatState::from_session_transcript(
                            rid.clone(),
                            summary.session_name,
                            summary.agent_type,
                            effective_workspace,
                            &transcript,
                        );
                        state.current_model_id = summary.model_id;
                        state.apply_workspace_binding(workspace_binding);

                        tracing::info!(
                            "Session restored: {}, {} messages loaded",
                            rid,
                            transcript.messages.len()
                        );

                        Ok::<_, anyhow::Error>((rid, state, migration_notices))
                    })
                })?
            } else {
                // Create new session
                let agent = self.agent.clone();
                let agent_type = self.agent_type.clone();
                let (session_id, workspace_binding) = tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let session_id = agent.ensure_session(&agent_type).await?;
                        let binding = agent.session_workspace_binding(&session_id).await?;
                        Ok::<_, anyhow::Error>((session_id, binding))
                    })
                })?;
                tracing::info!("Core session ready: {}", session_id);

                let mut state = ChatState::new(
                    session_id.clone(),
                    "CLI Session".to_string(),
                    self.agent_type.clone(),
                    Some(workspace_binding.workspace_path.clone()),
                );
                state.apply_workspace_binding(workspace_binding);
                (session_id, state, Vec::new())
            };
        chat_state.set_worktree_control_available(!self.agent.is_shared());
        self.auto_approve_ask_override = None;
        self.agent
            .set_approval_policy(crate::runtime::approval::CliApprovalPolicy::Ask);
        chat_state.auto_approve_ask = self.auto_approve_ask_default;

        // Keep ChatMode workspace in sync with the session's effective workspace
        self.agent_type = chat_state.agent_type.clone();
        self.workspace = chat_state.workspace.clone();
        self.refresh_workspace_git_status(&mut chat_state, &rt_handle);

        let mut external_source_rx = None;
        if self.agent.is_shared() {
            chat_view.set_status(Some(format!(
                "{SHARED_TUI_CHAT_STATUS} {SHARED_TUI_EMBEDDED_HANDOFF}"
            )));
        } else {
            let external_workspace = self.agent.workspace_path_buf();
            let (initial_external_sources, updates, conflict_preferences) =
                tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let updates =
                            subscribe_external_source_updates(Some(&external_workspace)).await;
                        let snapshot =
                            external_source_snapshot(Some(&external_workspace), false).await;
                        let preferences = external_source_conflict_choices().await.map(Into::into);
                        (snapshot, updates.ok(), preferences)
                    })
                });
            external_source_rx = updates;
            match conflict_preferences {
                Ok(preferences) => self.replace_external_conflict_preferences(preferences),
                Err(error) => {
                    tracing::warn!("External source preferences are unavailable: {}", error)
                }
            }
            match initial_external_sources {
                Ok(snapshot) => {
                    let (available, restricted) = external_command_counts(&snapshot);
                    let pending_conflicts = snapshot
                        .command_conflicts
                        .iter()
                        .filter(|conflict| conflict.selected_candidate_id.is_none())
                        .count();
                    let tool_notice = self.take_external_tool_notice(&snapshot);
                    let agent_notice = self.take_external_agent_notice(&snapshot);
                    self.update_external_source_view(&mut chat_view, &snapshot);
                    self.external_source_snapshot = Some(snapshot.clone());
                    if snapshot.discovery_pending {
                        chat_view.set_status(Some(
                            "Checking compatible content from external AI applications".to_string(),
                        ));
                    } else if tool_notice.is_some() || agent_notice.is_some() {
                        chat_view.set_status(Some(
                            [tool_notice, agent_notice]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join("; "),
                        ));
                    } else if available + restricted > 0 || pending_conflicts > 0 {
                        chat_view.set_status(Some(format!(
                            "External sources: {available} commands available, {restricted} restricted, {pending_conflicts} need a choice"
                        )));
                    }
                }
                Err(error) => {
                    tracing::warn!("External source discovery is unavailable: {}", error);
                }
            }
        }

        // Load current model name for display
        self.load_current_model_name(&mut chat_state, &rt_handle);

        if self.agent_type == "HarmonyOSDev" {
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

        let mut event_rx = self
            .agent
            .subscribe_events()
            .map_err(|error| anyhow::anyhow!(error.into_message()))?;
        let mut permission_rx = self.agent.subscribe_permission_requests().ok();
        if let Ok(pending) = self.agent.pending_permission_requests() {
            for request in pending.into_iter().filter(|request| {
                crate::runtime::approval::permission_request_targets_session(request, &session_id)
            }) {
                chat_state.enqueue_permission_request(request);
            }
        }

        for notice in &migration_notices {
            chat_state.add_system_message(notice.user_message());
        }

        // Send initial prompt if provided (from startup page input)
        if let Some(prompt) = self.initial_prompt.take() {
            if !migration_notices.is_empty() {
                chat_view.text_input.set_text(&prompt);
                chat_view.set_status(Some(
                    "The restored session uses fallback settings. Review them, then send the preserved input explicitly."
                        .to_string(),
                ));
            } else if prompt.starts_with('/') {
                // Slash commands will be handled in the main loop
                chat_view.text_input.set_text(&prompt);
            } else {
                tracing::info!("Sending initial prompt: {}", prompt);
                let display_name = agent_display_name(&self.agent_type);
                chat_view.set_status(Some(format!("{} is thinking...", display_name)));

                let agent = self.agent.clone();
                let agent_type = self.agent_type.clone();
                match tokio::task::block_in_place(|| {
                    rt_handle.block_on(agent.send_message(prompt, &agent_type))
                }) {
                    Ok(turn_id) => {
                        tracing::info!("Started initial turn: {}", turn_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to send initial prompt: {}", e);
                        chat_view.set_status(Some(format!("Error: {}", e)));
                    }
                }
            }
        }

        let mut exit_reason = ChatExitReason::Quit;
        let mut should_quit = false;
        let mut needs_redraw = true;
        let mut subagent_parent_tools: HashMap<String, String> = HashMap::new();
        let mut last_spinner_redraw = Instant::now();
        let mut event_reader = crate::ui::input::EventReader::default();
        let mut fatal_event_stream_error: Option<String> = None;
        let spinner_redraw_interval = Duration::from_millis(SPINNER_REDRAW_INTERVAL_MS);
        let resize_redraw_debounce = Duration::from_millis(RESIZE_REDRAW_DEBOUNCE_MS);
        let mut resize_redraw = ResizeRedrawState::new(resize_redraw_debounce);

        while !should_quit {
            chat_view.set_action_state(
                self.action_state(chat_state.is_processing, false),
                &self.keymap,
            );
            chat_view.set_agent_mode_switch_allowed(session_update_allowed(
                chat_state.is_processing,
                self.pending_session_operation.is_some(),
            ));

            // Keep spinner animation smooth without forcing full redraw every loop.
            // Pause spinner updates while resize is still being debounced.
            if resize_redraw.is_pending() {
                last_spinner_redraw = Instant::now();
            } else if chat_state.is_processing {
                if last_spinner_redraw.elapsed() >= spinner_redraw_interval {
                    needs_redraw = true;
                    last_spinner_redraw = Instant::now();
                }
            } else {
                last_spinner_redraw = Instant::now();
            }

            // Poll completion of non-blocking MCP operations before rendering.
            if self.poll_mcp_task_completion(&mut chat_view, &mut chat_state, &rt_handle) {
                needs_redraw = true;
            }
            match self.poll_session_operation_completion(
                &mut chat_view,
                &mut chat_state,
                &rt_handle,
            ) {
                SessionUpdatePollOutcome::NoChange => {}
                SessionUpdatePollOutcome::Redraw => needs_redraw = true,
                SessionUpdatePollOutcome::ExitAfterSave => {
                    should_quit = true;
                    exit_reason = ChatExitReason::Quit;
                    continue;
                }
                SessionUpdatePollOutcome::ExitAfterUnknownOutcome(message) => {
                    fatal_event_stream_error = Some(message);
                    break;
                }
            }
            if self.poll_external_tool_mutation(&mut chat_view) {
                needs_redraw = true;
            }
            if self.poll_external_agent_mutation(&mut chat_view) {
                needs_redraw = true;
            }
            if self.poll_external_control_mutation(&mut chat_view) {
                needs_redraw = true;
            }
            if self.poll_hook_management(&mut chat_view, &mut chat_state) {
                needs_redraw = true;
            }

            if let Some(receiver) = permission_rx.as_mut() {
                for _ in 0..4 {
                    match receiver.try_recv() {
                        Ok(bitfun_agent_runtime::sdk::PermissionRequestEvent::Asked {
                            request,
                        }) if crate::runtime::approval::permission_request_targets_session(
                            &request,
                            &session_id,
                        ) =>
                        {
                            if chat_state.enqueue_permission_request(request) {
                                needs_redraw = true;
                            }
                        }
                        Ok(bitfun_agent_runtime::sdk::PermissionRequestEvent::Replied {
                            request_id,
                            ..
                        })
                        | Ok(bitfun_agent_runtime::sdk::PermissionRequestEvent::Cancelled {
                            request_id,
                            ..
                        }) => {
                            if chat_state.resolve_permission_request(&request_id) {
                                needs_redraw = true;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Lagged(_)) => {
                            match self.agent.pending_permission_requests() {
                                Ok(requests) => {
                                    let requests = requests
                                        .into_iter()
                                        .filter(|request| {
                                            crate::runtime::approval::permission_request_targets_session(
                                                request,
                                                &session_id,
                                            )
                                        })
                                        .collect();
                                    if chat_state.reconcile_permission_requests(requests) {
                                        needs_redraw = true;
                                    }
                                }
                                Err(error) => {
                                    let mut failure = format!(
                                        "Shared Runtime permission state could not be resynchronized: {}",
                                        error.into_message()
                                    );
                                    let agent = self.agent.clone();
                                    if let Err(error) = tokio::task::block_in_place(|| {
                                        rt_handle.block_on(agent.cancel_current_turn())
                                    }) {
                                        failure = format!(
                                            "{failure}; failed to cancel the active turn: {error}"
                                        );
                                    }
                                    mark_active_turn_failed(&mut chat_state, &failure);
                                    chat_view.set_status(Some(format!("Error: {failure}")));
                                    fatal_event_stream_error = Some(failure);
                                }
                            }
                            break;
                        }
                        Err(TryRecvError::Closed) => {
                            let mut failure =
                                "Shared Runtime permission event stream closed".to_string();
                            let agent = self.agent.clone();
                            if let Err(error) = tokio::task::block_in_place(|| {
                                rt_handle.block_on(agent.cancel_current_turn())
                            }) {
                                failure =
                                    format!("{failure}; failed to cancel the active turn: {error}");
                            }
                            mark_active_turn_failed(&mut chat_state, &failure);
                            chat_view.set_status(Some(format!("Error: {failure}")));
                            fatal_event_stream_error = Some(failure);
                            break;
                        }
                        Ok(_) => {}
                    }
                }
            }

            let mut external_source_closed = false;
            if let Some(receiver) = external_source_rx.as_mut() {
                let mut latest = None;
                for _ in 0..4 {
                    match receiver.try_recv() {
                        Ok(snapshot) => latest = Some(snapshot),
                        Err(TryRecvError::Lagged(_)) => continue,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Closed) => {
                            external_source_closed = true;
                            break;
                        }
                    }
                }
                if let Some(snapshot) = latest {
                    let discovery_just_finished = self
                        .external_source_snapshot
                        .as_ref()
                        .is_some_and(|previous| previous.discovery_pending)
                        && !snapshot.discovery_pending;
                    let preferences = tokio::task::block_in_place(|| {
                        rt_handle
                            .block_on(external_source_conflict_choices())
                            .map(Into::into)
                    });
                    if let Ok(preferences) = preferences {
                        self.replace_external_conflict_preferences(preferences);
                    }
                    let tool_notice = self.take_external_tool_notice(&snapshot);
                    let agent_notice = self.take_external_agent_notice(&snapshot);
                    self.update_external_source_view(&mut chat_view, &snapshot);
                    if snapshot.discovery_pending {
                        chat_view.set_status(Some(
                            "Checking compatible content from external AI applications".to_string(),
                        ));
                    } else if tool_notice.is_some() || agent_notice.is_some() {
                        chat_view.set_status(Some(
                            [tool_notice, agent_notice]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join("; "),
                        ));
                    } else if discovery_just_finished {
                        let (available, restricted) = external_command_counts(&snapshot);
                        let pending_conflicts = snapshot
                            .command_conflicts
                            .iter()
                            .filter(|conflict| conflict.selected_candidate_id.is_none())
                            .count();
                        chat_view.set_status(Some(format!(
                            "External sources ready: {available} commands available, {restricted} restricted, {pending_conflicts} need a choice"
                        )));
                    }
                    self.external_source_snapshot = Some(snapshot);
                    if chat_view.mcp_selector_visible() {
                        chat_view.mcp_selector_cancel_confirm_external();
                        chat_view.mcp_selector_update_items(self.get_mcp_items(&rt_handle));
                    }
                    needs_redraw = true;
                }
            }
            if external_source_closed {
                external_source_rx = None;
            }

            if chat_view.login_form_visible() {
                self.refresh_account_panel_live(&mut chat_view);
                if crate::account_sync::sync_in_flight() {
                    needs_redraw = true;
                }
            }

            let mut did_render_this_loop = false;
            if needs_redraw && resize_redraw.can_render() {
                terminal.draw(|frame| {
                    chat_view.render(frame, &chat_state);
                })?;
                needs_redraw = false;
                did_render_this_loop = true;
            }

            // 1.5. Execute pending MCP operations (after render so loading state is visible)
            if resize_redraw.can_render() {
                if let Some(op) = self.pending_mcp_op.take() {
                    if !did_render_this_loop {
                        terminal.draw(|frame| {
                            chat_view.render(frame, &chat_state);
                        })?;
                    }
                    match op {
                        PendingMcpOp::Toggle(server_id) => {
                            self.execute_mcp_toggle(
                                &server_id,
                                &mut chat_view,
                                &mut chat_state,
                                &rt_handle,
                            );
                        }
                        PendingMcpOp::External(item) => {
                            self.execute_external_mcp_action(
                                item,
                                &mut chat_view,
                                &mut chat_state,
                                &rt_handle,
                            );
                        }
                        PendingMcpOp::Add { name, config_json } => {
                            self.execute_mcp_add(
                                &name,
                                &config_json,
                                &mut chat_view,
                                &mut chat_state,
                                &rt_handle,
                            );
                        }
                        PendingMcpOp::Delete(server_id) => {
                            self.execute_mcp_delete(
                                &server_id,
                                &mut chat_view,
                                &mut chat_state,
                                &rt_handle,
                            );
                        }
                    }
                    needs_redraw = true;
                }
            }

            // 2. Process core events (non-blocking)
            let mut events = Vec::with_capacity(20);
            for _ in 0..20 {
                match event_rx.try_recv() {
                    Ok(envelope) => events.push(envelope),
                    Err(error) => {
                        let Some(mut failure) = agent_event_stream_failure(error) else {
                            break;
                        };

                        // The adapter records the turn before DialogTurnStarted reaches the UI,
                        // so cancellation must not depend on ChatState having seen that event.
                        let agent = self.agent.clone();
                        if let Err(cancel_error) = tokio::task::block_in_place(|| {
                            rt_handle.block_on(agent.cancel_current_turn())
                        }) {
                            failure = format!(
                                "{failure}; failed to cancel the active turn: {cancel_error}"
                            );
                        }
                        mark_active_turn_failed(&mut chat_state, &failure);
                        chat_view.invalidate_lines_cache();
                        chat_view.set_status(Some(format!("Error: {failure}")));
                        tracing::error!("{failure}");
                        fatal_event_stream_error = Some(failure);
                        break;
                    }
                }
            }
            if fatal_event_stream_error.is_some() {
                break;
            }
            for envelope in events {
                let event = &envelope.event;

                if let AgenticEvent::SubagentSessionLinked {
                    session_id: subagent_session_id,
                    parent_session_id,
                    parent_tool_call_id,
                    ..
                } = event
                {
                    if parent_session_id == &session_id {
                        subagent_parent_tools
                            .insert(subagent_session_id.clone(), parent_tool_call_id.clone());
                    }
                    continue;
                }

                // Check if this is a subagent event that belongs to our session
                if event.session_id() != Some(&session_id) {
                    // Check if this event was emitted by a subagent whose parent is in our session
                    if let Some(parent_tool_call_id) = event
                        .session_id()
                        .and_then(|event_session_id| subagent_parent_tools.get(event_session_id))
                    {
                        // Forward subagent event to the parent Task tool for progress display
                        chat_state.handle_subagent_event(parent_tool_call_id, event);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
                    }
                    continue;
                }

                tracing::debug!("Processing core event: {:?}", event);

                if event
                    .turn_id()
                    .is_some_and(|turn_id| chat_state.should_ignore_turn_event(turn_id))
                {
                    tracing::debug!(
                        "Ignoring event for a Turn retired by authoritative Session state: turn_id={:?}",
                        event.turn_id()
                    );
                    continue;
                }

                match event {
                    AgenticEvent::DialogTurnStarted {
                        turn_id,
                        user_input,
                        ..
                    } => {
                        chat_state.handle_turn_started(turn_id, user_input);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
                    }

                    AgenticEvent::TextChunk { turn_id, text, .. } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_text_chunk(text);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        } else {
                            tracing::debug!(
                                "Ignoring TextChunk for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::ThinkingChunk {
                        turn_id, content, ..
                    } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_thinking_chunk(content);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        } else {
                            tracing::debug!(
                                "Ignoring ThinkingChunk for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::ToolEvent {
                        turn_id,
                        tool_event,
                        ..
                    } => {
                        if chat_state.current_turn_id() != Some(turn_id.as_str()) {
                            tracing::debug!(
                                "Ignoring ToolEvent for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                            continue;
                        }
                        chat_state.handle_tool_event(tool_event);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
                    }

                    AgenticEvent::ContextCompressionStarted { .. }
                    | AgenticEvent::ContextCompressionCompleted { .. }
                    | AgenticEvent::ContextCompressionFailed { .. } => {
                        if let Some(tool_event) = context_compression_tool_event(event, &chat_state)
                        {
                            if matches!(event, AgenticEvent::ContextCompressionStarted { .. }) {
                                chat_view.set_status(Some("Compacting context...".to_string()));
                            }
                            chat_state.handle_tool_event(&tool_event);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        }
                    }

                    AgenticEvent::DialogTurnCompleted {
                        turn_id,
                        total_rounds,
                        total_tools,
                        ..
                    } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_turn_completed(*total_rounds, *total_tools);
                            self.refresh_workspace_git_status(&mut chat_state, &rt_handle);
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(None);
                            needs_redraw = true;
                            tracing::info!("Dialog turn completed");
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnCompleted for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::DialogTurnFailed { turn_id, error, .. } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_turn_failed(error);
                            self.refresh_workspace_git_status(&mut chat_state, &rt_handle);
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(Some(format!("Error: {}", error)));
                            needs_redraw = true;
                            tracing::error!("Dialog turn failed: {}", error);
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnFailed for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::DialogTurnCancelled { turn_id, .. } => {
                        if chat_state.should_apply_turn_cancelled(turn_id) {
                            chat_state.handle_turn_cancelled();
                            self.refresh_workspace_git_status(&mut chat_state, &rt_handle);
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(Some("Cancelled".to_string()));
                            needs_redraw = true;
                            tracing::info!("Dialog turn cancelled");
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnCancelled for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::TokenUsageUpdated { .. } => {
                        if let Some(usage) = primary_model_usage_for_active_turn(event, &chat_state)
                        {
                            chat_state.handle_primary_model_usage(usage);
                            needs_redraw = true;
                        }
                    }

                    AgenticEvent::SessionModelAutoMigrated {
                        session_id,
                        previous_model_id,
                        new_model_id,
                        reason,
                        ..
                    } => {
                        if apply_session_model_migration(
                            &mut chat_state,
                            session_id,
                            previous_model_id,
                            new_model_id,
                            reason,
                        ) {
                            self.load_current_model_name(&mut chat_state, &rt_handle);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        }
                    }

                    AgenticEvent::SystemError { error, .. } => {
                        chat_state.add_system_message(format!("[System error: {}]", error));
                        chat_view.invalidate_lines_cache();
                        chat_view.set_status(Some(format!("System error: {}", error)));
                        needs_redraw = true;
                        tracing::error!("System error: {}", error);
                    }

                    // Other events we don't need to handle in the UI
                    _ => {}
                }
            }

            // 3. Process terminal input
            if let Some(events) = event_reader.read_event_batch(Duration::from_millis(16))? {
                for event in events {
                    match event {
                        Event::Key(key) => {
                            if let Some(reason) = self.handle_key_event(
                                key,
                                &mut chat_view,
                                &mut chat_state,
                                &rt_handle,
                            )? {
                                Self::apply_exit_reason(
                                    reason,
                                    ChatEventContext {
                                        this: self,
                                        chat_view: &mut chat_view,
                                        chat_state: &mut chat_state,
                                        session_id: &mut session_id,
                                        rt_handle: &rt_handle,
                                        should_quit: &mut should_quit,
                                        exit_reason: &mut exit_reason,
                                    },
                                );
                            }
                            if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                                needs_redraw = true;
                            }
                        }
                        other => {
                            let outcome = Self::handle_non_key_event(
                                other,
                                ChatEventContext {
                                    this: self,
                                    chat_view: &mut chat_view,
                                    chat_state: &mut chat_state,
                                    session_id: &mut session_id,
                                    rt_handle: &rt_handle,
                                    should_quit: &mut should_quit,
                                    exit_reason: &mut exit_reason,
                                },
                            )?;
                            if outcome.request_redraw {
                                needs_redraw = true;
                            }
                            if outcome.resize_observed {
                                resize_redraw.observe(Instant::now());
                            }
                        }
                    }
                }
            }

            // Only invalidate after the complete input batch has been drained. The
            // next draw then uses Ratatui's current backend size instead of a stale
            // dimension captured from an earlier resize event in the same burst.
            if resize_redraw.take_ready(Instant::now()) {
                chat_view.invalidate_lines_cache();
                needs_redraw = true;
            }
        }

        let terminal_restore_result = restore_terminal(terminal);
        if let Some(failure) = fatal_event_stream_error {
            if let Err(restore_error) = terminal_restore_result {
                return Err(anyhow!(
                    "{failure}; failed to restore the terminal: {restore_error}"
                ));
            }
            return Err(anyhow!(failure));
        }
        terminal_restore_result?;
        tracing::info!("Chat mode exited");

        Ok(exit_reason)
    }
}
