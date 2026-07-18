impl ChatMode {
    fn show_skill_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        _rt_handle: &tokio::runtime::Handle,
    ) {
        chat_view.show_skill_menu();
    }

    /// Re-scan skill directories from disk and rebuild the registry cache.
    ///
    /// Mirrors Claude Code 2.1.152 `/reload-skills`. Safe to call at any
    /// time — does not require `is_processing` to be false because the
    /// registry swap is atomic and a held `SkillInfo` reference is not
    /// kept across the call.
    fn reload_skills_from_disk(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = SkillRegistry::global();
        let workspace = self.agent.workspace_path_buf();
        let outcome = tokio::task::block_in_place(|| {
            // refresh() is the global re-scan entry point; the workspace
            // arg of refresh_for_workspace is currently a no-op upstream,
            // so we call refresh() directly and re-resolve the workspace
            // count afterwards.
            rt_handle.block_on(async {
                registry.refresh().await;
                registry
                    .get_resolved_skills_for_workspace(Some(workspace.as_path()), None)
                    .await
            })
        });

        let count = outcome.len();
        chat_state.add_system_message(format!("Reloaded {} skill(s) from disk.", count));
        chat_view.set_status(Some(format!("Skills reloaded ({} available)", count)));
    }

    fn show_available_skill_list(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_resolved_skills_for_workspace(Some(workspace.as_path()), Some(&agent_type))
                    .await
            })
        });

        if skills.is_empty() {
            chat_state.add_system_message(format!(
                "No enabled skills found for agent mode '{}'. Add skills in .bitfun/skills/, .cursor/skills/, or ~/.cursor/skills/, or enable built-in skills for this mode.",
                self.agent_type
            ));
            return;
        }

        let skill_items: Vec<SkillItem> =
            skills.into_iter().map(Self::skill_item_from_info).collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_list(skill_items);
    }

    fn show_skill_config_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_mode_skill_infos_for_workspace(Some(workspace.as_path()), &agent_type)
                    .await
            })
        });

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(Self::skill_item_from_mode_info)
            .collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_config(skill_items);
    }

    fn handle_skill_selector_action(
        &self,
        action: SkillSelectorAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match action {
            SkillSelectorAction::ListSkills => {
                self.show_available_skill_list(chat_view, chat_state, rt_handle);
            }
            SkillSelectorAction::ConfigureSkills => {
                self.show_skill_config_selector(chat_view, chat_state, rt_handle);
            }
            SkillSelectorAction::Execute(selected) => {
                chat_view.hide_skill_selector();
                self.apply_skill_selection(&selected, chat_view);
            }
            SkillSelectorAction::Toggle(selected) => {
                self.set_skill_enabled(&selected, !selected.enabled, chat_state, rt_handle);
                self.show_skill_config_selector(chat_view, chat_state, rt_handle);
            }
        }
    }

    /// Apply skill selection: fill input box with execution command
    fn apply_skill_selection(&self, selected: &SkillItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!("Execute the {} skill.", selected.name));
    }

    fn set_skill_enabled(
        &self,
        selected: &SkillItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.agent.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let skill = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                match skill.level.as_str() {
                    "user" => {
                        set_user_mode_skill_state(
                            &mode_id,
                            &skill.key,
                            enabled,
                            skill.default_enabled,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    }
                    "project" => {
                        let mut document = load_project_mode_skills_document_local(&workspace)
                            .await
                            .map_err(|error| error.to_string())?;
                        set_mode_skill_disabled_in_document(
                            &mut document,
                            &mode_id,
                            &skill.key,
                            !enabled,
                        )
                        .map_err(|error| error.to_string())?;
                        save_project_mode_skills_document_local(&workspace, &document)
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    other => {
                        return Err(format!("Unsupported skill level '{}'", other));
                    }
                }

                Ok(())
            })
        });

        match result {
            Ok(()) => chat_state.add_system_message(format!(
                "Skill '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            )),
            Err(error) => chat_state.add_system_message(format!(
                "Failed to update skill '{}': {}",
                selected.name, error
            )),
        }
    }

    fn skill_item_from_info(info: SkillInfo) -> SkillItem {
        SkillItem {
            key: info.key,
            name: info.name,
            description: info.description,
            level: info.level.as_str().to_string(),
            source_slot: info.source_slot,
            source_label: info.source_label,
            enabled: true,
            selected_for_runtime: true,
            default_enabled: true,
            is_shadowed: info.is_shadowed,
            shadowed_by_key: info.shadowed_by_key,
        }
    }

    fn skill_item_from_mode_info(info: ModeSkillInfo) -> SkillItem {
        SkillItem {
            key: info.skill.key,
            name: info.skill.name,
            description: info.skill.description,
            level: info.skill.level.as_str().to_string(),
            source_slot: info.skill.source_slot,
            source_label: info.skill.source_label,
            enabled: info.effective_enabled,
            selected_for_runtime: info.selected_for_runtime,
            default_enabled: info.default_enabled,
            is_shadowed: info.skill.is_shadowed,
            shadowed_by_key: info.skill.shadowed_by_key,
        }
    }

    /// Show subagent list/configuration menu.
    fn show_subagent_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        _rt_handle: &tokio::runtime::Handle,
    ) {
        chat_view.show_subagent_menu();
    }

    fn show_available_subagent_list(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(registry.get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: Some(&agent_type),
                workspace_root: Some(workspace.as_path()),
                list_scope: SubagentListScope::TaskVisible,
                include_disabled: false,
                external_sources_supported: true,
            }))
        });

        if subagents.is_empty() {
            chat_state.add_system_message(format!(
                "No enabled subagents found for agent mode '{}'.",
                self.agent_type
            ));
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            chat_state.add_system_message("No subagents found.".to_string());
            return;
        }

        chat_view.show_subagent_list(subagent_items);
    }

    fn show_subagent_config_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(registry.get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: Some(&agent_type),
                workspace_root: Some(workspace.as_path()),
                list_scope: SubagentListScope::RegistryManagement,
                include_disabled: true,
                external_sources_supported: true,
            }))
        });

        let has_external_subagents = subagents
            .iter()
            .any(|info| info.subagent_source == Some(SubAgentSource::External));
        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .filter(|info| info.subagent_source != Some(SubAgentSource::External))
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            chat_state.add_system_message(if has_external_subagents {
                "No locally manageable subagents found. Open Agents from the command palette to review imported agents."
                    .to_string()
            } else {
                "No subagents found.".to_string()
            });
            return;
        }

        chat_view.show_subagent_config(subagent_items);
    }

    fn handle_subagent_selector_action(
        &mut self,
        action: SubagentSelectorAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match action {
            SubagentSelectorAction::ListSubagents => {
                self.show_available_subagent_list(chat_view, chat_state, rt_handle);
            }
            SubagentSelectorAction::ConfigureSubagents => {
                self.show_subagent_config_selector(chat_view, chat_state, rt_handle);
            }
            SubagentSelectorAction::Launch(selected) => {
                chat_view.hide_subagent_selector();
                self.apply_subagent_selection(&selected, chat_view);
            }
            SubagentSelectorAction::Toggle(selected) => {
                self.set_subagent_enabled(&selected, !selected.enabled, chat_state, rt_handle);
                self.show_subagent_config_selector(chat_view, chat_state, rt_handle);
            }
        }
    }

    /// Apply subagent selection: fill input box with launch command
    fn apply_subagent_selection(&self, selected: &SubagentItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!(
            "Launch subagent {} to finish task: ",
            selected.name
        ));
    }

    fn set_subagent_enabled(
        &self,
        selected: &SubagentItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let workspace = self.agent.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let subagent = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                registry
                    .update_subagent_override(
                        &mode_id,
                        &subagent.id,
                        enabled,
                        Some(workspace.as_path()),
                    )
                    .await
                    .map_err(|error| error.to_string())
            })
        });

        match result {
            Ok(()) => chat_state.add_system_message(format!(
                "Subagent '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            )),
            Err(error) => chat_state.add_system_message(format!(
                "Failed to update subagent '{}': {}",
                selected.name, error
            )),
        }
    }

    fn subagent_item_from_info(info: AgentInfo) -> SubagentItem {
        let source = match info.subagent_source {
            Some(SubAgentSource::Builtin) => "builtin",
            Some(SubAgentSource::Project) => "project",
            Some(SubAgentSource::User) => "user",
            Some(SubAgentSource::External) => "external",
            None => "builtin",
        }
        .to_string();

        SubagentItem {
            key: info.key,
            id: info.id,
            name: info.name,
            description: info.description,
            source,
            enabled: info.effective_enabled,
        }
    }
}
