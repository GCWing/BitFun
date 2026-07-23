fn mode_change_blocks_typed_submission(pending_for_current_session: bool, input: &str) -> bool {
    pending_for_current_session && !input.trim().starts_with('/')
}

impl ChatMode {
    /// Handle command palette action
    fn handle_palette_action(
        &mut self,
        action_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        // Hide command palette but keep it in stack for back navigation
        // (unless the action switches away or exits)
        let keep_in_stack = matches!(action_id, "new_session" | "exit");
        if !keep_in_stack {
            chat_view.hide_command_palette();
        }
        self.handle_action_id(action_id, chat_view, chat_state, rt_handle)
    }

    fn handle_action_id(
        &mut self,
        action_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if action_id == "toggle_auto_approve" || action_id.starts_with("toggle_auto_approve:") {
            let action = action_by_id("toggle_auto_approve", ActionContext::Chat)
                .expect("Auto mode action must remain registered");
            let state = ActionState::chat(chat_state.is_processing, false);
            if !action.available(state) {
                chat_view.set_status(Some(action.unavailable_message(state)));
                return Ok(None);
            }
            let argument = action_id.strip_prefix("toggle_auto_approve:");
            let next = match argument {
                Some("on") => Some(true),
                Some("off") => Some(false),
                Some("default") => None,
                _ => Some(!chat_state.auto_approve_ask),
            };
            self.auto_approve_ask_override = next;
            chat_state.auto_approve_ask = next.unwrap_or(self.auto_approve_ask_default);
            self.agent
                .set_approval_policy(if chat_state.auto_approve_ask {
                    crate::runtime::approval::CliApprovalPolicy::Auto
                } else if next.is_some() {
                    crate::runtime::approval::CliApprovalPolicy::DisableAuto
                } else {
                    crate::runtime::approval::CliApprovalPolicy::Ask
                });
            chat_view.set_status(Some(if next.is_none() {
                format!(
                    "Auto mode reset to user default ({}) for this session",
                    if self.auto_approve_ask_default {
                        "on"
                    } else {
                        "off"
                    }
                )
            } else if chat_state.auto_approve_ask {
                "Auto mode enabled for this session".to_string()
            } else {
                "Auto mode disabled for this session".to_string()
            }));
            return Ok(None);
        }
        if let Some(external) = self.external_command_projection_for_action(action_id) {
            return self.select_and_handle_external_command(
                &external, "", chat_view, chat_state, rt_handle,
            );
        }
        let Some(action) = action_by_id(action_id, ActionContext::Chat) else {
            chat_view.set_status(Some(format!("Unknown action: {action_id}")));
            return Ok(None);
        };
        if !action_opens_extension_management(action) {
            if let Some(collision) = self.native_command_collision_for_action(action.id) {
                self.remember_native_command_choice(
                    &collision,
                    &collision.native_candidate_id,
                    chat_view,
                    rt_handle,
                );
            } else if let Some(reconfirmation) = builtin_command_reconfirmation(
                action.id,
                action.name,
                &self.external_conflict_preferences(),
            )
            .filter(|reconfirmation| !reconfirmation.confirmed)
            {
                self.remember_command_choice(
                    &reconfirmation.conflict_key,
                    &reconfirmation.candidate_id,
                    vec![reconfirmation.candidate_id.clone()],
                    chat_view,
                    rt_handle,
                );
            }
        }
        self.dispatch_action(
            action,
            ActionState::chat(chat_state.is_processing, false),
            chat_view,
            chat_state,
            rt_handle,
        )
    }

