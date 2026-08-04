struct MessageLayout {
    prefix_sum: Vec<usize>,
    total_lines: usize,
}

fn scroll_offset_for_message(
    prefix_sum: &[usize],
    message_index: usize,
    viewport_lines: usize,
) -> usize {
    let total_lines = prefix_sum.last().copied().unwrap_or_default();
    let target_start = prefix_sum
        .get(message_index)
        .copied()
        .unwrap_or(total_lines);
    total_lines.saturating_sub(target_start.saturating_add(viewport_lines))
}

impl ChatView {
    pub(crate) fn clear_screen(&mut self) {
        self.list_state.select(None);
        self.auto_scroll = true;
        self.committed_message_anchor = None;
        self.tool_disclosures.clear();
        self.focused_block_tool = None;
        self.thinking_disclosures.clear();
        self.block_tool_regions.clear();
        self.thinking_regions.clear();
        self.visible_plain_lines.clear();
        self.selection_anchor = None;
        self.selection_focus = None;
        self.selection_mouse_down = None;
        self.selection_dragged = false;
        self.lines_cache_dirty = true;
        self.cached_total_lines = 0;
        self.cached_msg_count = 0;
        self.markdown_renderer.clear_cache();
        self.render_cache.clear();
        crate::ui::tool_cards::clear_tool_card_cache();
    }

    pub(crate) fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    pub(crate) fn begin_theme_preview(&mut self) {
        if self.theme_preview_original.is_none() {
            self.theme_preview_original = Some(self.theme.clone());
        }
    }

    pub(crate) fn cancel_theme_preview(&mut self) {
        if let Some(original) = self.theme_preview_original.take() {
            self.set_theme(original);
        }
        self.pending_theme_preview = None;
    }

    pub(crate) fn commit_theme_preview(&mut self) {
        self.theme_preview_original = None;
        self.pending_theme_preview = None;
    }

    pub(crate) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme.clone();
        self.markdown_renderer = MarkdownRenderer::new(theme);
        self.lines_cache_dirty = true;
        self.render_cache.clear();
    }

    pub(crate) fn toggle_browse_mode(&mut self) {
        self.committed_message_anchor = None;
        self.browse_mode = !self.browse_mode;
        if self.browse_mode {
            self.auto_scroll = false;
        } else {
            self.auto_scroll = true;
            self.scroll_offset = 0;
        }
    }

    pub(crate) fn scroll_up(&mut self, lines: usize, total_message_lines: usize) {
        self.committed_message_anchor = None;
        let visible = self
            .messages_area
            .map(|a| a.height.max(1) as usize)
            .unwrap_or(20);
        let max_offset = total_message_lines.saturating_sub(visible);
        if self.browse_mode {
            self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
        } else {
            self.browse_mode = true;
            self.auto_scroll = false;
            self.scroll_offset = lines.min(max_offset);
        }
    }

    pub(crate) fn scroll_down(&mut self, lines: usize) {
        self.committed_message_anchor = None;
        if self.scroll_offset > 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(lines);

            if self.scroll_offset == 0 && self.browse_mode {
                self.browse_mode = false;
                self.auto_scroll = true;
            }
        }
    }

    pub(crate) fn scroll_to_top(&mut self, total_message_lines: usize) {
        self.committed_message_anchor = None;
        self.browse_mode = true;
        self.auto_scroll = false;
        let visible = self
            .messages_area
            .map(|a| a.height.max(1) as usize)
            .unwrap_or(20);
        self.scroll_offset = total_message_lines.saturating_sub(visible);
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        self.committed_message_anchor = None;
        self.browse_mode = false;
        self.auto_scroll = true;
        self.scroll_offset = 0;
    }

    fn ensure_message_layout(&mut self, chat_state: &ChatState, width: u16) -> MessageLayout {
        let mut prefix_sum = Vec::with_capacity(chat_state.messages.len() + 1);
        prefix_sum.push(0usize);

        for message in &chat_state.messages {
            let cache_valid = self
                .render_cache
                .get(&message.id)
                .map(|entry| entry.version == message.version && entry.width == width)
                .unwrap_or(false);
            if message.is_streaming || !cache_valid {
                let rendered = self.render_message(message, width);
                let line_count = rendered.items.len();
                self.render_cache.insert(
                    message.id.clone(),
                    MessageRenderEntry {
                        items: rendered.items,
                        line_count,
                        version: message.version,
                        width,
                        plain_lines: rendered.plain_lines,
                        tool_regions: rendered.tool_regions,
                        thinking_regions: rendered.thinking_regions,
                    },
                );
            }
            let line_count = self
                .render_cache
                .get(&message.id)
                .map(|entry| entry.line_count)
                .unwrap_or_default();
            prefix_sum.push(prefix_sum.last().copied().unwrap_or_default() + line_count);
        }

        let total_lines = prefix_sum.last().copied().unwrap_or_default();
        self.cached_total_lines = total_lines;
        self.cached_msg_count = chat_state.messages.len();
        self.cached_width = width;
        self.lines_cache_dirty = false;
        MessageLayout {
            prefix_sum,
            total_lines,
        }
    }

    pub(crate) fn scroll_to_message(&mut self, chat_state: &ChatState, message_id: &str) -> bool {
        let Some(message_index) = chat_state
            .messages
            .iter()
            .position(|message| message.id == message_id)
        else {
            return false;
        };
        let (width, viewport_lines) = self
            .messages_area
            .map(|area| (area.width, area.height.max(1) as usize))
            .unwrap_or((80, 20));
        let layout = self.ensure_message_layout(chat_state, width);
        self.apply_message_jump(&layout.prefix_sum, message_index, viewport_lines);
        true
    }

    pub(crate) fn commit_message_jump(&mut self, chat_state: &ChatState, message_id: &str) -> bool {
        if !self.scroll_to_message(chat_state, message_id) {
            return false;
        }
        self.committed_message_anchor = Some(message_id.to_string());
        true
    }

    pub(crate) fn clear_committed_message_anchor(&mut self) {
        self.committed_message_anchor = None;
    }

    fn maintain_committed_message_anchor(
        &mut self,
        chat_state: &ChatState,
        layout: &MessageLayout,
        viewport_lines: usize,
    ) {
        let Some(message_id) = self.committed_message_anchor.clone() else {
            return;
        };
        let Some(message_index) = chat_state
            .messages
            .iter()
            .position(|message| message.id == message_id)
        else {
            self.committed_message_anchor = None;
            return;
        };
        self.apply_message_jump(&layout.prefix_sum, message_index, viewport_lines);
    }

    fn apply_message_jump(
        &mut self,
        prefix_sum: &[usize],
        message_index: usize,
        viewport_lines: usize,
    ) {
        self.scroll_offset = scroll_offset_for_message(prefix_sum, message_index, viewport_lines);
        self.browse_mode = self.scroll_offset > 0;
        self.auto_scroll = !self.browse_mode;
    }

    /// Count total rendered lines for all messages (used for scroll calculations).
    /// Uses cached value when possible to avoid O(N) full re-render on every scroll.
    pub(crate) fn count_message_lines(&mut self, chat_state: &ChatState) -> usize {
        let width = self.messages_area.map(|a| a.width).unwrap_or(80);

        // Return cached value if still valid (set by render_messages each frame)
        if !self.lines_cache_dirty
            && self.cached_msg_count == chat_state.messages.len()
            && self.cached_width == width
            && self.cached_total_lines > 0
        {
            return self.cached_total_lines;
        }

        self.ensure_message_layout(chat_state, width).total_lines
    }

    /// Mark the line count cache as dirty (call when streaming content changes)
    pub(crate) fn invalidate_lines_cache(&mut self) {
        self.lines_cache_dirty = true;
    }

    fn invalidate_render_cache(&mut self) {
        self.render_cache.clear();
        self.lines_cache_dirty = true;
    }
}

