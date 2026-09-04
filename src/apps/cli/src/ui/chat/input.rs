const MAX_LOCAL_DRAFT_HISTORY: usize = 50;
const MAX_LOCAL_DRAFT_HISTORY_IMAGE_BYTES: usize = 200 * 1024 * 1024;

impl SubmittedDraftHistory {
    fn record(&mut self, session_id: &str, draft: ComposerDraft) {
        self.record_with_mode(session_id, draft, ComposerMode::Chat);
    }

    fn record_with_mode(&mut self, session_id: &str, draft: ComposerDraft, mode: ComposerMode) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let history = self.sessions.entry(session_id.to_string()).or_default();
        history.undone.clear();
        history.active.push(SubmittedDraftRecord {
            sequence,
            draft,
            mode,
        });
        while self.record_count() > MAX_LOCAL_DRAFT_HISTORY {
            if !self.remove_oldest_record() {
                break;
            }
        }
    }

    #[cfg(test)]
    fn undo(&mut self, session_id: &str, text: &str) -> Option<ComposerDraft> {
        self.undo_with_mode(session_id, text)
            .map(|(draft, _)| draft)
    }

    fn undo_with_mode(
        &mut self,
        session_id: &str,
        text: &str,
    ) -> Option<(ComposerDraft, ComposerMode)> {
        let history = self.sessions.get_mut(session_id)?;
        if history
            .active
            .last()
            .is_some_and(|record| match record.mode {
                ComposerMode::Chat => record.draft.text == text,
                ComposerMode::Shell => text
                    .strip_prefix('!')
                    .is_some_and(|command| record.draft.text == command),
            })
        {
            let record = history.active.pop()?;
            let draft = record.draft.clone();
            let mode = record.mode;
            history.undone.push(record);
            Some((draft, mode))
        } else {
            // Runtime state is authoritative. If its reverted text no longer matches the
            // local success stack, fail closed for this Session instead of guessing by text.
            history.active.clear();
            history.undone.clear();
            self.sessions.remove(session_id);
            None
        }
    }

    fn redo(&mut self, session_id: &str) {
        let Some(history) = self.sessions.get_mut(session_id) else {
            return;
        };
        if let Some(record) = history.undone.pop() {
            history.active.push(record);
        }
    }

    fn image_bytes(&self) -> usize {
        self.sessions
            .values()
            .flat_map(|history| history.active.iter().chain(&history.undone))
            .map(|record| record.draft.image_byte_len())
            .fold(0usize, usize::saturating_add)
    }

    fn drop_oldest_image_metadata(&mut self) -> bool {
        let oldest = self
            .sessions
            .iter()
            .flat_map(|(session_id, history)| {
                history
                    .active
                    .iter()
                    .enumerate()
                    .map(move |(index, record)| (session_id, true, index, record))
                    .chain(
                        history
                            .undone
                            .iter()
                            .enumerate()
                            .map(move |(index, record)| (session_id, false, index, record)),
                    )
            })
            .filter(|(_, _, _, record)| record.draft.has_images())
            .min_by_key(|(_, _, _, record)| record.sequence)
            .map(|(session_id, active, index, _)| (session_id.clone(), active, index));
        let Some((session_id, active, index)) = oldest else {
            return false;
        };
        let history = self.sessions.get_mut(&session_id).expect("history exists");
        let record = if active {
            &mut history.active[index]
        } else {
            &mut history.undone[index]
        };
        record.draft.drop_image_metadata();
        true
    }

    fn record_count(&self) -> usize {
        self.sessions
            .values()
            .map(|history| history.active.len() + history.undone.len())
            .sum()
    }

    fn remove_oldest_record(&mut self) -> bool {
        let oldest = self
            .sessions
            .iter()
            .flat_map(|(session_id, history)| {
                history
                    .active
                    .iter()
                    .enumerate()
                    .map(move |(index, record)| (session_id, true, index, record.sequence))
                    .chain(
                        history
                            .undone
                            .iter()
                            .enumerate()
                            .map(move |(index, record)| {
                                (session_id, false, index, record.sequence)
                            }),
                    )
            })
            .min_by_key(|(_, _, _, sequence)| *sequence)
            .map(|(session_id, active, index, _)| (session_id.clone(), active, index));
        let Some((session_id, active, index)) = oldest else {
            return false;
        };
        let history = self.sessions.get_mut(&session_id).expect("history exists");
        if active {
            history.active.remove(index);
        } else {
            // Removing one redo record would make the remaining local stack disagree with
            // Runtime order. Discard this Session's redo metadata and fail closed instead.
            history.undone.clear();
        }
        if history.active.is_empty() && history.undone.is_empty() {
            self.sessions.remove(&session_id);
        }
        true
    }
}