    /// Handle shortcut commands
    fn handle_command(
        &mut self,
        command: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let token = parts[0];
        let (qualifier, command_name) = parse_command_token(token);
        let arguments = command
            .get(token.len()..)
            .map(str::trim_start)
            .unwrap_or("");
        if command_name == "auto" {
            let action_id = match arguments.trim() {
                "on" | "enable" => "toggle_auto_approve:on",
                "off" | "disable" => "toggle_auto_approve:off",
                "default" | "reset" => "toggle_auto_approve:default",
                "" | "toggle" => "toggle_auto_approve",
                other => {
                    chat_view.set_status(Some(format!(
                        "Usage: /auto [on|off|default|toggle] (current: {})",
                        if chat_state.auto_approve_ask {
                            "on"
                        } else {
                            "off"
                        }
                    )));
                    chat_state.add_system_message(format!(
                        "Unknown Auto mode value '{other}'. Use on, off, default, or toggle."
                    ));
                    return Ok(None);
                }
            };
            return self.handle_action_id(action_id, chat_view, chat_state, rt_handle);
        }
        if let Some(candidate) = self.external_conflict_projection_for_alias(token) {
            return self.select_and_handle_external_command(
                &candidate, arguments, chat_view, chat_state, rt_handle,
            );
        }
        let builtin_alias = format!("/{command_name}");
        let builtin_action = action_for_alias(&builtin_alias, ActionContext::Chat);
        let nonmutating_management_subcommand = builtin_action.is_some_and(|action| {
            action_opens_extension_management(action) && !arguments.trim().is_empty()
        });
        let mut external = self.external_command_projection(command_name);
        let authoritative_preferences = tokio::task::block_in_place(|| {
            rt_handle
                .block_on(external_source_conflict_choices())
                .map(Into::into)
        });
        if let Ok(authoritative_preferences) = authoritative_preferences {
            if authoritative_preferences != self.external_conflict_preferences() {
                self.replace_external_conflict_preferences(authoritative_preferences);
                external = self.external_command_projection(command_name);
                if let Some(snapshot) = &self.external_source_snapshot {
                    self.update_external_source_view(chat_view, snapshot);
                }
            }
        }
        let builtin_reconfirmation = builtin_action.and_then(|action| {
            builtin_command_reconfirmation(
                action.id,
                action.name,
                &self.external_conflict_preferences(),
            )
        });
        if let Some(collision) = external
            .as_ref()
            .and_then(|command| command.native_collision.as_ref())
            .cloned()
        {
            if qualifier == CommandQualifier::External {
                self.remember_native_command_choice(
                    &collision,
                    &collision.external_candidate_id,
                    chat_view,
                    rt_handle,
                );
            } else if qualifier == CommandQualifier::Builtin && !nonmutating_management_subcommand {
                self.remember_native_command_choice(
                    &collision,
                    &collision.native_candidate_id,
                    chat_view,
                    rt_handle,
                );
            }
        } else if qualifier == CommandQualifier::Builtin && !nonmutating_management_subcommand {
            if let Some(reconfirmation) = builtin_reconfirmation
                .as_ref()
                .filter(|reconfirmation| !reconfirmation.confirmed)
            {
                self.remember_command_choice(
                    &reconfirmation.conflict_key,
                    &reconfirmation.candidate_id,
                    vec![reconfirmation.candidate_id.clone()],
                    chat_view,
                    rt_handle,
                );
            } else if let Some(collision) = builtin_action
                .and_then(|action| self.native_command_collision_for_action(action.id))
            {
                let native_candidate_id = collision.native_candidate_id.clone();
                self.remember_native_command_choice(
                    &collision,
                    &native_candidate_id,
                    chat_view,
                    rt_handle,
                );
            }
        }
        let builtin_reconfirmation_required = external.is_none()
            && builtin_reconfirmation
                .as_ref()
                .is_some_and(|reconfirmation| !reconfirmation.confirmed);
        let unresolved_candidates = self.external_conflict_projections(command_name);
        let can_route_external_tool_review = builtin_action
            .is_some_and(|action| action.handler == ActionHandler::Tools)
            && qualifier != CommandQualifier::External
            && (qualifier == CommandQualifier::Builtin
                || (external.is_none()
                    && unresolved_candidates.is_empty()
                    && !builtin_reconfirmation_required));
        if can_route_external_tool_review {
            self.handle_external_tool_review(arguments, chat_view, chat_state, rt_handle);
            return Ok(None);
        }
        let can_route_external_agent_review = builtin_action
            .is_some_and(|action| action.handler == ActionHandler::OpenAgentSelector)
            && !arguments.trim().is_empty()
            && qualifier != CommandQualifier::External
            && (qualifier == CommandQualifier::Builtin
                || (external.is_none()
                    && unresolved_candidates.is_empty()
                    && !builtin_reconfirmation_required));
        if can_route_external_agent_review {
            self.handle_external_agent_review(arguments, chat_view, chat_state, rt_handle);
            return Ok(None);
        }
        let can_route_external_control = builtin_action
            .is_some_and(|action| action.handler == ActionHandler::Extensions)
            && qualifier != CommandQualifier::External
            && (qualifier == CommandQualifier::Builtin
                || (external.is_none()
                    && unresolved_candidates.is_empty()
                    && !builtin_reconfirmation_required));
        if can_route_external_control {
            self.handle_external_control(arguments, chat_view, chat_state, rt_handle);
            return Ok(None);
        }
        let native_choice_is_active = unresolved_candidates.iter().any(|candidate| {
            candidate
                .native_collision
                .as_ref()
                .is_some_and(|collision| {
                    collision.selected_candidate_id.as_deref()
                        == Some(collision.native_candidate_id.as_str())
                })
        });
        if external.is_none()
            && qualifier != CommandQualifier::Builtin
            && !unresolved_candidates.is_empty()
            && !native_choice_is_active
        {
            let mut choices = unresolved_candidates
                .iter()
                .map(|candidate| {
                    if candidate.restricted {
                        format!("{} (restricted)", candidate.invocation_alias)
                    } else {
                        candidate.invocation_alias.clone()
                    }
                })
                .collect::<Vec<_>>();
            if builtin_action.is_some() {
                choices.insert(0, format!("/builtin:{command_name}"));
            }
            chat_state.add_system_message(format!(
                "Command /{command_name} is provided by multiple sources. Choose one once: {}. The choice is remembered until a participant changes.",
                choices.join(", ")
            ));
            return Ok(None);
        }
        let discovery_pending = self
            .external_source_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.discovery_pending);
        match command_route(
            qualifier,
            builtin_action.is_some(),
            external.as_ref(),
            discovery_pending,
            builtin_reconfirmation_required,
        ) {
            CommandRoute::Builtin => {
                let action = builtin_action.expect("route requires an available built-in action");
                self.dispatch_action(
                    action,
                    ActionState::chat(chat_state.is_processing, false),
                    chat_view,
                    chat_state,
                    rt_handle,
                )
            }
            CommandRoute::External => match self.handle_external_command(
                command_name,
                arguments,
                external.as_ref(),
                chat_view,
                chat_state,
                rt_handle,
            ) {
                Ok(result) => Ok(result),
                Err(error) if error.to_string().contains("command not found") => {
                    let message = removed_management_command_hint(parts[0], ActionContext::Chat)
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            format!(
                                "Unknown command: {}\nUse /help or type / to see available commands",
                                parts[0]
                            )
                        });
                    chat_state.add_system_message(message);
                    Ok(None)
                }
                Err(error) => Err(error),
            },
            CommandRoute::AskForCollisionChoice => {
                if builtin_reconfirmation_required {
                    chat_state.add_system_message(format!(
                        "The previous external candidate for /{command_name} changed or was removed. Use /builtin:{command_name} once to confirm the remaining BitFun command."
                    ));
                } else {
                    chat_state.add_system_message(format!(
                        "Command /{command_name} is provided by BitFun and an external source. Choose /builtin:{command_name} or /external:{command_name}; the choice is remembered until the external command changes."
                    ));
                }
                Ok(None)
            }
            CommandRoute::WaitForDiscovery => {
                let explicit = if builtin_action.is_some() {
                    format!(" Use /builtin:{command_name} to run the BitFun command now.")
                } else {
                    String::new()
                };
                chat_state.add_system_message(format!(
                    "BitFun is still checking compatible external commands.{explicit}"
                ));
                Ok(None)
            }
            CommandRoute::UnknownBuiltin => {
                chat_state.add_system_message(format!(
                    "Unknown built-in command: /builtin:{command_name}\nUse /help or type / to see available commands"
                ));
                Ok(None)
            }
        }
    }

    fn external_command_projection(&self, command_name: &str) -> Option<ExternalCommandProjection> {
        external_command_projections(
            self.external_source_snapshot.as_ref()?,
            &self.external_source_conflict_choices,
        )
        .into_iter()
        .find(|command| {
            command.provider_conflict_key.is_none()
                && command.command_name.eq_ignore_ascii_case(command_name)
        })
    }

    fn external_command_projection_for_action(
        &self,
        action_id: &str,
    ) -> Option<ExternalCommandProjection> {
        external_command_projections(
            self.external_source_snapshot.as_ref()?,
            &self.external_source_conflict_choices,
        )
        .into_iter()
        .find(|command| command.action_id == action_id)
    }

    fn external_conflict_projection_for_alias(
        &self,
        token: &str,
    ) -> Option<ExternalCommandProjection> {
        external_command_projections(
            self.external_source_snapshot.as_ref()?,
            &self.external_source_conflict_choices,
        )
        .into_iter()
        .find(|command| {
            command.provider_conflict_key.is_some()
                && command.invocation_alias.eq_ignore_ascii_case(token)
        })
    }

    fn external_conflict_projections(&self, command_name: &str) -> Vec<ExternalCommandProjection> {
        self.external_source_snapshot
            .as_ref()
            .map(|snapshot| {
                external_command_projections(snapshot, &self.external_source_conflict_choices)
                    .into_iter()
                    .filter(|command| {
                        command.provider_conflict_key.is_some()
                            && command.command_name.eq_ignore_ascii_case(command_name)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn native_command_collision_for_action(
        &self,
        action_id: &str,
    ) -> Option<NativeCommandCollisionProjection> {
        external_command_projections(
            self.external_source_snapshot.as_ref()?,
            &self.external_source_conflict_choices,
        )
        .into_iter()
        .filter_map(|command| command.native_collision)
        .find(|collision| collision.native_action_id == action_id)
    }

    fn remember_native_command_choice(
        &mut self,
        collision: &NativeCommandCollisionProjection,
        candidate_id: &str,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.remember_command_choice(
            &collision.conflict_key,
            candidate_id,
            vec![
                collision.native_candidate_id.clone(),
                collision.external_candidate_id.clone(),
            ],
            chat_view,
            rt_handle,
        );
    }

    fn remember_command_choice(
        &mut self,
        conflict_key: &str,
        candidate_id: &str,
        participants: Vec<String>,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let expected_preference_revision = self
            .external_source_snapshot
            .as_ref()
            .map(|snapshot| snapshot.preference_revision)
            .unwrap_or(0);
        let persisted = tokio::task::block_in_place(|| {
            rt_handle.block_on(remember_external_source_conflict_choice(
                conflict_key,
                candidate_id,
                participants.clone(),
                expected_preference_revision,
            ))
        });
        match persisted {
            Ok((choices, lineage, candidates, preference_revision)) => {
                self.replace_external_conflict_preferences((choices, lineage, candidates).into());
                if let Some(snapshot) = &mut self.external_source_snapshot {
                    snapshot.preference_revision = preference_revision;
                }
            }
            Err(error) => {
                tracing::warn!(
                    "Failed to persist external command conflict choice: {}",
                    error
                );
                chat_view.set_status(Some(
                    "The command choice could not be saved; this explicit command will run once"
                        .to_string(),
                ));
            }
        }
        if let Some(snapshot) = &self.external_source_snapshot {
            self.update_external_source_view(chat_view, snapshot);
        }
    }

    fn select_and_handle_external_command(
        &mut self,
        projection: &ExternalCommandProjection,
        arguments: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if projection.restricted {
            chat_state.add_system_message(format!(
                "External command {} is currently restricted and cannot be selected.",
                projection.invocation_alias
            ));
            return Ok(None);
        }
        if let Some(provider_conflict_key) = &projection.provider_conflict_key {
            let workspace = self.agent.workspace_path_buf();
            let expected_preference_revision = self
                .external_source_snapshot
                .as_ref()
                .map(|snapshot| snapshot.preference_revision)
                .unwrap_or(0);
            let snapshot = tokio::task::block_in_place(|| {
                rt_handle.block_on(set_external_prompt_command_conflict_choice(
                    Some(&workspace),
                    provider_conflict_key,
                    &projection.candidate_id,
                    expected_preference_revision,
                ))
            });
            let snapshot = match snapshot {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    chat_state.add_system_message(format!(
                        "Could not select {}: {error}",
                        projection.invocation_alias
                    ));
                    return Ok(None);
                }
            };
            self.external_source_snapshot = Some(snapshot);
            if let Some(collision) = &projection.native_collision {
                self.remember_native_command_choice(
                    collision,
                    &projection.candidate_id,
                    chat_view,
                    rt_handle,
                );
            }
            let Some(active) = self.external_command_projection(&projection.command_name) else {
                chat_state.add_system_message(format!(
                    "Selected external command /{} is no longer available; refresh and choose again.",
                    projection.command_name
                ));
                return Ok(None);
            };
            if let Some(collision) = &active.native_collision {
                self.remember_native_command_choice(
                    collision,
                    &active.candidate_id,
                    chat_view,
                    rt_handle,
                );
            }
            if let Some(snapshot) = &self.external_source_snapshot {
                self.update_external_source_view(chat_view, snapshot);
            }
            return self.handle_external_command(
                &projection.command_name,
                arguments,
                Some(&active),
                chat_view,
                chat_state,
                rt_handle,
            );
        }
        if let Some(collision) = &projection.native_collision {
            self.remember_native_command_choice(
                collision,
                &projection.candidate_id,
                chat_view,
                rt_handle,
            );
        }
        self.handle_external_command(
            &projection.command_name,
            arguments,
            Some(projection),
            chat_view,
            chat_state,
            rt_handle,
        )
    }

    fn handle_external_command(
        &mut self,
        command_name: &str,
        arguments: &str,
        expected: Option<&ExternalCommandProjection>,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if chat_state.is_processing {
            chat_view.set_status(Some(
                "External prompt commands are unavailable while a turn is processing".to_string(),
            ));
            return Ok(None);
        }
        let workspace = self.agent.workspace_path_buf();
        let expanded = tokio::task::block_in_place(|| {
            rt_handle.block_on(expand_external_prompt_command(
                Some(&workspace),
                command_name,
                arguments,
                expected.map(|command| command.candidate_id.as_str()),
                expected.map(|command| command.content_version.as_str()),
            ))
        });
        match expanded {
            Ok(expanded) => {
                self.send_message_to_agent(expanded.content, chat_view, chat_state, rt_handle);
                Ok(None)
            }
            Err(error) if error.contains("command not found") => Err(anyhow!(error)),
            Err(error) => {
                chat_state.add_system_message(format!(
                    "External command /{command_name} is unavailable: {error}"
                ));
                Ok(None)
            }
        }
    }

    fn dispatch_action(
        &mut self,
        action: &'static ActionSpec,
        state: ActionState,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if !action.available(state) {
            chat_view.set_status(Some(action.unavailable_message(state)));
            return Ok(None);
        }

        match action.handler {
            ActionHandler::Help => {
                chat_view.show_info_popup(self.keymap.help_text(state));
            }
            ActionHandler::ClearConversation => {
                if chat_state.is_processing {
                    self.cancel_active_turn(chat_view, rt_handle);
                }
                chat_state.clear_messages();
                chat_view.clear_screen();
                chat_view.set_status(Some("Conversation cleared".to_string()));
            }
            ActionHandler::OpenAgentSelector => {
                self.show_agent_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SwitchAgent => {
                self.cycle_agent(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SwitchAgentReverse => {
                self.cycle_agent_reverse(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SelectModel => {
                self.show_model_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SelectTheme => {
                let themes = self.list_available_themes();
                chat_view.begin_theme_preview();
                chat_view.show_theme_selector(themes, Some(self.config.ui.theme_id.clone()));
                chat_view.set_status(Some(
                    "Theme selector: ↑↓ preview, Enter apply, Esc cancel".to_string(),
                ));
            }
            ActionHandler::AddModel => chat_view.show_provider_selector(),
            ActionHandler::NewSession => {
                return Ok(Some(ChatExitReason::NewSession));
            }
            ActionHandler::Sessions => {
                self.show_session_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Skills => {
                self.show_skill_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::ReloadSkills => {
                self.reload_skills_from_disk(chat_view, chat_state, rt_handle);
            }
            ActionHandler::McpServers => {
                self.show_mcp_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Tools => {
                self.handle_external_tool_review("", chat_view, chat_state, rt_handle);
            }
            ActionHandler::Extensions => {
                self.handle_external_control("", chat_view, chat_state, rt_handle);
            }
            ActionHandler::AcpHelp => {
                chat_state.add_system_message(crate::acp_cli::acp_help_text("bitfun"));
                chat_view.set_status(Some(
                    "ACP setup added to the conversation. You can keep typing.".to_string(),
                ));
            }
            ActionHandler::Init => match crate::prompts::get_cli_prompt("init") {
                Some(prompt) => {
                    self.send_message_to_agent(prompt.to_string(), chat_view, chat_state, rt_handle)
                }
                None => chat_state.add_system_message(
                    "Init prompt not found. Please create prompts/init.md in the CLI crate."
                        .to_string(),
                ),
            },
            ActionHandler::History => {
                chat_state.add_system_message(format!(
                    "Current session statistics:\n\
                     • Messages: {}\n\
                     • Tool calls: {}\n\
                     • Tokens: {}",
                    chat_state.metadata.message_count,
                    chat_state.metadata.tool_calls,
                    chat_state.metadata.total_tokens
                ));
            }
            ActionHandler::Usage => self.show_usage_report(chat_view, chat_state, rt_handle),
            ActionHandler::ToggleAutoApprove => {}
            ActionHandler::Exit => {
                if chat_state.is_processing {
                    self.cancel_active_turn(chat_view, rt_handle);
                }
                return Ok(Some(ChatExitReason::Quit));
            }
            ActionHandler::Login => {
                self.close_all_popups(chat_view);
                self.open_login_or_account_panel(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Logout => self.logout(chat_state, rt_handle),
            ActionHandler::OpenPalette => chat_view.show_command_palette(state),
            ActionHandler::SubmitInput => {
                return self.submit_input(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Interrupt => self.cancel_active_turn(chat_view, rt_handle),
            ActionHandler::ClosePopups => self.close_all_popups(chat_view),
            ActionHandler::NavigateBack => self.navigate_back(chat_view),
            ActionHandler::InsertNewline => chat_view.handle_newline(),
            ActionHandler::Paste => self.paste_clipboard(chat_view),
            ActionHandler::ToggleFocusedTool => {
                chat_view.toggle_focused_tool_expand(chat_state);
            }
            ActionHandler::PreviousTool => {
                chat_view.cycle_block_tool_focus_prev(chat_state);
            }
            ActionHandler::NextTool => {
                chat_view.cycle_block_tool_focus_next(chat_state);
            }
            ActionHandler::HistoryPrevious => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_up();
                } else {
                    chat_view.history_prev();
                }
            }
            ActionHandler::HistoryNext => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_down();
                } else {
                    chat_view.history_next();
                }
            }
            ActionHandler::JumpTop => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_to_top(total);
                chat_view.set_status(Some("Jumped to conversation top".to_string()));
            }
            ActionHandler::JumpBottom => {
                chat_view.scroll_to_bottom();
                chat_view.set_status(Some("Jumped to conversation bottom".to_string()));
            }
            ActionHandler::ClearInput => chat_view.clear_input(),
            ActionHandler::ToggleBrowse => {
                chat_view.toggle_browse_mode();
                let status = if chat_view.browse_mode {
                    "Entered browse mode, use PageUp/PageDown or mouse wheel to scroll conversation"
                } else {
                    "Exited browse mode"
                };
                chat_view.set_status(Some(status.to_string()));
            }
            ActionHandler::ScrollUp => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_up(10, total);
            }
            ActionHandler::ScrollDown => chat_view.scroll_down(10),
        }
        Ok(None)
    }

    fn submit_input(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if let Some(action_id) = chat_view.apply_command_menu_selection() {
            return self.handle_action_id(&action_id, chat_view, chat_state, rt_handle);
        }

        let trimmed = chat_view.input_text().trim();
        let pending_for_current_session = self
            .pending_mode_change
            .as_ref()
            .is_some_and(|pending| pending.session_id == chat_state.core_session_id);
        if mode_change_blocks_typed_submission(pending_for_current_session, trimmed) {
            chat_view.set_status(Some(
                "Waiting for the agent mode change to finish before sending.".to_string(),
            ));
            return Ok(None);
        }

        if chat_state.is_processing {
            if trimmed.starts_with('/') {
                if let Some(input) = chat_view.send_input() {
                    return self.handle_command(&input, chat_view, chat_state, rt_handle);
                }
            } else if !trimmed.is_empty() {
                chat_view.set_status(Some(
                    "Currently processing. Type a /command, or use the interrupt shortcut."
                        .to_string(),
                ));
            }
            return Ok(None);
        }

        if let Some(input) = chat_view.send_input() {
            tracing::info!("User input: {}", input);
            if input.starts_with('/') {
                return self.handle_command(&input, chat_view, chat_state, rt_handle);
            }
            self.send_message_to_agent(input, chat_view, chat_state, rt_handle);
        }
        Ok(None)
    }

    fn cancel_active_turn(&self, chat_view: &mut ChatView, rt_handle: &tokio::runtime::Handle) {
        tracing::info!("User requested cancellation");
        let agent = self.agent.clone();
        tokio::task::block_in_place(|| {
            rt_handle.block_on(async move {
                if let Err(error) = agent.cancel_current_turn().await {
                    tracing::error!("Failed to cancel turn: {}", error);
                }
            })
        });
        chat_view.set_status(Some("Cancelling...".to_string()));
    }

    fn paste_clipboard(&self, chat_view: &mut ChatView) {
        if let Ok(text) = Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            chat_view.insert_paste(&text);
        }
    }
}

fn action_opens_extension_management(action: &ActionSpec) -> bool {
    matches!(
        action.handler,
        ActionHandler::Tools | ActionHandler::Extensions | ActionHandler::OpenAgentSelector
    )
}
