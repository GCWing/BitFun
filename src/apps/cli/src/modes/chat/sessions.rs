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

        let (new_state, restored_agent_type, migration_notice) =
            tokio::task::block_in_place(|| {
                rt_handle.block_on(async {
                    let (session_summary, effective_workspace_path, migration_notice) =
                        agent.restore_session_in_current_workspace(&sid).await?;
                    let restored_agent_type = session_summary.agent_type.clone();
                    let effective_workspace =
                        Some(effective_workspace_path.to_string_lossy().to_string());

                    // Load historical messages through the runtime transcript contract.
                    let transcript = agent.get_transcript(&sid).await.unwrap_or_else(|_| {
                        bitfun_agent_runtime::sdk::SessionTranscript {
                            session_id: sid.clone(),
                            messages: Vec::new(),
                        }
                    });

                    let state = ChatState::from_session_transcript(
                        sid.clone(),
                        session_summary.session_name,
                        restored_agent_type.clone(),
                        effective_workspace,
                        &transcript,
                    );

                    Ok::<_, anyhow::Error>((state, restored_agent_type, migration_notice))
                })
            })?;

        // Update session state
        *session_id = new_session_id.to_string();
        *chat_state = new_state;
        self.agent_type = restored_agent_type;
        self.workspace = chat_state.workspace.clone();
        self.auto_approve_ask_override = None;
        chat_state.auto_approve_ask = self.auto_approve_ask_default;
        self.agent
            .set_approval_policy(crate::runtime::approval::CliApprovalPolicy::Ask);

        // Reload model name
        self.load_current_model_name(chat_state, rt_handle);

        if let Some(notice) = migration_notice {
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
        let workspace = self.workspace.clone();

        let new_session_id = tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.create_new_session(&agent_type))
        })?;

        let new_state = ChatState::new(
            new_session_id.clone(),
            "CLI Session".to_string(),
            agent_type,
            workspace,
        );

        *session_id = new_session_id;
        *chat_state = new_state;
        self.workspace = chat_state.workspace.clone();
        self.auto_approve_ask_override = None;
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
        &self,
        message: String,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if self
            .pending_mode_change
            .as_ref()
            .is_some_and(|pending| pending.session_id == chat_state.core_session_id)
        {
            chat_view.set_status(Some(
                "Waiting for the agent mode change to finish before sending.".to_string(),
            ));
            return;
        }
        if chat_state.is_processing {
            chat_state.add_system_message("Already processing, please wait.".to_string());
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

        let sessions = tokio::task::block_in_place(|| {
            rt_handle.block_on(async { agent.list_sessions().await.unwrap_or_default() })
        });

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
                    workspace: self.workspace.clone(),
                }
            })
            .collect();

        chat_view.show_session_selector(session_items, Some(current_session_id));
    }

    /// Handle session deletion from the session selector
    fn handle_session_delete(
        &self,
        item: &SessionItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
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
