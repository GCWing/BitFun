impl ChatView {
    // ============ Info popup methods ============

    pub fn show_info_popup(&mut self, message: String) {
        self.info_popup = Some(message);
        self.popup_stack.push(PopupType::InfoPopup);
    }

    pub fn info_popup_visible(&self) -> bool {
        self.info_popup.is_some()
    }

    pub fn dismiss_info_popup(&mut self) {
        self.info_popup = None;
    }

    #[allow(dead_code)]
    pub fn reshow_info_popup(&mut self, message: String) {
        self.info_popup = Some(message);
    }

    // ============ Command palette methods ============

    pub fn show_command_palette(&mut self) {
        self.command_palette.show();
        self.popup_stack.push(PopupType::CommandPalette);
    }

    pub fn hide_command_palette(&mut self) {
        self.command_palette.hide();
    }

    pub fn reshow_command_palette(&mut self) {
        self.command_palette.show();
    }

    pub fn command_palette_visible(&self) -> bool {
        self.command_palette.is_visible()
    }

    pub fn command_palette_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> PaletteAction {
        self.command_palette.handle_key_event(key)
    }

    pub fn command_palette_handle_mouse(
        &mut self,
        mouse: &crossterm::event::MouseEvent,
    ) -> PaletteAction {
        self.command_palette.handle_mouse_event(mouse)
    }

    pub fn command_palette_captures_mouse(
        &self,
        mouse: &crossterm::event::MouseEvent,
    ) -> bool {
        self.command_palette.captures_mouse(mouse)
    }

    // ============ Model selector methods ============

    pub fn show_model_selector(
        &mut self,
        models: Vec<ModelItem>,
        current_model_id: Option<String>,
    ) {
        self.model_selector.show(models, current_model_id);
        self.popup_stack.push(PopupType::ModelSelector);
    }

    pub fn hide_model_selector(&mut self) {
        self.model_selector.hide();
    }

    pub fn reshow_model_selector(&mut self) {
        self.model_selector.reshow();
    }

    pub fn model_selector_visible(&self) -> bool {
        self.model_selector.is_visible()
    }

    pub fn model_selector_up(&mut self) {
        self.model_selector.move_up();
    }

    pub fn model_selector_down(&mut self) {
        self.model_selector.move_down();
    }

    pub fn model_selector_confirm(&self) -> Option<ModelItem> {
        self.model_selector.confirm_selection()
    }

    // ============ Theme selector methods ============

    pub fn show_theme_selector(
        &mut self,
        themes: Vec<ThemeItem>,
        current_theme_id: Option<String>,
    ) {
        self.theme_selector.show(themes, current_theme_id);
        self.popup_stack.push(PopupType::ThemeSelector);
    }

    pub fn hide_theme_selector(&mut self) {
        self.theme_selector.hide();
    }

    pub fn reshow_theme_selector(&mut self) {
        self.theme_selector.reshow();
    }

    pub fn theme_selector_visible(&self) -> bool {
        self.theme_selector.is_visible()
    }

    pub fn theme_selector_up(&mut self) {
        self.theme_selector.move_up();
    }

    pub fn theme_selector_down(&mut self) {
        self.theme_selector.move_down();
    }

    pub fn theme_selector_confirm(&self) -> Option<ThemeItem> {
        self.theme_selector.confirm_selection()
    }

    pub fn theme_selector_selected(&self) -> Option<ThemeItem> {
        self.theme_selector.selected_item().cloned()
    }

    // ============ Agent selector methods ============

    pub fn show_agent_selector(
        &mut self,
        agents: Vec<AgentItem>,
        current_agent_id: Option<String>,
    ) {
        self.agent_selector.show(agents, current_agent_id);
        self.popup_stack.push(PopupType::AgentSelector);
    }

    pub fn hide_agent_selector(&mut self) {
        self.agent_selector.hide();
    }

    pub fn reshow_agent_selector(&mut self) {
        self.agent_selector.reshow();
    }

    pub fn agent_selector_visible(&self) -> bool {
        self.agent_selector.is_visible()
    }

    pub fn agent_selector_up(&mut self) {
        self.agent_selector.move_up();
    }

    pub fn agent_selector_down(&mut self) {
        self.agent_selector.move_down();
    }

    pub fn agent_selector_confirm(&self) -> Option<AgentItem> {
        self.agent_selector.confirm_selection()
    }

    // ============ Skill selector methods ============

    pub fn show_skill_selector(&mut self, skills: Vec<SkillItem>) {
        self.skill_selector.show(skills);
        self.popup_stack.push(PopupType::SkillSelector);
    }

    pub fn hide_skill_selector(&mut self) {
        self.skill_selector.hide();
    }

    pub fn reshow_skill_selector(&mut self) {
        self.skill_selector.reshow();
    }

    pub fn skill_selector_visible(&self) -> bool {
        self.skill_selector.is_visible()
    }

    pub fn skill_selector_up(&mut self) {
        self.skill_selector.move_up();
    }

    pub fn skill_selector_down(&mut self) {
        self.skill_selector.move_down();
    }

    pub fn skill_selector_confirm(&self) -> Option<SkillItem> {
        self.skill_selector.confirm_selection()
    }

    // ============ Subagent selector methods ============

    pub fn show_subagent_selector(&mut self, subagents: Vec<SubagentItem>) {
        self.subagent_selector.show(subagents);
        self.popup_stack.push(PopupType::SubagentSelector);
    }

    pub fn hide_subagent_selector(&mut self) {
        self.subagent_selector.hide();
    }

    pub fn reshow_subagent_selector(&mut self) {
        self.subagent_selector.reshow();
    }

    pub fn subagent_selector_visible(&self) -> bool {
        self.subagent_selector.is_visible()
    }

    pub fn subagent_selector_up(&mut self) {
        self.subagent_selector.move_up();
    }

    pub fn subagent_selector_down(&mut self) {
        self.subagent_selector.move_down();
    }

    pub fn subagent_selector_confirm(&self) -> Option<SubagentItem> {
        self.subagent_selector.confirm_selection()
    }

    // ============ MCP selector methods ============

    pub fn show_mcp_selector(&mut self, items: Vec<McpItem>) {
        self.mcp_selector.show(items);
        self.popup_stack.push(PopupType::McpSelector);
    }

    pub fn hide_mcp_selector(&mut self) {
        self.mcp_selector.hide();
    }

    pub fn reshow_mcp_selector(&mut self) {
        self.mcp_selector.reshow();
    }

    pub fn mcp_selector_visible(&self) -> bool {
        self.mcp_selector.is_visible()
    }

    pub fn mcp_selector_up(&mut self) {
        self.mcp_selector.move_up();
    }

    pub fn mcp_selector_down(&mut self) {
        self.mcp_selector.move_down();
    }

    pub fn mcp_selector_confirm(&self) -> Option<McpItem> {
        self.mcp_selector.confirm_selection()
    }

    pub fn mcp_selector_set_loading(&mut self, id: Option<String>) {
        self.mcp_selector.loading_id = id;
    }

    pub fn mcp_selector_update_items(&mut self, items: Vec<McpItem>) {
        self.mcp_selector.update_items(items);
    }

    /// Take the pending MCP toggle (set by mouse click)
    pub fn take_pending_mcp_toggle(&mut self) -> Option<String> {
        self.pending_mcp_toggle.take()
    }

    pub fn mcp_selector_start_confirm_delete(&mut self, server_id: String) {
        self.mcp_selector.start_confirm_delete(server_id);
    }

    pub fn mcp_selector_cancel_confirm_delete(&mut self) {
        self.mcp_selector.cancel_confirm_delete();
    }

    pub fn mcp_selector_is_confirm_delete(&self, server_id: &str) -> bool {
        self.mcp_selector.is_confirm_delete(server_id)
    }

    // ============ MCP add dialog methods ============

    pub fn show_mcp_add_dialog(&mut self) {
        self.mcp_add_dialog.show();
        self.popup_stack.push(PopupType::McpAddDialog);
    }

    pub fn mcp_add_dialog_visible(&self) -> bool {
        self.mcp_add_dialog.is_visible()
    }

    pub fn mcp_add_dialog_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> McpAddAction {
        self.mcp_add_dialog.handle_key_event(key)
    }

    pub fn mcp_add_dialog_handle_paste(&mut self, text: &str) {
        self.mcp_add_dialog.insert_text(text);
    }

    pub fn hide_mcp_add_dialog(&mut self) {
        self.mcp_add_dialog.hide();
    }

    pub fn reshow_mcp_add_dialog(&mut self) {
        self.mcp_add_dialog.show();
    }

    // ============ Session selector methods ============

    pub fn show_session_selector(
        &mut self,
        sessions: Vec<SessionItem>,
        current_session_id: Option<String>,
    ) {
        self.session_selector.show(sessions, current_session_id);
        self.popup_stack.push(PopupType::SessionSelector);
    }

    pub fn session_selector_visible(&self) -> bool {
        self.session_selector.is_visible()
    }

    pub fn hide_session_selector(&mut self) {
        self.session_selector.hide();
    }

    pub fn reshow_session_selector(&mut self) {
        self.session_selector.reshow();
    }

    pub fn session_selector_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> SessionAction {
        self.session_selector.handle_key_event(key)
    }

    pub fn session_selector_remove_item(&mut self, session_id: &str) {
        self.session_selector.remove_item(session_id);
    }

    // ============ Provider selector methods (add model step 1) ============

    pub fn show_provider_selector(&mut self) {
        self.provider_selector.show();
        self.popup_stack.push(PopupType::ProviderSelector);
    }

    pub fn provider_selector_visible(&self) -> bool {
        self.provider_selector.is_visible()
    }

    pub fn hide_provider_selector(&mut self) {
        self.provider_selector.hide();
    }

    pub fn reshow_provider_selector(&mut self) {
        self.provider_selector.show();
    }

    pub fn provider_selector_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<ProviderSelection> {
        self.provider_selector.handle_key_event(key)
    }

    pub fn provider_selector_handle_mouse(
        &mut self,
        mouse: &crossterm::event::MouseEvent,
    ) -> Option<ProviderSelection> {
        self.provider_selector.handle_mouse_event(mouse)
    }

    pub fn provider_selector_captures_mouse(
        &self,
        mouse: &crossterm::event::MouseEvent,
    ) -> bool {
        self.provider_selector.captures_mouse(mouse)
    }

    // ============ Model config form methods (add model step 2) ============

    pub fn show_model_config_form_custom(&mut self) {
        self.model_config_form.show_custom();
        self.popup_stack.push(PopupType::ModelConfigForm);
    }

    pub fn show_model_config_form_from_provider(
        &mut self,
        provider_name: &str,
        base_url: &str,
        format: &str,
        default_model: &str,
    ) {
        self.model_config_form
            .show_from_provider(provider_name, base_url, format, default_model);
        self.popup_stack.push(PopupType::ModelConfigForm);
    }

    pub fn show_model_config_form_for_edit(
        &mut self,
        model_id: &str,
        result: &super::model_config_form::ModelFormResult,
    ) {
        self.model_config_form.show_for_edit(model_id, result);
        self.popup_stack.push(PopupType::ModelConfigForm);
    }

    pub fn model_config_form_visible(&self) -> bool {
        self.model_config_form.is_visible()
    }

    pub fn hide_model_config_form(&mut self) {
        self.model_config_form.hide();
    }

    pub fn reshow_model_config_form(&mut self) {
        self.model_config_form.reshow();
    }

    pub fn model_config_form_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> ModelFormAction {
        self.model_config_form.handle_key_event(key)
    }
}