impl ChatView {
    // ============ Input handling methods (delegate to TextInput) ============

    pub(crate) fn input_text(&self) -> &str {
        self.text_input.text()
    }

    pub(crate) fn draft_snapshot(&self) -> ComposerDraft {
        ComposerDraft {
            text: self.text_input.text().to_string(),
            workspace_references: self.workspace_references.clone(),
            image_attachments: self.image_attachments.clone(),
        }
    }

    fn refresh_command_menu(&mut self) {
        if self.is_shell_mode() {
            self.command_menu.update("", 0);
        } else {
            self.command_menu
                .update(&self.text_input.input, self.text_input.cursor);
        }
    }

    pub(crate) fn is_shell_mode(&self) -> bool {
        self.composer_mode == ComposerMode::Shell
    }

    pub(crate) fn try_enter_shell_mode(&mut self) -> bool {
        if self.is_shell_mode()
            || !self.text_input.text().is_empty()
            || !self.workspace_references.is_empty()
            || !self.image_attachments.is_empty()
        {
            return false;
        }
        self.composer_mode = ComposerMode::Shell;
        self.history_index = None;
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
        true
    }

    pub(crate) fn exit_shell_mode(&mut self) -> bool {
        if !self.is_shell_mode() {
            return false;
        }
        self.composer_mode = ComposerMode::Chat;
        self.history_index = None;
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
        true
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
        self.retain_valid_composer_sources();
        let text = self.text_input.take_input()?;
        let draft = ComposerDraft {
            text,
            workspace_references: std::mem::take(&mut self.workspace_references),
            image_attachments: std::mem::take(&mut self.image_attachments),
        };

        let history = if self.is_shell_mode() {
            &mut self.shell_input_history
        } else {
            &mut self.input_history
        };
        history.push_front(draft.clone());
        if history.len() > MAX_LOCAL_DRAFT_HISTORY {
            history.pop_back();
        }
        self.enforce_draft_history_image_budget();
        self.history_index = None;
        self.refresh_command_menu();

        self.workspace_reference_popup.hide();
        Some(draft)
    }

    pub(crate) fn handle_char(&mut self, c: char) {
        let cursor = self.safe_insertion_cursor();
        self.text_input.cursor = cursor;
        self.text_input.handle_char(c);
        let inserted = self.text_input.cursor.saturating_sub(cursor);
        self.reconcile_composer_edit(cursor, 0, inserted);
        self.retain_valid_composer_sources();
        self.refresh_command_menu();
    }

    pub(crate) fn insert_paste(&mut self, text: &str) {
        let cursor = self.safe_insertion_cursor();
        self.text_input.cursor = cursor;
        self.text_input.insert_paste(text);
        let inserted = self.text_input.cursor.saturating_sub(cursor);
        self.reconcile_composer_edit(cursor, 0, inserted);
        self.retain_valid_composer_sources();
        self.refresh_command_menu();
    }

    pub(crate) fn insert_image(
        &mut self,
        image: super::composer::ComposerImage,
    ) -> Result<(), super::composer::ComposerImageInsertError> {
        self.insert_image_with_budget(image, MAX_LOCAL_DRAFT_HISTORY_IMAGE_BYTES)
    }

    fn insert_image_with_budget(
        &mut self,
        image: super::composer::ComposerImage,
        image_budget: usize,
    ) -> Result<(), super::composer::ComposerImageInsertError> {
        if self.is_shell_mode() {
            return Err(super::composer::ComposerImageInsertError::ShellModeUnsupported);
        }
        let mut draft = self.draft_snapshot();
        let cursor = draft.safe_insertion_cursor(self.text_input.cursor);
        let cursor = draft.insert_image(cursor, image)?;
        if !self.make_room_for_projected_active_image_bytes(draft.image_byte_len(), image_budget) {
            return Err(super::composer::ComposerImageInsertError::LocalDraftBudgetExceeded);
        }
        self.apply_draft_at_cursor(draft, cursor);
        Ok(())
    }

