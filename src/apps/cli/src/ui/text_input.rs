use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Reusable multiline text input component with wrap support.
///
/// Manages input buffer, cursor, scroll offset, and provides rendering
/// with automatic line wrapping for text that exceeds the available width.
pub struct TextInput {
    pub input: String,
    pub cursor: usize,
    pub scroll_offset: usize,
}

/// Style configuration for rendering a TextInput.
pub struct TextInputStyle {
    pub first_line_prefix: &'static str,
    pub continuation_prefix: &'static str,
    pub placeholder: String,
    pub text_style: ratatui::style::Style,
    pub placeholder_style: ratatui::style::Style,
}

impl TextInputStyle {
    /// The display width of the longest prefix (first_line or continuation).
    pub fn prefix_display_width(&self) -> usize {
        let a = self.first_line_prefix.width();
        let b = self.continuation_prefix.width();
        a.max(b)
    }
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            first_line_prefix: "> ",
            continuation_prefix: "  ",
            placeholder: "Enter message...".to_string(),
            text_style: ratatui::style::Style::default(),
            placeholder_style: ratatui::style::Style::default(),
        }
    }
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            scroll_offset: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.input
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub fn handle_char(&mut self, c: char) {
        if c == '\n' {
            self.handle_newline();
            return;
        }
        if c.is_control() || c == '\u{0}' {
            return;
        }
        let byte_pos = self.char_pos_to_byte_pos(self.cursor);
        self.input.insert(byte_pos, c);
        self.cursor += 1;
    }

    pub fn handle_newline(&mut self) {
        let byte_pos = self.char_pos_to_byte_pos(self.cursor);
        self.input.insert(byte_pos, '\n');
        self.cursor += 1;
    }

    pub fn handle_backspace(&mut self) {
        if self.cursor > 0 && !self.input.is_empty() {
            let byte_pos = self.char_pos_to_byte_pos(self.cursor - 1);
            if byte_pos < self.input.len() {
                self.input.remove(byte_pos);
                self.cursor -= 1;
            }
        }
    }

    pub fn handle_delete(&mut self) {
        let char_count = self.input.chars().count();
        if self.cursor < char_count {
            let byte_pos = self.char_pos_to_byte_pos(self.cursor);
            if byte_pos < self.input.len() {
                self.input.remove(byte_pos);
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let char_count = self.input.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    /// Returns (logical_line, col_in_line, char_offset_of_line_start)
    pub fn cursor_line_col(&self) -> (usize, usize, usize) {
        let mut line = 0;
        let mut line_start = 0usize;
        for (i, ch) in self.input.chars().enumerate() {
            if i == self.cursor {
                return (line, self.cursor - line_start, line_start);
            }
            if ch == '\n' {
                line += 1;
                line_start = i + 1;
            }
        }
        (line, self.cursor - line_start, line_start)
    }

    fn input_line_char_counts(&self) -> Vec<usize> {
        self.input.split('\n').map(|l| l.chars().count()).collect()
    }

    /// Move cursor up one logical line. Returns false if already on first line.
    pub fn move_cursor_up(&mut self) -> bool {
        let (line, col, _) = self.cursor_line_col();
        if line == 0 {
            return false;
        }
        let counts = self.input_line_char_counts();
        let prev_len = counts[line - 1];
        let target_col = col.min(prev_len);
        self.cursor = counts[..line - 1].iter().sum::<usize>() + (line - 1) + target_col;
        true
    }

    /// Move cursor down one logical line. Returns false if already on last line.
    pub fn move_cursor_down(&mut self) -> bool {
        let (line, col, _) = self.cursor_line_col();
        let counts = self.input_line_char_counts();
        if line >= counts.len() - 1 {
            return false;
        }
        let next_len = counts[line + 1];
        let target_col = col.min(next_len);
        self.cursor = counts[..line + 1].iter().sum::<usize>() + (line + 1) + target_col;
        true
    }

    pub fn set_cursor_home(&mut self) {
        let (_, _, line_start) = self.cursor_line_col();
        self.cursor = line_start;
    }

    pub fn set_cursor_end(&mut self) {
        let (line, _, line_start) = self.cursor_line_col();
        let counts = self.input_line_char_counts();
        self.cursor = line_start + counts[line];
    }

    #[allow(dead_code)]
    pub fn set_cursor_buffer_start(&mut self) {
        self.cursor = 0;
    }

    #[allow(dead_code)]
    pub fn set_cursor_buffer_end(&mut self) {
        self.cursor = self.input.chars().count();
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn set_text(&mut self, text: &str) {
        self.input = text.to_string();
        self.cursor = self.input.chars().count();
        self.scroll_offset = 0;
    }

    /// Take input text and reset state. Returns None if input is blank.
    pub fn take_input(&mut self) -> Option<String> {
        if self.input.trim().is_empty() {
            return None;
        }
        let text = self.input.clone();
        self.clear();
        Some(text)
    }

    pub fn insert_paste(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for c in normalized.chars() {
            self.handle_char(c);
        }
    }

    // ── Visual line calculation (for dynamic height and cursor positioning) ──

    fn avail_width(inner_width: u16, prefix_w: usize) -> usize {
        (inner_width as usize).saturating_sub(prefix_w).max(1)
    }

    /// Compute the total visual line count, accounting for wrap.
    /// `prefix_w` is the display width of the line prefix (e.g. 2 for "> ").
    pub fn visual_line_count_with_prefix(&self, inner_width: u16, prefix_w: usize) -> usize {
        if self.input.is_empty() {
            return 1;
        }
        let avail = Self::avail_width(inner_width, prefix_w);
        self.input
            .split('\n')
            .map(|line_text| {
                let text_w = line_text.width();
                if text_w == 0 {
                    1
                } else {
                    (text_w + avail - 1) / avail
                }
            })
            .sum()
    }

    /// Convenience: compute visual line count using default prefix width of 2.
    pub fn visual_line_count(&self, inner_width: u16) -> usize {
        self.visual_line_count_with_prefix(inner_width, 2)
    }

    /// Compute cursor visual position: (visual_row, visual_col) considering wrap.
    fn cursor_visual_position(&self, inner_width: u16, prefix_w: usize) -> (usize, usize) {
        // Use cursor_line_col() to get accurate logical line position
        let (cursor_logical_line, _, line_start) = self.cursor_line_col();

        // Calculate display width of text before cursor on the current line only
        let byte_pos = self.char_pos_to_byte_pos(self.cursor);
        let line_start_byte = self.char_pos_to_byte_pos(line_start);
        let text_before_cursor = &self.input[line_start_byte..byte_pos];
        let cursor_col_w = text_before_cursor.width();

        let avail = Self::avail_width(inner_width, prefix_w);

        let mut visual_row = 0usize;
        for (i, logical_line) in self.input.split('\n').enumerate() {
            if i >= cursor_logical_line {
                break;
            }
            let text_w = logical_line.width();
            if text_w == 0 {
                visual_row += 1;
            } else {
                visual_row += (text_w + avail - 1) / avail;
            }
        }

        let extra_rows = if cursor_col_w > 0 {
            (cursor_col_w - 1) / avail
        } else {
            0
        };
        visual_row += extra_rows;
        let visual_col = prefix_w + cursor_col_w - extra_rows * avail;
        (visual_row, visual_col)
    }

    /// Update scroll_offset so the cursor stays visible within `visible_lines`.
    fn ensure_cursor_visible(&mut self, inner_width: u16, prefix_w: usize, visible_lines: usize) {
        if visible_lines == 0 {
            self.scroll_offset = 0;
            return;
        }
        // Clamp scroll_offset to valid range first (content may have shrunk)
        let total = self.visual_line_count_with_prefix(inner_width, prefix_w);
        let max_scroll = total.saturating_sub(visible_lines);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
        let (cursor_vrow, _) = self.cursor_visual_position(inner_width, prefix_w);
        if cursor_vrow < self.scroll_offset {
            self.scroll_offset = cursor_vrow;
        } else if cursor_vrow >= self.scroll_offset + visible_lines {
            self.scroll_offset = cursor_vrow - visible_lines + 1;
        }
    }

    pub fn char_pos_to_byte_pos(&self, char_pos: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_pos)
            .map(|(pos, _)| pos)
            .unwrap_or(self.input.len())
    }

    // ── Rendering ──

    /// Build visual lines with wrap. Returns the lines and uses `style` for the text content.
    fn build_visual_lines(&self, inner_width: u16, style: &TextInputStyle) -> Vec<Line<'static>> {
        let prefix_w = style.prefix_display_width();
        let avail = Self::avail_width(inner_width, prefix_w);
        let mut visual_lines: Vec<Line<'static>> = Vec::new();

        for (i, logical_line) in self.input.split('\n').enumerate() {
            let prefix = if i == 0 {
                style.first_line_prefix
            } else {
                style.continuation_prefix
            };
            if logical_line.is_empty() {
                visual_lines.push(Line::from(vec![Span::raw(prefix.to_string())]));
            } else {
                let mut chars = logical_line.chars().peekable();
                let mut first_segment = true;
                while chars.peek().is_some() {
                    let seg_prefix = if first_segment {
                        prefix
                    } else {
                        style.continuation_prefix
                    };
                    let mut segment = String::new();
                    let mut seg_w = 0usize;
                    while let Some(&ch) = chars.peek() {
                        let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
                        if seg_w + ch_w > avail && seg_w > 0 {
                            break;
                        }
                        segment.push(ch);
                        seg_w += ch_w;
                        chars.next();
                    }
                    visual_lines.push(Line::from(vec![
                        Span::raw(seg_prefix.to_string()),
                        Span::styled(segment, style.text_style),
                    ]));
                    first_segment = false;
                }
            }
        }

        visual_lines
    }

    /// Render the text input content into the given inner area (no block/border).
    /// Sets cursor position on frame. Pass `show_cursor = false` to skip cursor.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        inner: Rect,
        style: &TextInputStyle,
        show_cursor: bool,
    ) {
        let visible_lines = inner.height as usize;

        if self.input.is_empty() {
            let line = Line::from(vec![
                Span::raw(style.first_line_prefix.to_string()),
                Span::styled(style.placeholder.clone(), style.placeholder_style),
            ]);
            let paragraph = Paragraph::new(line);
            frame.render_widget(paragraph, inner);
            if show_cursor {
                let prefix_w = style.first_line_prefix.width() as u16;
                frame.set_cursor_position((inner.x + prefix_w, inner.y));
            }
            return;
        }

        let prefix_w = style.prefix_display_width();
        self.ensure_cursor_visible(inner.width, prefix_w, visible_lines);

        let visual_lines = self.build_visual_lines(inner.width, style);
        let scroll = self.scroll_offset;
        let visible_slice: Vec<Line<'static>> = visual_lines
            .into_iter()
            .skip(scroll)
            .take(visible_lines)
            .collect();

        let paragraph = Paragraph::new(visible_slice);
        frame.render_widget(paragraph, inner);

        if show_cursor {
            let (cursor_vrow, cursor_vcol) = self.cursor_visual_position(inner.width, prefix_w);
            let row_in_view = cursor_vrow.saturating_sub(scroll);
            if row_in_view < visible_lines {
                frame.set_cursor_position((
                    inner.x + cursor_vcol as u16,
                    inner.y + row_in_view as u16,
                ));
            }
        }
    }
}
