impl ChatView {
    // ============ Input handling methods (delegate to TextInput) ============

    pub(crate) fn input_text(&self) -> &str {
        self.text_input.text()
    }

    fn refresh_command_menu(&mut self) {
        self.command_menu
            .update(&self.text_input.input, self.text_input.cursor);
    }

    pub(crate) fn set_external_source_state(
        &mut self,
        commands: Vec<crate::ui::command_menu::ExternalCommandProjection>,
        discovery_pending: bool,
        builtin_reconfirmations: std::collections::BTreeSet<String>,
    ) {
        self.command_menu.set_external_source_state(
            commands,
            discovery_pending,
            builtin_reconfirmations,
        );
        self.refresh_command_menu();
    }

    /// Send user input, returns the input text if non-empty
    pub(crate) fn send_input(&mut self) -> Option<ComposerDraft> {
        self.retain_valid_workspace_references();
        let text = self.text_input.take_input()?;
        let draft = ComposerDraft {
            text,
            workspace_references: std::mem::take(&mut self.workspace_references),
        };

        self.input_history.push_front(draft.clone());
        if self.input_history.len() > 50 {
            self.input_history.pop_back();
        }
        self.history_index = None;
        self.refresh_command_menu();

        self.workspace_reference_popup.hide();
        Some(draft)
    }

    pub(crate) fn handle_char(&mut self, c: char) {
        let cursor = self.text_input.cursor;
        self.text_input.handle_char(c);
        let inserted = self.text_input.cursor.saturating_sub(cursor);
        self.reconcile_workspace_reference_edit(cursor, 0, inserted);
        self.retain_valid_workspace_references();
        self.refresh_command_menu();
    }

    pub(crate) fn insert_paste(&mut self, text: &str) {
        let cursor = self.text_input.cursor;
        self.text_input.insert_paste(text);
        let inserted = self.text_input.cursor.saturating_sub(cursor);
        self.reconcile_workspace_reference_edit(cursor, 0, inserted);
        self.retain_valid_workspace_references();
        self.refresh_command_menu();
    }

    pub(crate) fn handle_newline(&mut self) {
        let cursor = self.text_input.cursor;
        self.text_input.handle_newline();
        self.reconcile_workspace_reference_edit(cursor, 0, 1);
        self.retain_valid_workspace_references();
        self.refresh_command_menu();
    }

    pub(crate) fn handle_backspace(&mut self) {
        let cursor = self.text_input.cursor;
        self.text_input.handle_backspace();
        if self.text_input.cursor < cursor {
            self.reconcile_workspace_reference_edit(cursor - 1, 1, 0);
        }
        self.retain_valid_workspace_references();
        self.refresh_command_menu();
    }

    pub(crate) fn handle_delete(&mut self) {
        let cursor = self.text_input.cursor;
        let before = self.text_input.input.chars().count();
        self.text_input.handle_delete();
        if self.text_input.input.chars().count() < before {
            self.reconcile_workspace_reference_edit(cursor, 1, 0);
        }
        self.retain_valid_workspace_references();
        self.refresh_command_menu();
    }

    pub(crate) fn move_cursor_left(&mut self) {
        self.text_input.move_cursor_left();
        self.refresh_command_menu();
    }

    pub(crate) fn move_cursor_right(&mut self) {
        self.text_input.move_cursor_right();
        self.refresh_command_menu();
    }

    pub(crate) fn set_cursor_home(&mut self) {
        self.text_input.set_cursor_home();
        self.refresh_command_menu();
    }

    pub(crate) fn set_cursor_end(&mut self) {
        self.text_input.set_cursor_end();
        self.refresh_command_menu();
    }