    pub(crate) fn handle_newline(&mut self) {
        let cursor = self.safe_insertion_cursor();
        self.text_input.cursor = cursor;
        self.text_input.handle_newline();
        self.reconcile_composer_edit(cursor, 0, 1);
        self.retain_valid_composer_sources();
        self.refresh_command_menu();
    }

    pub(crate) fn handle_backspace(&mut self) {
        let cursor = self.text_input.cursor;
        if cursor > 0 {
            let mut draft = self.draft_snapshot();
            if let Some(cursor) = draft.remove_image_overlapping_edit(cursor - 1, 1) {
                self.apply_draft_at_cursor(draft, cursor);
                return;
            }
        }
        self.text_input.handle_backspace();
        if self.text_input.cursor < cursor {
            self.reconcile_composer_edit(cursor - 1, 1, 0);
        }
        self.retain_valid_composer_sources();
        self.refresh_command_menu();
    }

    pub(crate) fn handle_delete(&mut self) {
        let cursor = self.text_input.cursor;
        let mut draft = self.draft_snapshot();
        if let Some(cursor) = draft.remove_image_overlapping_edit(cursor, 1) {
            self.apply_draft_at_cursor(draft, cursor);
            return;
        }
        let before = self.text_input.input.chars().count();
        self.text_input.handle_delete();
        if self.text_input.input.chars().count() < before {
            self.reconcile_composer_edit(cursor, 1, 0);
        }
        self.retain_valid_composer_sources();
        self.refresh_command_menu();
    }

    pub(crate) fn move_cursor_left(&mut self) {
        self.text_input.cursor = self.draft_snapshot().cursor_left(self.text_input.cursor);
        self.refresh_command_menu();
    }

