impl ChatView {
    pub fn clear_screen(&mut self) {
        self.list_state.select(None);
        self.auto_scroll = true;
        self.collapsed_tools.clear();
        self.focused_block_tool = None;
        self.collapsed_thinking.clear();
        self.thinking_auto_collapsed.clear();
        self.thinking_user_overrides.clear();
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

    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    pub fn begin_theme_preview(&mut self) {
        if self.theme_preview_original.is_none() {
            self.theme_preview_original = Some(self.theme.clone());
        }
    }

    pub fn cancel_theme_preview(&mut self) {
        if let Some(original) = self.theme_preview_original.take() {
            self.set_theme(original);
        }
        self.pending_theme_preview = None;
    }

    pub fn commit_theme_preview(&mut self) {
        self.theme_preview_original = None;
        self.pending_theme_preview = None;
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme.clone();
        self.markdown_renderer = MarkdownRenderer::new(theme);
        self.lines_cache_dirty = true;
        self.render_cache.clear();
    }

    pub fn toggle_browse_mode(&mut self) {
        self.browse_mode = !self.browse_mode;
        if self.browse_mode {
            self.auto_scroll = false;
        } else {
            self.auto_scroll = true;
            self.scroll_offset = 0;
        }
    }

    pub fn scroll_up(&mut self, lines: usize, total_message_lines: usize) {
        if self.browse_mode {
            self.scroll_offset = (self.scroll_offset + lines).min(total_message_lines.saturating_sub(1));
        } else {
            self.browse_mode = true;
            self.auto_scroll = false;
            self.scroll_offset = lines;
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        if self.scroll_offset > 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(lines);

            if self.scroll_offset == 0 && self.browse_mode {
                self.browse_mode = false;
                self.auto_scroll = true;
            }
        }
    }

    pub fn scroll_to_top(&mut self, total_message_lines: usize) {
        self.browse_mode = true;
        self.auto_scroll = false;
        self.scroll_offset = total_message_lines.saturating_sub(1);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.browse_mode = false;
        self.auto_scroll = true;
        self.scroll_offset = 0;
    }

    /// Count total rendered lines for all messages (used for scroll calculations).
    /// Uses cached value when possible to avoid O(N) full re-render on every scroll.
    pub fn count_message_lines(&mut self, chat_state: &ChatState) -> usize {
        let width = self.messages_area.map(|a| a.width).unwrap_or(80);

        // Return cached value if still valid (set by render_messages each frame)
        if !self.lines_cache_dirty
            && self.cached_msg_count == chat_state.messages.len()
            && self.cached_width == width
            && self.cached_total_lines > 0
        {
            return self.cached_total_lines;
        }

        // Try to compute from per-message render cache (avoids full re-render)
        let mut total = 0;
        let mut all_cached = true;
        for msg in &chat_state.messages {
            if let Some(entry) = self.render_cache.get(&msg.id) {
                if entry.version == msg.version && entry.width == width {
                    total += entry.line_count;
                    continue;
                }
            }
            all_cached = false;
            break;
        }

        if all_cached {
            return total;
        }

        // Full fallback: render all messages to count lines
        let mut total = 0;
        for msg in &chat_state.messages {
            total += self.render_message(msg, width).items.len();
        }
        total
    }

    /// Mark the line count cache as dirty (call when streaming content changes)
    pub fn invalidate_lines_cache(&mut self) {
        self.lines_cache_dirty = true;
    }

    fn invalidate_render_cache(&mut self) {
        self.render_cache.clear();
        self.lines_cache_dirty = true;
    }
}