    pub(crate) fn clear_input(&mut self) {
        self.text_input.clear();
        self.workspace_references.clear();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    /// Set input text programmatically (e.g. from skill selection)
    pub(crate) fn set_input(&mut self, text: &str) {
        self.text_input.set_text(text);
        self.workspace_references.clear();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    pub(crate) fn set_draft(&mut self, mut draft: ComposerDraft) {
        draft.retain_valid_sources();
        self.text_input.set_text(&draft.text);
        self.workspace_references = draft.workspace_references;
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    pub(crate) fn current_workspace_reference_query(&self) -> Option<WorkspaceReferenceQuery> {
        super::workspace_reference::workspace_reference_query(
            &self.text_input.input,
            self.text_input.cursor,
        )
    }

    pub(crate) fn set_workspace_reference_query(&mut self, query: Option<WorkspaceReferenceQuery>) {
        self.workspace_reference_popup.set_query(query);
    }

    pub(crate) fn set_workspace_reference_results(
        &mut self,
        entries: Vec<bitfun_agent_runtime::sdk::AgentWorkspaceReferenceSearchEntry>,
    ) {
        self.workspace_reference_popup.set_results(entries);
    }

    pub(crate) fn workspace_reference_popup_visible(&self) -> bool {
        self.workspace_reference_popup.is_visible()
    }

    pub(crate) fn workspace_reference_up(&mut self) {
        self.workspace_reference_popup.up();
    }

    pub(crate) fn workspace_reference_down(&mut self) {
        self.workspace_reference_popup.down();
    }

    pub(crate) fn hide_workspace_reference_popup(&mut self) {
        self.workspace_reference_popup.hide();
    }

    pub(crate) fn apply_workspace_reference_selection(&mut self, drill_directory: bool) -> bool {
        let Some(query) = self.workspace_reference_popup.query.clone() else {
            return false;
        };
        let Some(entry) = self.workspace_reference_popup.selected() else {
            return false;
        };
        if drill_directory
            && entry.kind == bitfun_agent_runtime::sdk::AgentWorkspaceReferenceKind::Directory
        {
            let replacement = format!("@{}/", entry.path);
            self.replace_workspace_reference_token(&query, &replacement, None, false);
            return true;
        }
        let (replacement, reference) =
            super::workspace_reference::reference_from_selection(&query, &entry);
        self.replace_workspace_reference_token(&query, &replacement, Some(reference), true);
        true
    }

    fn replace_workspace_reference_token(
        &mut self,
        query: &WorkspaceReferenceQuery,
        replacement: &str,
        reference: Option<bitfun_agent_runtime::sdk::AgentWorkspaceReference>,
        trailing_space: bool,
    ) {
        let removed = query.token_end.saturating_sub(query.token_start);
        let inserted = replacement.chars().count() + usize::from(trailing_space);
        self.reconcile_workspace_reference_edit(query.token_start, removed, inserted);
        let text = if trailing_space {
            format!("{replacement} ")
        } else {
            replacement.to_string()
        };
        self.text_input
            .replace_char_range(query.token_start, query.token_end, &text);
        if let Some(reference) = reference {
            self.workspace_references.push(reference);
        }
        self.retain_valid_workspace_references();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    fn reconcile_workspace_reference_edit(
        &mut self,
        edit_start: usize,
        removed_chars: usize,
        inserted_chars: usize,
    ) {
        let mut draft = ComposerDraft {
            text: String::new(),
            workspace_references: std::mem::take(&mut self.workspace_references),
        };
        draft.reconcile_edit(edit_start, removed_chars, inserted_chars);
        self.workspace_references = draft.workspace_references;
    }

    fn retain_valid_workspace_references(&mut self) {
        let mut draft = ComposerDraft {
            text: self.text_input.input.clone(),
            workspace_references: std::mem::take(&mut self.workspace_references),
        };
        draft.retain_valid_sources();
        self.workspace_references = draft.workspace_references;
    }

    pub(crate) fn command_menu_visible(&self) -> bool {
        self.command_menu.is_visible()
    }

    pub(crate) fn command_menu_up(&mut self) {
        self.command_menu.move_up();
    }

    pub(crate) fn command_menu_down(&mut self) {
        self.command_menu.move_down();
    }

    pub(crate) fn apply_command_menu_selection(
        &mut self,
    ) -> Option<crate::ui::command_menu::CommandMenuSelection> {
        let cmd = self.command_menu.apply_selection_with_name()?;
        self.text_input.clear();
        self.refresh_command_menu();
        Some(cmd)
    }

    pub(crate) fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => 0,
            Some(i) if i + 1 < self.input_history.len() => i + 1,
            Some(i) => i,
        };

        if let Some(history_item) = self.input_history.get(new_index) {
            self.text_input.set_text(&history_item.text);
            self.workspace_references = history_item.workspace_references.clone();
            self.history_index = Some(new_index);
            self.refresh_command_menu();
        }
    }

    pub(crate) fn history_next(&mut self) {
        match self.history_index {
            None => {}
            Some(0) => {
                self.text_input.clear();
                self.workspace_references.clear();
                self.history_index = None;
                self.refresh_command_menu();
            }
            Some(i) => {
                let new_index = i - 1;
                if let Some(history_item) = self.input_history.get(new_index) {
                    self.text_input.set_text(&history_item.text);
                    self.workspace_references = history_item.workspace_references.clone();
                    self.history_index = Some(new_index);
                    self.refresh_command_menu();
                }
            }
        }
    }
}