#[cfg(test)]
mod transcript_navigation_tests {
    use super::{scroll_offset_for_message, ChatView, MessageLayout};
    use crate::chat_state::{ChatMessage, ChatState, FlowItem, MessageRole};
    use crate::ui::theme::Theme;

    fn text_message(id: &str, role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            turn_id: Some(format!("turn-{id}")),
            role,
            timestamp: std::time::SystemTime::now(),
            flow_items: vec![FlowItem::Text {
                content: content.to_string(),
                is_streaming: false,
            }],
            is_streaming: false,
            version: 0,
        }
    }

    #[test]
    fn message_jump_places_each_target_at_the_viewport_start_when_space_allows() {
        let prefix_sum = vec![0, 10, 30, 60, 100];

        assert_eq!(scroll_offset_for_message(&prefix_sum, 0, 20), 80);
        assert_eq!(scroll_offset_for_message(&prefix_sum, 2, 20), 50);
        assert_eq!(scroll_offset_for_message(&prefix_sum, 3, 20), 20);

        let after_streaming_growth = vec![0, 10, 30, 60, 110];
        assert_eq!(
            scroll_offset_for_message(&after_streaming_growth, 0, 20),
            90
        );
    }

    #[test]
    fn committed_timeline_jump_tracks_streaming_growth_until_manual_scroll() {
        let mut chat_state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            None,
        );
        chat_state.messages = vec![
            text_message("target", MessageRole::User, "target"),
            text_message("stream", MessageRole::Assistant, "streaming"),
        ];
        let mut view = ChatView::new(Theme::dark(), Vec::new());

        assert!(view.commit_message_jump(&chat_state, "target"));
        assert_eq!(view.committed_message_anchor.as_deref(), Some("target"));

        view.maintain_committed_message_anchor(
            &chat_state,
            &MessageLayout {
                prefix_sum: vec![0, 30, 100],
                total_lines: 100,
            },
            20,
        );
        assert_eq!(view.scroll_offset, 80);

        view.maintain_committed_message_anchor(
            &chat_state,
            &MessageLayout {
                prefix_sum: vec![0, 30, 110],
                total_lines: 110,
            },
            20,
        );
        assert_eq!(view.scroll_offset, 90);

        view.scroll_down(1);
        assert_eq!(view.committed_message_anchor, None);
    }
}
