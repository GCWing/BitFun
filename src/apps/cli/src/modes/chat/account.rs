impl ChatMode {
    fn replace_external_conflict_preferences(
        &mut self,
        preferences: ExternalSourceConflictPreferences,
    ) {
        self.external_source_conflict_choices = preferences.choices;
        self.external_source_conflict_lineage_current_keys = preferences.lineage_current_keys;
        self.external_source_conflicted_candidate_ids = preferences.conflicted_candidate_ids;
    }

    fn workspace_path_for_sync(&self, chat_state: &ChatState) -> std::path::PathBuf {
        chat_state
            .workspace
            .as_ref()
            .map(std::path::PathBuf::from)
            .or_else(|| self.workspace.clone().map(std::path::PathBuf::from))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    fn open_login_or_account_panel(
        &self,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let logged_in =
            tokio::task::block_in_place(|| rt_handle.block_on(crate::account::is_logged_in()));
        if logged_in {
            self.open_account_panel(chat_view, rt_handle);
        } else {
            chat_view.show_login_form();
        }
        let _ = chat_state;
    }

    fn open_account_panel(&self, chat_view: &mut ChatView, rt_handle: &tokio::runtime::Handle) {
        let (info, devices, progress) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let info = crate::account::account_info().await;
                let devices = crate::account::list_devices().await.unwrap_or_default();
                let progress = crate::account_sync::current_sync_progress().await;
                (info, devices, progress)
            })
        });
        match info {
            Ok(info) => chat_view.show_account_panel(info, devices, progress),
            Err(e) => {
                chat_view.set_status(Some(format!("Failed to load account: {e}")));
                chat_view.show_login_form();
            }
        }
    }

    fn refresh_account_panel_live(&self, chat_view: &mut ChatView) {
        if !chat_view.login_form_visible() {
            return;
        }
        let progress = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::account_sync::current_sync_progress())
        });
        let devices = if matches!(
            progress.status,
            crate::account_sync::SyncStatus::Syncing | crate::account_sync::SyncStatus::Done
        ) {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(crate::account::list_devices())
                    .ok()
            })
        } else {
            None
        };
        chat_view.update_account_panel_progress(devices, progress);
    }

    fn start_sync_and_show_account(
        &self,
        is_first_login: bool,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.workspace_path_for_sync(chat_state);
        crate::account_sync::start_auto_sync_background(
            self.runtime.compatibility().clone(),
            is_first_login,
            workspace,
        );
        self.open_account_panel(chat_view, rt_handle);
        chat_state.add_system_message(if is_first_login {
            "Sync started (use local / upload settings).".to_string()
        } else {
            "Sync started (use cloud / download settings).".to_string()
        });
    }

    fn handle_login_form_action(
        &self,
        action: LoginFormAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        match action {
            LoginFormAction::Submit(creds) => {
                let result = tokio::task::block_in_place(|| {
                    rt_handle.block_on(crate::account::login_with_credentials(
                        &creds.relay_url,
                        &creds.username,
                        &creds.password,
                    ))
                });
                match result {
                    Ok(login) => {
                        chat_state.add_system_message(login.status_message.clone());
                        if login.has_cloud_settings {
                            chat_view.show_sync_choice_panel(&login.user_id, &login.relay_url);
                        } else {
                            self.start_sync_and_show_account(
                                true, chat_view, chat_state, rt_handle,
                            );
                        }
                    }
                    Err(e) => {
                        chat_view.login_form_set_error(format!("Login failed: {e}"));
                    }
                }
            }
            LoginFormAction::SyncUseLocal => {
                if let Err(e) = tokio::task::block_in_place(|| {
                    rt_handle.block_on(crate::account::finalize_login_after_sync_choice())
                }) {
                    chat_view.login_form_set_error(format!("Finalize login failed: {e}"));
                    let _ = tokio::task::block_in_place(|| {
                        rt_handle.block_on(crate::account::logout())
                    });
                    chat_view.show_login_form();
                    return Ok(None);
                }
                self.start_sync_and_show_account(true, chat_view, chat_state, rt_handle);
            }
            LoginFormAction::SyncUseCloud => {
                if let Err(e) = tokio::task::block_in_place(|| {
                    rt_handle.block_on(crate::account::finalize_login_after_sync_choice())
                }) {
                    chat_view.login_form_set_error(format!("Finalize login failed: {e}"));
                    let _ = tokio::task::block_in_place(|| {
                        rt_handle.block_on(crate::account::logout())
                    });
                    chat_view.show_login_form();
                    return Ok(None);
                }
                self.start_sync_and_show_account(false, chat_view, chat_state, rt_handle);
            }
            LoginFormAction::SyncCancel => {
                let _ =
                    tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout()));
                chat_view.show_login_form();
                chat_state.add_system_message("Sync cancelled; logged out.".to_string());
            }
            LoginFormAction::Logout => {
                match tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout())) {
                    Ok(()) => {
                        chat_view.show_login_form();
                        chat_state.add_system_message("Logged out.".to_string());
                    }
                    Err(e) => {
                        chat_view.login_form_set_error(format!("Logout failed: {e}"));
                    }
                }
            }
            LoginFormAction::Cancel => {
                chat_view.set_status(Some("Account panel closed".to_string()));
            }
            LoginFormAction::None => {}
        }
        Ok(None)
    }

    /// Check if any popup is currently visible
    fn any_popup_visible(&self, chat_view: &ChatView) -> bool {
        chat_view.command_palette_visible()
            || chat_view.model_selector_visible()
            || chat_view.agent_selector_visible()
            || chat_view.session_selector_visible()
            || chat_view.skill_selector_visible()
            || chat_view.subagent_selector_visible()
            || chat_view.mcp_selector_visible()
            || chat_view.mcp_add_dialog_visible()
            || chat_view.provider_selector_visible()
            || chat_view.model_config_form_visible()
            || chat_view.login_form_visible()
            || chat_view.theme_selector_visible()
            || chat_view.info_popup_visible()
    }

    /// Close all popups and clear the navigation stack
    fn close_all_popups(&self, chat_view: &mut ChatView) {
        // Cancel theme preview if active
        if chat_view.theme_selector_visible() {
            chat_view.cancel_theme_preview();
        }
        chat_view.hide_command_palette();
        chat_view.hide_model_selector();
        chat_view.hide_agent_selector();
        chat_view.hide_session_selector();
        chat_view.hide_skill_selector();
        chat_view.hide_subagent_selector();
        chat_view.hide_mcp_selector();
        chat_view.hide_mcp_add_dialog();
        chat_view.hide_provider_selector();
        chat_view.hide_model_config_form();
        chat_view.hide_login_form();
        chat_view.hide_theme_selector();
        chat_view.dismiss_info_popup();
        chat_view.popup_stack.clear();
    }

    /// Navigate back to the previous popup in the stack, or close all if at the root
    fn navigate_back(&self, chat_view: &mut ChatView) {
        // Pop the current popup from the stack and hide it
        if let Some(current) = chat_view.popup_stack.pop() {
            // Hide the current popup
            match current {
                crate::ui::chat::PopupType::CommandPalette => chat_view.hide_command_palette(),
                crate::ui::chat::PopupType::ModelSelector => chat_view.hide_model_selector(),
                crate::ui::chat::PopupType::AgentSelector => chat_view.hide_agent_selector(),
                crate::ui::chat::PopupType::SessionSelector => chat_view.hide_session_selector(),
                crate::ui::chat::PopupType::SkillSelector => chat_view.hide_skill_selector(),
                crate::ui::chat::PopupType::SubagentSelector => chat_view.hide_subagent_selector(),
                crate::ui::chat::PopupType::McpSelector => chat_view.hide_mcp_selector(),
                crate::ui::chat::PopupType::McpAddDialog => chat_view.hide_mcp_add_dialog(),
                crate::ui::chat::PopupType::ProviderSelector => chat_view.hide_provider_selector(),
                crate::ui::chat::PopupType::ModelConfigForm => chat_view.hide_model_config_form(),
                crate::ui::chat::PopupType::LoginForm => chat_view.hide_login_form(),
                crate::ui::chat::PopupType::ThemeSelector => {
                    chat_view.hide_theme_selector();
                    chat_view.cancel_theme_preview();
                }
                crate::ui::chat::PopupType::InfoPopup => chat_view.dismiss_info_popup(),
            }

            // If there's a previous popup in the stack, re-show it
            if let Some(previous) = chat_view.popup_stack.peek() {
                match previous {
                    crate::ui::chat::PopupType::CommandPalette => {
                        chat_view.reshow_command_palette()
                    }
                    crate::ui::chat::PopupType::ModelSelector => chat_view.reshow_model_selector(),
                    crate::ui::chat::PopupType::AgentSelector => chat_view.reshow_agent_selector(),
                    crate::ui::chat::PopupType::SessionSelector => {
                        chat_view.reshow_session_selector()
                    }
                    crate::ui::chat::PopupType::SkillSelector => chat_view.reshow_skill_selector(),
                    crate::ui::chat::PopupType::SubagentSelector => {
                        chat_view.reshow_subagent_selector()
                    }
                    crate::ui::chat::PopupType::McpSelector => chat_view.reshow_mcp_selector(),
                    crate::ui::chat::PopupType::McpAddDialog => chat_view.reshow_mcp_add_dialog(),
                    crate::ui::chat::PopupType::ProviderSelector => {
                        chat_view.reshow_provider_selector()
                    }
                    crate::ui::chat::PopupType::ModelConfigForm => {
                        chat_view.reshow_model_config_form()
                    }
                    crate::ui::chat::PopupType::LoginForm => chat_view.reshow_login_form(),
                    crate::ui::chat::PopupType::ThemeSelector => chat_view.reshow_theme_selector(),
                    crate::ui::chat::PopupType::InfoPopup => {}
                }
            }
        }
    }
}
