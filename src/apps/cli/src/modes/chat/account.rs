impl ChatMode {
    fn replace_external_conflict_preferences(
        &mut self,
        preferences: ExternalSourceConflictPreferences,
    ) {
        self.external_source_conflict_choices = preferences.choices;
        self.external_source_conflict_lineage_current_keys = preferences.lineage_current_keys;
        self.external_source_conflicted_candidate_ids = preferences.conflicted_candidate_ids;
    }

    fn open_login_or_account_panel(
        &self,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let snapshot = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let account = self.account_runtime.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Account management is unavailable for this TUI Host")
                })?;
                Ok::<_, anyhow::Error>(crate::account::account_snapshot_projection(
                    account.snapshot().await,
                ))
            })
        });
        match snapshot {
            Ok(snapshot) if snapshot.logged_in => self.open_account_panel(chat_view, snapshot),
            Ok(_) => chat_view.show_login_form(),
            Err(error) => {
                chat_view.show_login_form();
                chat_view.login_form_set_error(format!("Failed to load account: {error}"));
            }
        }
        let _ = chat_state;
    }

    fn open_account_panel(&self, chat_view: &mut ChatView, snapshot: AccountSnapshotProjection) {
        let Some(info) = snapshot.info else {
            chat_view.show_login_form();
            return;
        };
        chat_view.show_account_panel(info, snapshot.devices, snapshot.sync);
    }

    fn refresh_account_panel_live(&self, chat_view: &mut ChatView) -> bool {
        if !chat_view.login_form_visible() {
            return false;
        }
        let Ok(progress) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let account = self.account_runtime.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Account management is unavailable for this TUI Host")
                })?;
                Ok::<_, anyhow::Error>(crate::account::settings_sync_progress(
                    account.current_sync_progress().await,
                ))
            })
        }) else {
            return false;
        };
        let progress = progress;
        let devices = if matches!(
            progress.status,
            SettingsSyncStatus::Syncing | SettingsSyncStatus::Done
        ) {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let account = self.account_runtime.as_ref()?;
                    Some(
                        crate::account::account_snapshot_projection(account.snapshot().await)
                            .devices,
                    )
                })
            })
        } else {
            None
        };
        let syncing = progress.status == SettingsSyncStatus::Syncing;
        chat_view.update_account_panel_progress(devices, progress);
        syncing
    }

    fn start_sync_and_show_account(
        &self,
        is_first_login: bool,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let account = self.account_runtime.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Account management is unavailable for this TUI Host")
                })?;
                if !account.is_logged_in().await {
                    anyhow::bail!("Account login must be finalized before settings sync starts")
                }
                if !account
                    .start_auto_sync_background(
                        format!("tui-account-{}", uuid::Uuid::new_v4()),
                        is_first_login,
                        std::path::PathBuf::from(self.agent.project_workspace_path_string()),
                    )
                    .await
                {
                    anyhow::bail!("Account settings sync is already in progress")
                }
                Ok::<(), anyhow::Error>(())
            })
        });
        if let Err(error) = result {
            chat_state.add_system_message(format!("Account settings sync failed: {error}"));
            return;
        }
        if let Ok(snapshot) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let account = self.account_runtime.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Account management is unavailable for this TUI Host")
                })?;
                Ok::<_, anyhow::Error>(crate::account::account_snapshot_projection(
                    account.snapshot().await,
                ))
            })
        }) {
            self.open_account_panel(chat_view, snapshot);
        }
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
                    rt_handle.block_on(async {
                        let account = self.account_runtime.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Account management is unavailable for this TUI Host")
                        })?;
                        let relay_url = creds.relay_url;
                        let username = creds.username;
                        let password = creds.password;
                        let result = account
                            .login_with_credentials(&relay_url, &username, &password)
                            .await
                            .map_err(|error| {
                                crate::account::redact_login_error(
                                    error,
                                    [&relay_url, &username, &password],
                                )
                            })?;
                        let status_message = crate::account::account_login_status_message(&result);
                        Ok::<_, anyhow::Error>(
                            openbitfun_product_domains::account::AccountLoginProjection {
                                user_id: result.user_id,
                                relay_url: result.relay_url,
                                has_cloud_settings: result.has_cloud_settings,
                                status_message,
                            },
                        )
                    })
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
                let result = tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let account = self.account_runtime.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Account management is unavailable for this TUI Host")
                        })?;
                        account.finalize_login_after_sync_choice().await?;
                        if !account
                            .start_auto_sync_background(
                                format!("tui-account-{}", uuid::Uuid::new_v4()),
                                true,
                                std::path::PathBuf::from(
                                    self.agent.project_workspace_path_string(),
                                ),
                            )
                            .await
                        {
                            anyhow::bail!("Account settings sync is already in progress")
                        }
                        Ok::<_, anyhow::Error>(crate::account::account_snapshot_projection(
                            account.snapshot().await,
                        ))
                    })
                });
                let snapshot = match result {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        chat_view.login_form_set_error(format!("Finalize login failed: {error}"));
                        let _ = tokio::task::block_in_place(|| {
                            rt_handle.block_on(async {
                                let account = self.account_runtime.as_ref().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Account management is unavailable for this TUI Host"
                                    )
                                })?;
                                account.logout().await?;
                                account
                                    .mark_sync_cancelled(format!(
                                        "tui-account-{}",
                                        uuid::Uuid::new_v4()
                                    ))
                                    .await;
                                Ok::<(), anyhow::Error>(())
                            })
                        });
                        chat_view.show_login_form();
                        return Ok(None);
                    }
                };
                self.open_account_panel(chat_view, snapshot);
                chat_state
                    .add_system_message("Sync started (use local / upload settings).".to_string());
            }
            LoginFormAction::SyncUseCloud => {
                let result = tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let account = self.account_runtime.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Account management is unavailable for this TUI Host")
                        })?;
                        account.finalize_login_after_sync_choice().await?;
                        if !account
                            .start_auto_sync_background(
                                format!("tui-account-{}", uuid::Uuid::new_v4()),
                                false,
                                std::path::PathBuf::from(
                                    self.agent.project_workspace_path_string(),
                                ),
                            )
                            .await
                        {
                            anyhow::bail!("Account settings sync is already in progress")
                        }
                        Ok::<_, anyhow::Error>(crate::account::account_snapshot_projection(
                            account.snapshot().await,
                        ))
                    })
                });
                let snapshot = match result {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        chat_view.login_form_set_error(format!("Finalize login failed: {error}"));
                        let _ = tokio::task::block_in_place(|| {
                            rt_handle.block_on(async {
                                let account = self.account_runtime.as_ref().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Account management is unavailable for this TUI Host"
                                    )
                                })?;
                                account.logout().await?;
                                account
                                    .mark_sync_cancelled(format!(
                                        "tui-account-{}",
                                        uuid::Uuid::new_v4()
                                    ))
                                    .await;
                                Ok::<(), anyhow::Error>(())
                            })
                        });
                        chat_view.show_login_form();
                        return Ok(None);
                    }
                };
                self.open_account_panel(chat_view, snapshot);
                chat_state.add_system_message(
                    "Sync started (use cloud / download settings).".to_string(),
                );
            }
            LoginFormAction::SyncCancel => {
                let _ = tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let account = self.account_runtime.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Account management is unavailable for this TUI Host")
                        })?;
                        let progress = account
                            .cancel_sync(format!("tui-account-{}", uuid::Uuid::new_v4()))
                            .await?;
                        Ok::<_, anyhow::Error>(crate::account::settings_sync_progress(progress))
                    })
                });
                chat_view.show_login_form();
                chat_state.add_system_message("Sync cancelled; logged out.".to_string());
            }
            LoginFormAction::Logout => {
                match tokio::task::block_in_place(|| {
                    rt_handle.block_on(async {
                        let account = self.account_runtime.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Account management is unavailable for this TUI Host")
                        })?;
                        account.logout().await?;
                        account
                            .mark_sync_cancelled(format!("tui-account-{}", uuid::Uuid::new_v4()))
                            .await;
                        Ok::<_, anyhow::Error>(crate::account::account_snapshot_projection(
                            account.snapshot().await,
                        ))
                    })
                }) {
                    Ok(_) => {
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
            || chat_view.session_lineage_selector_visible()
            || chat_view.fork_selector_visible()
            || chat_view.timeline_selector_visible()
            || chat_view.prompt_stash_selector_visible()
            || chat_view.export_dialog_visible()
            || chat_view.skill_selector_visible()
            || chat_view.subagent_selector_visible()
            || chat_view.mcp_selector_visible()
            || chat_view.mcp_add_dialog_visible()
            || chat_view.provider_selector_visible()
            || chat_view.model_config_form_visible()
            || chat_view.login_form_visible()
            || chat_view.theme_selector_visible()
            || chat_view.info_popup_visible()
            || chat_view.prompt_command_shell_review_visible()
            || chat_view.workspace_diff_visible()
    }

    /// Close all popups and clear the navigation stack
    fn close_all_popups(&mut self, chat_view: &mut ChatView) {
        // Cancel theme preview if active
        if chat_view.theme_selector_visible() {
            chat_view.cancel_theme_preview();
        }
        chat_view.hide_command_palette();
        chat_view.hide_model_selector();
        chat_view.hide_agent_selector();
        chat_view.hide_session_selector();
        chat_view.hide_session_lineage_selector();
        chat_view.hide_fork_selector();
        chat_view.hide_timeline_selector();
        chat_view.hide_prompt_stash_selector();
        chat_view.hide_export_dialog();
        chat_view.hide_skill_selector();
        chat_view.hide_subagent_selector();
        chat_view.hide_mcp_selector();
        chat_view.hide_mcp_add_dialog();
        chat_view.hide_provider_selector();
        chat_view.hide_model_config_form();
        chat_view.hide_login_form();
        chat_view.hide_theme_selector();
        chat_view.dismiss_info_popup();
        chat_view.hide_prompt_command_shell_review();
        self.pending_prompt_command_shell_invocation = None;
        chat_view.hide_workspace_diff();
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
                crate::ui::chat::PopupType::SessionLineageSelector => {
                    chat_view.hide_session_lineage_selector()
                }
                crate::ui::chat::PopupType::ForkSelector => chat_view.hide_fork_selector(),
                crate::ui::chat::PopupType::TimelineSelector => chat_view.hide_timeline_selector(),
                crate::ui::chat::PopupType::PromptStashSelector => {
                    chat_view.hide_prompt_stash_selector()
                }
                crate::ui::chat::PopupType::ExportDialog => chat_view.hide_export_dialog(),
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
                crate::ui::chat::PopupType::WorkspaceDiff => chat_view.hide_workspace_diff(),
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
                    crate::ui::chat::PopupType::SessionLineageSelector => {
                        chat_view.reshow_session_lineage_selector()
                    }
                    crate::ui::chat::PopupType::ForkSelector => chat_view.reshow_fork_selector(),
                    crate::ui::chat::PopupType::TimelineSelector => {
                        chat_view.reshow_timeline_selector()
                    }
                    crate::ui::chat::PopupType::PromptStashSelector => {
                        chat_view.reshow_prompt_stash_selector()
                    }
                    crate::ui::chat::PopupType::ExportDialog => {}
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
                    crate::ui::chat::PopupType::WorkspaceDiff => chat_view.reshow_workspace_diff(),
                }
            }
        }
    }
}