    pub(crate) fn move_cursor_right(&mut self) {
        self.text_input.cursor = self.draft_snapshot().cursor_right(self.text_input.cursor);
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
        self.image_attachments.clear();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    /// Set input text programmatically (e.g. from skill selection)
    pub(crate) fn set_input(&mut self, text: &str) {
        self.composer_mode = ComposerMode::Chat;
        self.text_input.set_text(text);
        self.workspace_references.clear();
        self.image_attachments.clear();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    pub(crate) fn set_draft(&mut self, mut draft: ComposerDraft) {
        draft.retain_valid_sources();
        self.text_input.set_text(&draft.text);
        self.workspace_references = draft.workspace_references;
        self.image_attachments = draft.image_attachments;
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    pub(crate) fn set_chat_draft(&mut self, draft: ComposerDraft) {
        self.composer_mode = ComposerMode::Chat;
        self.history_index = None;
        self.set_draft(draft);
    }

    /// Make `next_session_id` the owner of the visible composer.
    ///
    /// Runtime Session state intentionally does not own unsent UI input. Keep
    /// only inactive Session composers here and move, rather than clone, the
    /// target draft back into the existing active composer fields.
    pub(crate) fn activate_session_composer(
        &mut self,
        previous_session_id: &str,
        next_session_id: &str,
    ) {
        if previous_session_id == next_session_id {
            return;
        }

        let current = InactiveSessionComposer {
            draft: self.draft_snapshot(),
            mode: self.composer_mode,
        };
        if current.draft == ComposerDraft::default() && current.mode == ComposerMode::Chat {
            self.inactive_session_composers.remove(previous_session_id);
        } else {
            self.inactive_session_composers
                .insert(previous_session_id.to_string(), current);
        }

        let next = self
            .inactive_session_composers
            .remove(next_session_id)
            .unwrap_or_default();
        self.composer_mode = next.mode;
        self.set_draft(next.draft);
        self.history_index = None;
        self.enforce_draft_history_image_budget();
    }

    pub(crate) fn forget_session_composer(&mut self, session_id: &str) {
        self.inactive_session_composers.remove(session_id);
    }

    pub(crate) fn restore_undo_draft(
        &mut self,
        session_id: &str,
        text: String,
        workspace_references: Vec<openbitfun_agent_runtime::sdk::AgentWorkspaceReference>,
    ) -> ComposerDraft {
        let restored = self.submitted_drafts.undo_with_mode(session_id, &text);
        let (mut draft, mode) =
            restored.unwrap_or_else(|| (ComposerDraft::from_text(text), ComposerMode::Chat));
        self.composer_mode = mode;
        draft.workspace_references = workspace_references;
        draft.retain_valid_sources();
        draft
    }

    pub(crate) fn remember_submitted_draft(&mut self, session_id: &str, draft: &ComposerDraft) {
        self.submitted_drafts.record(session_id, draft.clone());
        self.enforce_draft_history_image_budget();
    }

    pub(crate) fn remember_submitted_shell_command(
        &mut self,
        session_id: &str,
        draft: &ComposerDraft,
    ) {
        self.submitted_drafts
            .record_with_mode(session_id, draft.clone(), ComposerMode::Shell);
        self.enforce_draft_history_image_budget();
    }

    pub(crate) fn note_session_redo(&mut self, session_id: &str) {
        self.submitted_drafts.redo(session_id);
        self.enforce_draft_history_image_budget();
    }

    fn enforce_draft_history_image_budget(&mut self) {
        self.enforce_draft_history_image_budget_with_limit(MAX_LOCAL_DRAFT_HISTORY_IMAGE_BYTES);
    }

    fn enforce_draft_history_image_budget_with_limit(&mut self, limit: usize) {
        let active_bytes = self.draft_snapshot().image_byte_len();
        let _ = self.make_room_for_projected_active_image_bytes(active_bytes, limit);
    }

    fn make_room_for_projected_active_image_bytes(
        &mut self,
        projected_active_bytes: usize,
        limit: usize,
    ) -> bool {
        loop {
            let input_bytes = self
                .input_history
                .iter()
                .map(ComposerDraft::image_byte_len)
                .fold(0usize, usize::saturating_add);
            let inactive_bytes = self
                .inactive_session_composers
                .values()
                .map(|composer| composer.draft.image_byte_len())
                .fold(0usize, usize::saturating_add);
            if projected_active_bytes
                .saturating_add(input_bytes)
                .saturating_add(inactive_bytes)
                .saturating_add(self.submitted_drafts.image_bytes())
                <= limit
            {
                return true;
            }
            if let Some(draft) = self
                .input_history
                .iter_mut()
                .rev()
                .find(|draft| draft.has_images())
            {
                draft.drop_image_metadata();
            } else if self.submitted_drafts.drop_oldest_image_metadata() {
                continue;
            } else {
                return false;
            }
        }
    }

    pub(crate) fn current_workspace_reference_query(&self) -> Option<WorkspaceReferenceQuery> {
        if self.is_shell_mode() {
            return None;
        }
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
        entries: Vec<openbitfun_agent_runtime::sdk::AgentWorkspaceReferenceSearchEntry>,
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
            && entry.kind == openbitfun_agent_runtime::sdk::AgentWorkspaceReferenceKind::Directory
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
        reference: Option<openbitfun_agent_runtime::sdk::AgentWorkspaceReference>,
        trailing_space: bool,
    ) {
        let removed = query.token_end.saturating_sub(query.token_start);
        let inserted = replacement.chars().count() + usize::from(trailing_space);
        self.reconcile_composer_edit(query.token_start, removed, inserted);
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
        self.retain_valid_composer_sources();
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
    }

    fn reconcile_composer_edit(
        &mut self,
        edit_start: usize,
        removed_chars: usize,
        inserted_chars: usize,
    ) {
        let mut draft = ComposerDraft {
            text: String::new(),
            workspace_references: std::mem::take(&mut self.workspace_references),
            image_attachments: std::mem::take(&mut self.image_attachments),
        };
        draft.reconcile_edit(edit_start, removed_chars, inserted_chars);
        self.workspace_references = draft.workspace_references;
        self.image_attachments = draft.image_attachments;
    }

    fn retain_valid_composer_sources(&mut self) {
        let mut draft = ComposerDraft {
            text: self.text_input.input.clone(),
            workspace_references: std::mem::take(&mut self.workspace_references),
            image_attachments: std::mem::take(&mut self.image_attachments),
        };
        draft.retain_valid_sources();
        self.text_input
            .set_text_and_cursor(&draft.text, self.text_input.cursor);
        self.workspace_references = draft.workspace_references;
        self.image_attachments = draft.image_attachments;
    }

    fn safe_insertion_cursor(&self) -> usize {
        self.draft_snapshot()
            .safe_insertion_cursor(self.text_input.cursor)
    }

    fn apply_draft_at_cursor(&mut self, draft: ComposerDraft, cursor: usize) {
        self.text_input.set_text_and_cursor(&draft.text, cursor);
        self.workspace_references = draft.workspace_references;
        self.image_attachments = draft.image_attachments;
        self.workspace_reference_popup.hide();
        self.refresh_command_menu();
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
        let history = if self.is_shell_mode() {
            &self.shell_input_history
        } else {
            &self.input_history
        };
        if history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => 0,
            Some(i) if i + 1 < history.len() => i + 1,
            Some(i) => i,
        };

        if let Some(history_item) = history.get(new_index) {
            self.text_input.set_text(&history_item.text);
            self.workspace_references = history_item.workspace_references.clone();
            self.image_attachments = history_item.image_attachments.clone();
            self.history_index = Some(new_index);
            self.refresh_command_menu();
        }
    }

    pub(crate) fn history_next(&mut self) {
        let history = if self.is_shell_mode() {
            &self.shell_input_history
        } else {
            &self.input_history
        };
        match self.history_index {
            None => {}
            Some(0) => {
                self.text_input.clear();
                self.workspace_references.clear();
                self.image_attachments.clear();
                self.history_index = None;
                self.refresh_command_menu();
            }
            Some(i) => {
                let new_index = i - 1;
                if let Some(history_item) = history.get(new_index) {
                    self.text_input.set_text(&history_item.text);
                    self.workspace_references = history_item.workspace_references.clone();
                    self.image_attachments = history_item.image_attachments.clone();
                    self.history_index = Some(new_index);
                    self.refresh_command_menu();
                }
            }
        }
    }
}

#[cfg(test)]
mod composer_input_tests {
    use super::*;
    use std::sync::Arc;

    fn image(id: &str) -> crate::ui::composer::ComposerImage {
        crate::ui::composer::ComposerImage::new(
            id,
            format!("{id}.png"),
            "image/png",
            Arc::<[u8]>::from([1, 2, 3]),
        )
    }

    #[test]
    fn shell_mode_enters_only_from_an_empty_composer_and_keeps_the_marker_out_of_input() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());

        assert!(view.try_enter_shell_mode());
        assert!(view.is_shell_mode());
        assert_eq!(view.input_text(), "");
        assert!(!view.command_menu_visible());

        view.handle_char('/');
        let keymap =
            crate::actions::ResolvedKeymap::new(&crate::config::ShortcutsConfig::default());
        view.set_action_state(crate::actions::ActionState::chat(true, false), &keymap);
        assert!(!view.command_menu_visible());

        view.clear_input();
        view.exit_shell_mode();
        view.handle_char('x');
        assert!(!view.try_enter_shell_mode());
        assert!(!view.is_shell_mode());
        assert_eq!(view.input_text(), "x");
    }

