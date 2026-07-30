impl ChatMode {
    /// Switch to a different session: restore it from core, reload messages, update state
    fn switch_to_session(
        &mut self,
        new_session_id: &str,
        session_id: &mut String,
        chat_state: &mut ChatState,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        let agent = self.agent.clone();
        let sid = new_session_id.to_string();

        let (new_state, restored_agent_type, migration_notices) =
            tokio::task::block_in_place(|| {
                rt_handle.block_on(async {
                    let (session_summary, workspace_binding, migration_notices, transcript) =
                        agent.restore_session_in_current_workspace(&sid).await?;
                    let restored_agent_type = session_summary.agent_type.clone();
                    let effective_workspace = Some(workspace_binding.workspace_path.clone());

                    let mut state = ChatState::from_session_transcript(
                        sid.clone(),
                        session_summary.session_name,
                        restored_agent_type.clone(),
                        effective_workspace,
                        &transcript,
                    );
                    state.current_model_id = session_summary.model_id;
                    state.apply_workspace_binding(workspace_binding);

                    Ok::<_, anyhow::Error>((state, restored_agent_type, migration_notices))
                })
            })?;

        // Update session state
        *session_id = new_session_id.to_string();
        *chat_state = new_state;
        chat_state.set_worktree_control_available(!self.agent.is_shared());
        self.agent_type = restored_agent_type;
        self.workspace = chat_state.workspace.clone();
        self.refresh_workspace_git_status(chat_state, rt_handle);
        self.auto_approve_ask_override = None;
        clear_selected_native_command_prefill(&mut self.selected_native_command_once, chat_view);
        chat_state.auto_approve_ask = self.auto_approve_ask_default;
        self.agent
            .set_approval_policy(crate::runtime::approval::CliApprovalPolicy::Ask);

        // Reload model name
        self.load_current_model_name(chat_state, rt_handle);

        for notice in migration_notices {
            chat_state.add_system_message(notice.user_message());
        }

        // Reset view state
        chat_view.scroll_to_bottom();
        chat_view.set_status(Some(format!("Switched to session: {}", new_session_id)));

        Ok(())
    }

    /// Create a new session: reset state and start fresh
    fn create_new_session(
        &mut self,
        session_id: &mut String,
        chat_state: &mut ChatState,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        let agent = self.agent.clone();
        let agent_type = self.agent_type.clone();

        let (new_session_id, workspace_binding) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let session_id = agent.create_new_session(&agent_type).await?;
                let binding = agent.session_workspace_binding(&session_id).await?;
                Ok::<_, anyhow::Error>((session_id, binding))
            })
        })?;

        let mut new_state = ChatState::new(
            new_session_id.clone(),
            "CLI Session".to_string(),
            agent_type,
            Some(workspace_binding.workspace_path.clone()),
        );
        new_state.apply_workspace_binding(workspace_binding);

        *session_id = new_session_id;
        *chat_state = new_state;
        chat_state.set_worktree_control_available(!self.agent.is_shared());
        self.workspace = chat_state.workspace.clone();
        self.refresh_workspace_git_status(chat_state, rt_handle);
        self.auto_approve_ask_override = None;
        clear_selected_native_command_prefill(&mut self.selected_native_command_once, chat_view);
        chat_state.auto_approve_ask = self.auto_approve_ask_default;
        self.agent
            .set_approval_policy(crate::runtime::approval::CliApprovalPolicy::Ask);

        // Reload model name
        self.load_current_model_name(chat_state, rt_handle);

        // Reset view state
        chat_view.clear_screen();
        chat_view.scroll_to_bottom();
        chat_view.set_status(Some("New session created".to_string()));

        Ok(())
    }

    /// Show skill list/configuration menu.
    /// Send a message to the agent programmatically (used by slash commands like /init)
    fn send_message_to_agent(
        &mut self,
        message: String,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if self
            .pending_session_update
            .as_ref()
            .is_some_and(|pending| pending.session_id == chat_state.core_session_id)
        {
            chat_view.set_status(Some(
                "Waiting for the current session update to finish before sending.".to_string(),
            ));
            return;
        }
        if chat_state.is_processing {
            chat_state.add_system_message("Already processing, please wait.".to_string());
            return;
        }

        if let Err(error) = self.materialize_requested_worktree(chat_view, chat_state, rt_handle) {
            tracing::error!("Failed to prepare worktree for submitted prompt: {error}");
            chat_view.set_status(Some(format!("Error: {error}")));
            chat_state.add_system_message(error);
            return;
        }

        let display_name = agent_display_name(&self.agent_type);
        chat_view.set_status(Some(format!("{} is thinking...", display_name)));

        let agent = self.agent.clone();
        let agent_type = self.agent_type.clone();
        match tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.send_message(message, &agent_type))
        }) {
            Ok(turn_id) => {
                tracing::info!("Started turn: {}", turn_id);
            }
            Err(e) => {
                tracing::error!("Failed to send message: {}", e);
                chat_view.set_status(Some(format!("Error: {}", e)));
            }
        }
    }

    /// Show session selector popup with all available sessions
    fn show_session_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent = self.agent.clone();
        let current_session_id = chat_state.core_session_id.clone();
        let project_workspace = Some(agent.project_workspace_path_string());

        let sessions = tokio::task::block_in_place(|| rt_handle.block_on(agent.list_sessions()));
        let sessions = match sessions {
            Ok(sessions) => sessions,
            Err(error) => {
                tracing::error!("Failed to list sessions: {error}");
                chat_view.set_status(Some(format!("Failed to load sessions: {error}")));
                return;
            }
        };

        if sessions.is_empty() {
            chat_state.add_system_message("No sessions found.".to_string());
            return;
        }

        let session_items: Vec<SessionItem> = sessions
            .into_iter()
            .map(|s| {
                let last_activity = {
                    let last_activity =
                        std::time::UNIX_EPOCH + Duration::from_millis(s.last_active_at_ms);
                    let elapsed = last_activity.elapsed().unwrap_or_default();
                    if elapsed.as_secs() < 60 {
                        "just now".to_string()
                    } else if elapsed.as_secs() < 3600 {
                        format!("{}m ago", elapsed.as_secs() / 60)
                    } else if elapsed.as_secs() < 86400 {
                        format!("{}h ago", elapsed.as_secs() / 3600)
                    } else {
                        format!("{}d ago", elapsed.as_secs() / 86400)
                    }
                };
                SessionItem {
                    session_id: s.session_id,
                    session_name: s.session_name,
                    last_activity,
                    workspace: project_workspace.clone(),
                }
            })
            .collect();

        chat_view.show_session_selector(
            session_items,
            Some(current_session_id),
            !self.agent.is_shared(),
        );
    }

    /// Handle session deletion from the session selector
    fn handle_session_delete(
        &self,
        item: &SessionItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if self.agent.is_shared() {
            chat_view.set_status(Some(format!(
                "Session deletion is unavailable in Shared TUI preview. {SHARED_TUI_EMBEDDED_HANDOFF}; then run `bitfun sessions delete`"
            )));
            return;
        }
        // Prevent deleting the currently active session
        if item.session_id == chat_state.core_session_id {
            chat_view.set_status(Some("Cannot delete the active session".to_string()));
            return;
        }

        let agent = self.agent.clone();
        let sid = item.session_id.clone();

        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async { agent.delete_session(&sid).await })
        });

        match result {
            Ok(()) => {
                chat_view.session_selector_remove_item(&item.session_id);
                chat_view.set_status(Some(format!("Session deleted: {}", item.session_name)));
                tracing::info!("Deleted session: {}", item.session_id);
            }
            Err(e) => {
                chat_view.set_status(Some(format!("Failed to delete session: {}", e)));
                tracing::error!("Failed to delete session: {}", e);
            }
        }
    }
}
