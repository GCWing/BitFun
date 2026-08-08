impl ChatMode {
    fn show_skill_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        _rt_handle: &tokio::runtime::Handle,
    ) {
        chat_view.show_skill_menu();
    }

    fn reload_context(
        &self,
        target: bitfun_runtime_ports::AgentContextReloadTarget,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        use bitfun_runtime_ports::{AgentContextReloadRequest, AgentContextReloadTarget};

        let request = AgentContextReloadRequest {
            session_id: chat_state.core_session_id.clone(),
            target,
        };
        let outcome =
            tokio::task::block_in_place(|| rt_handle.block_on(self.agent.reload_context(request)));

        match outcome {
            Ok(_) => {
                let message = match target {
                    AgentContextReloadTarget::All => {
                        "Reloaded skills. Instructions will be reread for the next message."
                    }
                    AgentContextReloadTarget::Skills => "Reloaded skills.",
                    AgentContextReloadTarget::Instructions => {
                        "Instructions will be reread for the next message."
                    }
                };
                chat_state.add_system_message(message.to_string());
                chat_view.set_status(Some(message.to_string()));
            }
            Err(error) => {
                chat_state.add_system_message(format!("Could not reload context: {error}"));
                chat_view.set_status(Some("Context reload failed".to_string()));
            }
        }
    }

    fn show_available_skill_list(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            rt_handle.block_on(self.agent.list_skills(self.agent_type.clone(), false))
        });
        let skills = match skills {
            Ok(response) => response.skills,
            Err(error) => {
                chat_state.add_system_message(format!("Could not load skills: {error}"));
                return;
            }
        };

        if skills.is_empty() {
            chat_state.add_system_message(format!(
                "No user-invocable skills found for agent mode '{}'. Add or enable a skill, then check its user-invocable metadata.",
                self.agent_type
            ));
            return;
        }

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(Self::skill_item_from_summary)
            .collect();

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
            rt_handle.block_on(self.agent.list_skills(self.agent_type.clone(), true))
        });
        let skills = match skills {
            Ok(response) => response.skills,
            Err(error) => {
                chat_state.add_system_message(format!("Could not load skills: {error}"));
                return;
            }
        };

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(Self::skill_item_from_summary)
            .collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_config(skill_items);
    }

    fn handle_skill_selector_action(
        &mut self,
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
    fn apply_skill_selection(&mut self, selected: &SkillItem, chat_view: &mut ChatView) {
        chat_view.set_input(&selected.invocation_text());
        self.selected_native_command_once = None;
    }

    fn set_skill_enabled(
        &self,
        selected: &SkillItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mode_id = self.agent_type.clone();
        let skill = selected.clone();

        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(self.agent.set_skill_enabled(
                mode_id,
                skill.key,
                enabled,
                skill.default_enabled,
                skill.level,
            ))
        });

        match result {
            Ok(_) => chat_state.add_system_message(format!(
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

    fn skill_item_from_summary(info: SkillSummary) -> SkillItem {
        SkillItem {
            key: info.key,
            name: info.name,
            description: info.description,
            level: info.level,
            source_slot: info.source_slot.unwrap_or_default(),
            source_label: info.source_label.unwrap_or_default(),
            enabled: info.enabled,
            selected_for_runtime: info.selected_for_runtime,
            default_enabled: info.default_enabled,
            is_shadowed: info.is_shadowed,
            shadowed_by_key: info.shadowed_by_key,
            argument_hint: info.argument_hint,
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
        let subagents = tokio::task::block_in_place(|| {
            rt_handle.block_on(self.agent.list_subagents(self.agent_type.clone(), false))
        });
        let subagents = match subagents {
            Ok(response) => response.subagents,
            Err(error) => {
                chat_state.add_system_message(format!("Could not load subagents: {error}"));
                return;
            }
        };

        if subagents.is_empty() {
            chat_state.add_system_message(format!(
                "No enabled subagents found for agent mode '{}'.",
                self.agent_type
            ));
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_summary)
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
        let subagents = tokio::task::block_in_place(|| {
            rt_handle.block_on(self.agent.list_subagents(self.agent_type.clone(), true))
        });
        let response = match subagents {
            Ok(response) => response,
            Err(error) => {
                chat_state.add_system_message(format!("Could not load subagents: {error}"));
                return;
            }
        };
        let has_external_subagents = response.has_external;
        let subagent_items: Vec<SubagentItem> = response
            .subagents
            .into_iter()
            .map(Self::subagent_item_from_summary)
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
    fn apply_subagent_selection(&mut self, selected: &SubagentItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!(
            "Launch subagent {} to finish task: ",
            selected.name
        ));
        self.selected_native_command_once = None;
    }

    fn set_subagent_enabled(
        &self,
        selected: &SubagentItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mode_id = self.agent_type.clone();
        let subagent = selected.clone();

        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(
                self.agent
                    .set_subagent_enabled(mode_id, subagent.id, enabled),
            )
        });

        match result {
            Ok(_) => chat_state.add_system_message(format!(
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

    fn subagent_item_from_summary(info: SubagentSummary) -> SubagentItem {
        SubagentItem {
            key: info.key,
            id: info.id,
            name: info.name,
            description: info.description,
            source: info.source,
            enabled: info.enabled,
        }
    }
}