    #[test]
    fn programmatic_chat_prefill_exits_shell_mode() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        assert!(view.try_enter_shell_mode());

        view.set_input("/rename ");

        assert!(!view.is_shell_mode());
        assert_eq!(view.input_text(), "/rename ");
    }

    #[test]
    fn restored_prompt_draft_exits_shell_mode() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        assert!(view.try_enter_shell_mode());

        view.set_chat_draft(ComposerDraft::from_text("restored prompt"));

        assert!(!view.is_shell_mode());
        assert_eq!(view.input_text(), "restored prompt");
    }

    #[test]
    fn switching_sessions_swaps_exact_inactive_composer_drafts() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        let first_image = image("first");
        let mut first = ComposerDraft::from_text("first draft");
        first
            .insert_image(first.text.chars().count(), first_image.clone())
            .unwrap();
        view.set_draft(first.clone());

        view.activate_session_composer("session-1", "session-2");
        assert_eq!(view.draft_snapshot(), ComposerDraft::default());
        assert!(!view.is_shell_mode());

        assert!(view.try_enter_shell_mode());
        view.handle_char('p');
        view.handle_char('w');
        view.handle_char('d');
        let second = view.draft_snapshot();

        view.activate_session_composer("session-2", "session-1");
        assert_eq!(view.draft_snapshot(), first);
        assert!(!view.is_shell_mode());

        view.activate_session_composer("session-1", "session-2");
        assert_eq!(view.draft_snapshot(), second);
        assert!(view.is_shell_mode());
    }

    #[test]
    fn activating_the_current_session_does_not_mutate_its_composer() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.set_input("keep me");

        view.activate_session_composer("session-1", "session-1");

        assert_eq!(view.input_text(), "keep me");
    }

    #[test]
    fn forgotten_session_composer_is_not_restored() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.set_input("discard me");
        view.activate_session_composer("session-1", "session-2");

        view.forget_session_composer("session-1");
        view.activate_session_composer("session-2", "session-1");

        assert_eq!(view.draft_snapshot(), ComposerDraft::default());
        assert!(!view.is_shell_mode());
    }

    #[test]
    fn inactive_session_images_are_preserved_when_the_local_budget_rejects_growth() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("first")).unwrap();
        view.activate_session_composer("session-1", "session-2");

        assert_eq!(
            view.insert_image_with_budget(image("second"), 5),
            Err(crate::ui::composer::ComposerImageInsertError::LocalDraftBudgetExceeded)
        );
        assert!(!view.draft_snapshot().has_images());

        view.activate_session_composer("session-2", "session-1");
        assert!(view.draft_snapshot().has_images());
    }

    #[test]
    fn shell_mode_uses_separate_history_and_restores_shell_undo_identity() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.handle_char('n');
        view.handle_char('o');
        view.handle_char('r');
        view.handle_char('m');
        view.handle_char('a');
        view.handle_char('l');
        assert_eq!(view.send_input().unwrap().text, "normal");

        assert!(view.try_enter_shell_mode());
        view.insert_paste("git status --short");
        let shell = view.send_input().unwrap();
        view.remember_submitted_shell_command("session-1", &shell);
        view.exit_shell_mode();

        assert!(view.try_enter_shell_mode());
        view.history_prev();
        assert_eq!(view.input_text(), "git status --short");

        view.exit_shell_mode();
        view.clear_input();
        view.history_prev();
        assert_eq!(view.input_text(), "normal");

        let restored =
            view.restore_undo_draft("session-1", "!git status --short".to_string(), Vec::new());
        assert!(view.is_shell_mode());
        assert_eq!(restored.text, "git status --short");
    }

    #[test]
    fn local_input_history_restores_image_bytes_without_copying_file_paths() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("history-image")).unwrap();
        let sent = view.send_input().unwrap();

        view.history_prev();
        let restored = view.draft_snapshot();

        assert_eq!(restored.text, sent.text);
        assert_eq!(restored.image_attachments, sent.image_attachments);
        assert_eq!(
            restored.runtime_attachments()[0].metadata["name"],
            "history-image.png"
        );
    }

    #[test]
    fn session_revert_reuses_local_image_history_when_the_prompt_matches() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("history-image")).unwrap();
        let sent = view.send_input().unwrap();
        view.remember_submitted_draft("session-1", &sent);

        let restored = view.restore_undo_draft("session-1", sent.text.clone(), Vec::new());

        assert_eq!(restored.image_attachments, sent.image_attachments);
    }

    #[test]
    fn session_revert_uses_only_the_latest_successful_draft_for_that_session() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("first")).unwrap();
        let first = view.send_input().unwrap();
        view.insert_image(image("second")).unwrap();
        let second = view.send_input().unwrap();
        assert_eq!(first.text, second.text);
        view.remember_submitted_draft("session-1", &second);

        let restored = view.restore_undo_draft("session-1", first.text, Vec::new());

        assert_eq!(restored.image_attachments, second.image_attachments);
        assert_eq!(restored.image_attachments[0].image.id, "second");
    }

    #[test]
    fn session_revert_does_not_reuse_history_without_a_matching_successful_submission() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("history-only")).unwrap();
        let history_only = view.send_input().unwrap();

        let wrong_session =
            view.restore_undo_draft("session-2", history_only.text.clone(), Vec::new());
        let not_submitted = view.restore_undo_draft("session-1", history_only.text, Vec::new());

        assert!(wrong_session.image_attachments.is_empty());
        assert!(not_submitted.image_attachments.is_empty());
    }

    #[test]
    fn submitted_image_history_is_isolated_across_session_switches() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        let mut session_a = ComposerDraft::default();
        session_a.insert_image(0, image("session-a")).unwrap();
        let mut session_b = ComposerDraft::default();
        session_b.insert_image(0, image("session-b")).unwrap();

        view.remember_submitted_draft("session-a", &session_a);
        view.remember_submitted_draft("session-b", &session_b);

        let restored_a = view.restore_undo_draft("session-a", session_a.text.clone(), Vec::new());
        let restored_b = view.restore_undo_draft("session-b", session_b.text.clone(), Vec::new());
        assert_eq!(restored_a.image_attachments[0].image.id, "session-a");
        assert_eq!(restored_b.image_attachments[0].image.id, "session-b");
    }

    #[test]
    fn startup_submission_retains_images_for_undo_without_a_second_history_writer() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        let mut startup = ComposerDraft::default();
        startup.insert_image(0, image("startup")).unwrap();

        view.remember_submitted_draft("session-1", &startup);
        let restored = view.restore_undo_draft("session-1", startup.text.clone(), Vec::new());

        assert_eq!(restored, startup);
        assert!(view.input_history.is_empty());
    }

    #[test]
    fn successful_submission_history_has_one_global_record_bound() {
        let mut history = SubmittedDraftHistory::default();
        for index in 0..=MAX_LOCAL_DRAFT_HISTORY {
            history.record(
                &format!("session-{index}"),
                ComposerDraft::from_text(format!("draft-{index}")),
            );
        }

        assert_eq!(history.record_count(), MAX_LOCAL_DRAFT_HISTORY);
        assert!(history.undo("session-0", "draft-0").is_none());
        assert_eq!(
            history.undo(
                &format!("session-{MAX_LOCAL_DRAFT_HISTORY}"),
                &format!("draft-{MAX_LOCAL_DRAFT_HISTORY}")
            ),
            Some(ComposerDraft::from_text(format!(
                "draft-{MAX_LOCAL_DRAFT_HISTORY}"
            )))
        );
    }

    #[test]
    fn evicting_one_undone_record_invalidates_that_sessions_redo_metadata() {
        let mut history = SubmittedDraftHistory::default();
        let mut first = ComposerDraft::default();
        first.insert_image(0, image("first")).unwrap();
        let mut second = ComposerDraft::default();
        second.insert_image(0, image("second")).unwrap();
        assert_eq!(first.text, second.text);
        history.record("session-a", first.clone());
        history.record("session-a", second.clone());
        assert_eq!(history.undo("session-a", &second.text), Some(second));
        assert_eq!(history.undo("session-a", &first.text), Some(first));

        for index in 0..MAX_LOCAL_DRAFT_HISTORY - 1 {
            history.record(
                &format!("other-{index}"),
                ComposerDraft::from_text(format!("other-{index}")),
            );
        }

        history.redo("session-a");
        assert!(history.undo("session-a", "[Image 1] ").is_none());
        assert!(!history.sessions.contains_key("session-a"));
    }

    #[test]
    fn unknown_session_revert_does_not_allocate_empty_history() {
        let mut history = SubmittedDraftHistory::default();

        assert!(history.undo("unknown", "text").is_none());
        history.redo("unknown");

        assert!(history.sessions.is_empty());
    }

    #[test]
    fn consecutive_undo_and_redo_follow_the_successful_submission_stack() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("first")).unwrap();
        let first = view.send_input().unwrap();
        view.remember_submitted_draft("session-1", &first);
        view.insert_image(image("second")).unwrap();
        let second = view.send_input().unwrap();
        view.remember_submitted_draft("session-1", &second);

        let undo_second = view.restore_undo_draft("session-1", second.text.clone(), Vec::new());
        let undo_first = view.restore_undo_draft("session-1", first.text.clone(), Vec::new());
        assert_eq!(undo_second.image_attachments[0].image.id, "second");
        assert_eq!(undo_first.image_attachments[0].image.id, "first");

        view.note_session_redo("session-1");
        let undo_first_again = view.restore_undo_draft("session-1", first.text, Vec::new());
        assert_eq!(undo_first_again.image_attachments[0].image.id, "first");
    }

    #[test]
    fn image_history_budget_drops_old_metadata_without_copying_or_removing_text() {
        let mut view = ChatView::new(Theme::dark(), Vec::new());
        view.insert_image(image("bounded")).unwrap();
        let sent = view.send_input().unwrap();
        view.remember_submitted_draft("session-1", &sent);

        view.enforce_draft_history_image_budget_with_limit(5);
        view.history_prev();

        assert_eq!(view.input_text(), sent.text);
        assert!(!view.draft_snapshot().has_images());
        let undo = view.restore_undo_draft("session-1", sent.text, Vec::new());
        assert!(undo.has_images());
    }
}
