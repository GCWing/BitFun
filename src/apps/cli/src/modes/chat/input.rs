impl ChatMode {
    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return Ok(None);
        }

        let modal_state =
            ActionState::chat(chat_state.is_processing, self.any_popup_visible(chat_view));
        if let Some(action) = self.keymap.resolve_modal_safe(key, modal_state) {
            return self.dispatch_action(action, modal_state, chat_view, chat_state, rt_handle);
        }

        if let Some(ref mut prompt) = chat_state.permission_prompt {
            match prompt.handle_key_event(key) {
                PermissionAction::Reply(reply) => {
                    let request_id = prompt.request.request_id.clone();
                    let runtime = self.runtime.agent_runtime().clone();
                    let result = tokio::task::block_in_place(|| {
                        rt_handle.block_on(runtime.respond_permission(&request_id, reply))
                    });
                    match result {
                        Ok(()) => {
                            chat_state.resolve_permission_request(&request_id);
                            chat_view.set_status(Some("Permission response sent".to_string()));
                        }
                        Err(error) => {
                            tracing::error!("Failed to respond to permission request: {error}");
                            chat_view.set_status(Some(format!("Error: {error}")));
                        }
                    }
                }
                PermissionAction::None => {}
            }
            return Ok(None);
        }

        // ── Question prompt intercepts all keys when active ──
        if let Some(ref mut prompt) = chat_state.question_prompt {
            let action = prompt.handle_key_event(key);
            match action {
                QuestionAction::Submit(answers) => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    chat_state.question_prompt = None;
                    tracing::info!("User submitted answers for tool: {}", tool_id);
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.submit_user_answers(&tool_id, answers).await {
                                tracing::error!("Failed to submit answers: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some("Answers submitted".to_string()));
                }
                QuestionAction::Reject => {
                    let tool_id = prompt.tool_id.clone();
                    chat_state.question_prompt = None;
                    tracing::info!("User dismissed question prompt: {}", tool_id);
                    chat_view.set_status(Some("Question dismissed".to_string()));
                }
                QuestionAction::None => {
                    // Question prompt consumed the key, no further action
                }
            }
            return Ok(None);
        }

        // ── Normal key handling ──

        // Host recovery keys win over configured actions while a popup is open.
        if self.any_popup_visible(chat_view) {
            let state = ActionState::chat(chat_state.is_processing, true);
            if let Some(action) = self.keymap.resolve_reserved(key, state) {
                return self.dispatch_action(action, state, chat_view, chat_state, rt_handle);
            }
        }

        // Info popup intercepts all keys when visible
        if chat_view.info_popup_visible() {
            match key.code {
                KeyCode::Up => chat_view.info_popup_scroll_up(1),
                KeyCode::Down => chat_view.info_popup_scroll_down(1),
                KeyCode::PageUp => chat_view.info_popup_scroll_up(10),
                KeyCode::PageDown => chat_view.info_popup_scroll_down(10),
                KeyCode::Home => chat_view.info_popup_scroll_to_start(),
                KeyCode::End => chat_view.info_popup_scroll_to_end(),
                KeyCode::Esc => chat_view.dismiss_info_popup(),
                _ => {}
            }
            return Ok(None);
        }

        // Command palette intercepts all keys when visible
        if chat_view.command_palette_visible() {
            let action = chat_view.command_palette_handle_key(key);
            match action {
                PaletteAction::Execute(id) => {
                    return self.handle_palette_action(&id, chat_view, chat_state, rt_handle);
                }
                PaletteAction::Dismiss => self.navigate_back(chat_view),
                PaletteAction::None => {}
            }
            return Ok(None);
        }

        // Handle popup events first (when visible)
        if chat_view.model_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.model_selector_up(),
                KeyCode::Down => chat_view.model_selector_down(),
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.model_selector_confirm() {
                        chat_view.hide_model_selector();
                        self.apply_model_selection(&selected, chat_view, chat_state, rt_handle);
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(selected) = chat_view.model_selector_confirm() {
                        chat_view.hide_model_selector();
                        self.edit_model(&selected, chat_view, rt_handle);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.theme_selector_visible() {
            match key.code {
                KeyCode::Up => {
                    chat_view.theme_selector_up();
                    if let Some(selected) = chat_view.theme_selector_selected() {
                        self.preview_theme_selection(&selected, chat_view);
                    }
                }
                KeyCode::Down => {
                    chat_view.theme_selector_down();
                    if let Some(selected) = chat_view.theme_selector_selected() {
                        self.preview_theme_selection(&selected, chat_view);
                    }
                }
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.theme_selector_confirm() {
                        chat_view.hide_theme_selector();
                        self.apply_theme_selection(&selected, chat_view);
                        chat_view.commit_theme_preview();
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.agent_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.agent_selector_up(),
                KeyCode::Down => chat_view.agent_selector_down(),
                KeyCode::Enter => {
                    if let Some(action) = chat_view.agent_selector_confirm() {
                        self.handle_agent_selector_action(action, chat_view, chat_state, rt_handle);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.session_selector_visible() {
            let action = chat_view.session_selector_handle_key(key);
            match action {
                SessionAction::Switch(item) => {
                    return Ok(Some(ChatExitReason::SwitchSession(item.session_id)));
                }
                SessionAction::Delete(item) => {
                    self.handle_session_delete(&item, chat_view, chat_state, rt_handle);
                }
                SessionAction::Close | SessionAction::None => {}
            }
            return Ok(None);
        }

        if chat_view.skill_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.skill_selector_up(),
                KeyCode::Down => chat_view.skill_selector_down(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = chat_view.skill_selector_confirm() {
                        self.handle_skill_selector_action(action, chat_view, chat_state, rt_handle);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.subagent_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.subagent_selector_up(),
                KeyCode::Down => chat_view.subagent_selector_down(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = chat_view.subagent_selector_confirm() {
                        self.handle_subagent_selector_action(
                            action, chat_view, chat_state, rt_handle,
                        );
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.mcp_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.mcp_selector_up(),
                KeyCode::Down => chat_view.mcp_selector_down(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(selected) = chat_view.mcp_selector_confirm() {
                        if selected.requires_external_confirmation()
                            && !chat_view.mcp_selector_is_confirm_external(&selected.id)
                        {
                            chat_view.mcp_selector_start_confirm_external(selected.id.clone());
                        } else {
                            chat_view.mcp_selector_cancel_confirm_external();
                            self.activate_mcp_item(selected, chat_view, chat_state);
                        }
                    }
                }
                KeyCode::Char('a') => {
                    // Open add dialog (hide selector first)
                    chat_view.hide_mcp_selector();
                    chat_view.show_mcp_add_dialog();
                }
                KeyCode::Char('d') => {
                    if let Some(selected) = chat_view.mcp_selector_confirm() {
                        if selected.is_external() {
                            chat_state.add_system_message(
                                "External MCP settings are read-only in BitFun. Disable the server here or edit it in the source application."
                                    .to_string(),
                            );
                            return Ok(None);
                        }
                        // First press: enter confirm-delete mode
                        // Second press: actually delete (handled by confirm_delete state)
                        if chat_view.mcp_selector_is_confirm_delete(&selected.id) {
                            self.delete_mcp_server(&selected.id, chat_view);
                        } else {
                            chat_view.mcp_selector_start_confirm_delete(selected.id.clone());
                        }
                    }
                }
                KeyCode::Char('e') => {
                    if chat_view
                        .mcp_selector_confirm()
                        .is_some_and(|selected| selected.is_external())
                    {
                        chat_state.add_system_message(
                            "External MCP settings are read-only in BitFun. Edit them in the source application."
                                .to_string(),
                        );
                    } else {
                        chat_view.hide_mcp_selector();
                        self.open_mcp_config(chat_state);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {
                    // Any other key cancels the confirm-delete state
                    chat_view.mcp_selector_cancel_confirm_delete();
                    chat_view.mcp_selector_cancel_confirm_external();
                }
            }
            return Ok(None);
        }

        if chat_view.mcp_add_dialog_visible() {
            let action = chat_view.mcp_add_dialog_handle_key(key);
            match action {
                McpAddAction::Confirm { name, config_json } => {
                    self.add_mcp_server(&name, &config_json, chat_view);
                }
                McpAddAction::Cancel => {
                    // Re-open the MCP selector
                    self.show_mcp_selector(chat_view, chat_state, rt_handle);
                }
                McpAddAction::None => {}
            }
            return Ok(None);
        }

        if chat_view.provider_selector_visible() {
            if let Some(selection) = chat_view.provider_selector_handle_key(key) {
                self.handle_provider_selection(selection, chat_view);
            }
            return Ok(None);
        }

        if chat_view.model_config_form_visible() {
            let action = chat_view.model_config_form_handle_key(key);
            match action {
                ModelFormAction::Save(result) => {
                    if result.editing_model_id.is_some() {
                        self.update_existing_model(result, chat_view, chat_state, rt_handle);
                    } else {
                        self.save_new_model(result, chat_view, chat_state, rt_handle);
                    }
                }
                ModelFormAction::Cancel => {
                    chat_view.set_status(Some("Model form cancelled".to_string()));
                }
                ModelFormAction::None => {}
            }
            return Ok(None);
        }

        if chat_view.login_form_visible() {
            self.refresh_account_panel_live(chat_view);
            let action = chat_view.login_form_handle_key(key);
            return self.handle_login_form_action(action, chat_view, chat_state, rt_handle);
        }

        if let Some(action) = self
            .keymap
            .resolve(key, ActionState::chat(chat_state.is_processing, false))
        {
            return self.dispatch_action(
                action,
                ActionState::chat(chat_state.is_processing, false),
                chat_view,
                chat_state,
                rt_handle,
            );
        }

        match (key.code, key.modifiers) {
            (KeyCode::Backspace, _) => {
                chat_view.handle_backspace();
            }

            (KeyCode::Left, _) => {
                chat_view.move_cursor_left();
            }
            (KeyCode::Right, _) => {
                chat_view.move_cursor_right();
            }

            (KeyCode::Home, _) => {
                chat_view.set_cursor_home();
            }

            (KeyCode::End, _) => {
                chat_view.set_cursor_end();
            }

            (KeyCode::Esc, _) => {
                if chat_view.browse_mode {
                    chat_view.scroll_to_bottom();
                    chat_view.set_status(Some("Exited browse mode".to_string()));
                }
            }

            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT)
                if !c.is_control() && c != '\u{0}' =>
            {
                chat_view.handle_char(c);
            }

            _ => {}
        }

        Ok(None)
    }

    /// Apply an exit reason from handle_key_event (shared by normal and batch paths).
    fn apply_exit_reason(reason: ChatExitReason, context: ChatEventContext<'_>) {
        let ChatEventContext {
            this,
            chat_view,
            chat_state,
            session_id,
            rt_handle,
            should_quit,
            exit_reason,
        } = context;
        match reason {
            ChatExitReason::SwitchSession(new_session_id) => {
                if let Some(pending) = this.pending_mode_change.as_mut() {
                    pending.exit_warning_shown = false;
                }
                match this.switch_to_session(
                    &new_session_id,
                    session_id,
                    chat_state,
                    chat_view,
                    rt_handle,
                ) {
                    Ok(()) => tracing::info!("Switched to session: {}", new_session_id),
                    Err(e) => {
                        chat_state.add_system_message(format!("Failed to switch session: {}", e));
                        tracing::error!("Failed to switch session: {}", e);
                    }
                }
            }
            ChatExitReason::NewSession => {
                if let Some(pending) = this.pending_mode_change.as_mut() {
                    pending.exit_warning_shown = false;
                }
                match this.create_new_session(session_id, chat_state, chat_view, rt_handle) {
                    Ok(()) => tracing::info!("Created new session: {}", session_id),
                    Err(e) => {
                        chat_state
                            .add_system_message(format!("Failed to create new session: {}", e));
                        tracing::error!("Failed to create new session: {}", e);
                    }
                }
            }
            ChatExitReason::Quit => {
                if let Some(pending) = this.pending_mode_change.as_mut() {
                    if !pending.exit_warning_shown {
                        pending.exit_warning_shown = true;
                        chat_view.set_status(Some(
                            "Exit requested. Waiting for the agent mode change to finish; exit again to leave now. This mode change may not be saved, and the next restore will use the last successfully persisted mode."
                                .to_string(),
                        ));
                        return;
                    }
                }
                *should_quit = true;
                *exit_reason = ChatExitReason::Quit;
            }
        }
    }

    /// Handle non-key events (Mouse, Paste, Resize, etc.).
    fn handle_non_key_event(
        event: Event,
        context: ChatEventContext<'_>,
    ) -> Result<NonKeyEventOutcome> {
        let mut outcome = NonKeyEventOutcome::default();
        match event {
            Event::Mouse(mouse) => {
                if context.chat_view.command_palette_captures_mouse(&mouse) {
                    let action = context.chat_view.command_palette_handle_mouse(&mouse);
                    match action {
                        PaletteAction::Execute(id) => {
                            if let Some(reason) = context.this.handle_palette_action(
                                &id,
                                context.chat_view,
                                context.chat_state,
                                context.rt_handle,
                            )? {
                                Self::apply_exit_reason(
                                    reason,
                                    ChatEventContext {
                                        this: &mut *context.this,
                                        chat_view: &mut *context.chat_view,
                                        chat_state: &mut *context.chat_state,
                                        session_id: &mut *context.session_id,
                                        rt_handle: context.rt_handle,
                                        should_quit: &mut *context.should_quit,
                                        exit_reason: &mut *context.exit_reason,
                                    },
                                );
                            }
                        }
                        PaletteAction::Dismiss => context.this.navigate_back(context.chat_view),
                        PaletteAction::None => {}
                    }
                } else if context.chat_view.provider_selector_captures_mouse(&mouse) {
                    if let Some(selection) =
                        context.chat_view.provider_selector_handle_mouse(&mouse)
                    {
                        context
                            .this
                            .handle_provider_selection(selection, context.chat_view);
                    }
                } else if context.chat_view.handle_mouse_event(&mouse) {
                    if let Some(action) = context.chat_view.take_pending_agent_action() {
                        context.this.handle_agent_selector_action(
                            action,
                            context.chat_view,
                            context.chat_state,
                            context.rt_handle,
                        );
                    }
                    if let Some(action) = context.chat_view.take_pending_skill_action() {
                        context.this.handle_skill_selector_action(
                            action,
                            context.chat_view,
                            context.chat_state,
                            context.rt_handle,
                        );
                    }
                    if let Some(action) = context.chat_view.take_pending_subagent_action() {
                        context.this.handle_subagent_selector_action(
                            action,
                            context.chat_view,
                            context.chat_state,
                            context.rt_handle,
                        );
                    }
                } else {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            let total = context.chat_view.count_message_lines(context.chat_state);
                            context.chat_view.scroll_up(3, total);
                        }
                        MouseEventKind::ScrollDown => {
                            context.chat_view.scroll_down(3);
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            let _ = context
                                .chat_view
                                .begin_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            let _ = context
                                .chat_view
                                .update_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            match context
                                .chat_view
                                .complete_mouse_selection_or_click(mouse.column, mouse.row)
                            {
                                MouseGestureOutcome::CopyText(text) => {
                                    match Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                        Ok(()) => context
                                            .chat_view
                                            .set_status(Some("Copied to clipboard".to_string())),
                                        Err(_) => context.chat_view.set_status(Some(
                                            "Failed to copy selection".to_string(),
                                        )),
                                    }
                                }
                                MouseGestureOutcome::Click(col, row) => {
                                    context.chat_view.handle_mouse_click(col, row);
                                }
                                MouseGestureOutcome::None => {}
                            }
                        }
                        MouseEventKind::Moved
                            if !context
                                .chat_view
                                .update_mouse_selection(mouse.column, mouse.row) =>
                        {
                            context.chat_view.handle_mouse_move(mouse.column, mouse.row);
                        }
                        _ => {}
                    }
                }
                if let Some(action_id) = context.chat_view.take_pending_command() {
                    if let Some(reason) = context.this.handle_action_id(
                        &action_id,
                        context.chat_view,
                        context.chat_state,
                        context.rt_handle,
                    )? {
                        Self::apply_exit_reason(
                            reason,
                            ChatEventContext {
                                this: &mut *context.this,
                                chat_view: &mut *context.chat_view,
                                chat_state: &mut *context.chat_state,
                                session_id: &mut *context.session_id,
                                rt_handle: context.rt_handle,
                                should_quit: &mut *context.should_quit,
                                exit_reason: &mut *context.exit_reason,
                            },
                        );
                    }
                }
                if let Some(theme) = context.chat_view.take_pending_theme_preview() {
                    context
                        .this
                        .preview_theme_selection(&theme, context.chat_view);
                }
                if let Some(item) = context.chat_view.take_pending_mcp_toggle() {
                    context
                        .this
                        .activate_mcp_item(item, context.chat_view, context.chat_state);
                }
                outcome.request_redraw = true;
            }
            Event::Paste(text) => {
                if context.chat_view.mcp_add_dialog_visible() {
                    context.chat_view.mcp_add_dialog_handle_paste(&text);
                } else if context.chat_view.login_form_visible() {
                    context.chat_view.login_form_insert_paste(&text);
                } else if context.chat_state.permission_prompt.is_none()
                    && context.chat_state.question_prompt.is_none()
                    && !context.this.any_popup_visible(context.chat_view)
                {
                    context.chat_view.insert_paste(&text);
                }
                outcome.request_redraw = true;
            }
            Event::Resize(_, _) => {
                outcome.resize_observed = true;
            }
            _ => {}
        }
        Ok(outcome)
    }
}
