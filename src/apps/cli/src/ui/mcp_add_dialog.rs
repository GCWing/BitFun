/// MCP server add dialog — step-by-step wizard (opencode style)
///
/// Step 1: Enter server name (ID)
/// Step 2: Select type — local (stdio) / remote (streamable-http)
/// Step 3: Enter command (local) or URL (remote)
///
/// Enter advances to next step, Esc goes back or cancels.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// Action returned by the MCP add dialog
#[derive(Debug, Clone)]
pub enum McpAddAction {
    /// No action, dialog consumed the key
    None,
    /// User completed all steps — returns name + generated JSON config string
    Confirm {
        name: String,
        config_json: String,
    },
    /// User cancelled (Esc on step 1)
    Cancel,
}

/// Current wizard step
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Name,
    ServerType,
    CommandOrUrl,
}

/// Server type choice
#[derive(Debug, Clone, Copy, PartialEq)]
enum ServerType {
    Local,
    Remote,
}

/// MCP add dialog state
pub struct McpAddDialogState {
    visible: bool,
    step: Step,
    /// Server name / ID
    name_buf: String,
    name_cursor: usize,
    /// Server type selection
    server_type: ServerType,
    /// Command (local) or URL (remote)
    value_buf: String,
    value_cursor: usize,
    /// Error message to display
    error: Option<String>,
}

impl McpAddDialogState {
    pub fn new() -> Self {
        Self {
            visible: false,
            step: Step::Name,
            name_buf: String::new(),
            name_cursor: 0,
            server_type: ServerType::Local,
            value_buf: String::new(),
            value_cursor: 0,
            error: None,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.step = Step::Name;
        self.name_buf.clear();
        self.name_cursor = 0;
        self.server_type = ServerType::Local;
        self.value_buf.clear();
        self.value_cursor = 0;
        self.error = None;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.name_buf.clear();
        self.value_buf.clear();
        self.error = None;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Insert pasted text into the current active text field
    pub fn insert_text(&mut self, text: &str) {
        if !self.visible {
            return;
        }
        let cleaned: String = text
            .chars()
            .filter(|c| *c != '\n' && *c != '\r' && *c != '\t')
            .collect();
        for c in cleaned.chars() {
            self.insert_char_into_active(c);
        }
    }

    /// Handle a key event
    pub fn handle_key_event(&mut self, key: KeyEvent) -> McpAddAction {
        if !self.visible {
            return McpAddAction::None;
        }

        self.error = None;

        match self.step {
            Step::Name => self.handle_name_step(key),
            Step::ServerType => self.handle_type_step(key),
            Step::CommandOrUrl => self.handle_value_step(key),
        }
    }

    // ── Step 1: Name ──

    fn handle_name_step(&mut self, key: KeyEvent) -> McpAddAction {
        match key.code {
            KeyCode::Esc => {
                self.hide();
                McpAddAction::Cancel
            }
            KeyCode::Enter => {
                let name = self.name_buf.trim().to_string();
                if name.is_empty() {
                    self.error = Some("Server name cannot be empty".to_string());
                    return McpAddAction::None;
                }
                if name.contains(' ') {
                    self.error = Some("Name cannot contain spaces".to_string());
                    return McpAddAction::None;
                }
                // Advance to type selection
                self.step = Step::ServerType;
                McpAddAction::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.name_buf.clear();
                self.name_cursor = 0;
                McpAddAction::None
            }
            KeyCode::Char(c) => {
                insert_char(&mut self.name_buf, &mut self.name_cursor, c);
                McpAddAction::None
            }
            KeyCode::Backspace => {
                backspace(&mut self.name_buf, &mut self.name_cursor);
                McpAddAction::None
            }
            KeyCode::Delete => {
                delete_forward(&mut self.name_buf, &mut self.name_cursor);
                McpAddAction::None
            }
            KeyCode::Left => {
                self.name_cursor = self.name_cursor.saturating_sub(1);
                McpAddAction::None
            }
            KeyCode::Right => {
                let max = self.name_buf.chars().count();
                self.name_cursor = (self.name_cursor + 1).min(max);
                McpAddAction::None
            }
            KeyCode::Home => {
                self.name_cursor = 0;
                McpAddAction::None
            }
            KeyCode::End => {
                self.name_cursor = self.name_buf.chars().count();
                McpAddAction::None
            }
            _ => McpAddAction::None,
        }
    }

    // ── Step 2: Type selection ──

    fn handle_type_step(&mut self, key: KeyEvent) -> McpAddAction {
        match key.code {
            KeyCode::Esc => {
                // Go back to name step
                self.step = Step::Name;
                McpAddAction::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Confirm selection, advance to value step
                self.value_buf.clear();
                self.value_cursor = 0;
                self.step = Step::CommandOrUrl;
                McpAddAction::None
            }
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                self.server_type = ServerType::Local;
                McpAddAction::None
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('l') | KeyCode::Char('j') => {
                self.server_type = ServerType::Remote;
                McpAddAction::None
            }
            KeyCode::Tab => {
                self.server_type = match self.server_type {
                    ServerType::Local => ServerType::Remote,
                    ServerType::Remote => ServerType::Local,
                };
                McpAddAction::None
            }
            _ => McpAddAction::None,
        }
    }

    // ── Step 3: Command / URL ──

    fn handle_value_step(&mut self, key: KeyEvent) -> McpAddAction {
        match key.code {
            KeyCode::Esc => {
                // Go back to type step
                self.step = Step::ServerType;
                McpAddAction::None
            }
            KeyCode::Enter => {
                let value = self.value_buf.trim().to_string();
                if value.is_empty() {
                    let label = match self.server_type {
                        ServerType::Local => "Command",
                        ServerType::Remote => "URL",
                    };
                    self.error = Some(format!("{} cannot be empty", label));
                    return McpAddAction::None;
                }
                // Build JSON config and confirm
                let config_json = self.build_config_json(&value);
                let name = self.name_buf.trim().to_string();
                self.hide();
                McpAddAction::Confirm { name, config_json }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value_buf.clear();
                self.value_cursor = 0;
                McpAddAction::None
            }
            KeyCode::Char(c) => {
                insert_char(&mut self.value_buf, &mut self.value_cursor, c);
                McpAddAction::None
            }
            KeyCode::Backspace => {
                backspace(&mut self.value_buf, &mut self.value_cursor);
                McpAddAction::None
            }
            KeyCode::Delete => {
                delete_forward(&mut self.value_buf, &mut self.value_cursor);
                McpAddAction::None
            }
            KeyCode::Left => {
                self.value_cursor = self.value_cursor.saturating_sub(1);
                McpAddAction::None
            }
            KeyCode::Right => {
                let max = self.value_buf.chars().count();
                self.value_cursor = (self.value_cursor + 1).min(max);
                McpAddAction::None
            }
            KeyCode::Home => {
                self.value_cursor = 0;
                McpAddAction::None
            }
            KeyCode::End => {
                self.value_cursor = self.value_buf.chars().count();
                McpAddAction::None
            }
            _ => McpAddAction::None,
        }
    }

    /// Build a Cursor-format JSON config from the wizard inputs
    fn build_config_json(&self, value: &str) -> String {
        match self.server_type {
            ServerType::Local => {
                // Split command string into command + args
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() <= 1 {
                    format!(r#"{{"type":"stdio","command":"{}"}}"#, value)
                } else {
                    let cmd = parts[0];
                    let args: Vec<String> = parts[1..].iter().map(|s| format!("\"{}\"", s)).collect();
                    format!(
                        r#"{{"type":"stdio","command":"{}","args":[{}]}}"#,
                        cmd,
                        args.join(",")
                    )
                }
            }
            ServerType::Remote => {
                format!(r#"{{"type":"streamable-http","url":"{}"}}"#, value)
            }
        }
    }

    fn insert_char_into_active(&mut self, c: char) {
        if c == '\n' || c == '\r' {
            return;
        }
        match self.step {
            Step::Name => insert_char(&mut self.name_buf, &mut self.name_cursor, c),
            Step::CommandOrUrl => insert_char(&mut self.value_buf, &mut self.value_cursor, c),
            Step::ServerType => {} // no text input on type step
        }
    }

    // ── Rendering ──

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }

        // Dialog height:
        //   border(2) + completed steps + prompt(1) + current input(1) + hint(1) + error?(1)
        let completed_rows = match self.step {
            Step::Name => 0u16,
            Step::ServerType => 1,
            Step::CommandOrUrl => 2,
        };
        let has_error = self.error.is_some();
        let dialog_height: u16 = 2 + completed_rows + 1 + 1 + 1 + if has_error { 1 } else { 0 };
        let dialog_width = area.width.saturating_sub(4).min(65);
        if dialog_width < 35 || area.height < dialog_height + 2 {
            return;
        }

        let dialog_x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x: dialog_x,
            y: dialog_y,
            width: dialog_width,
            height: dialog_height,
        };

        frame.render_widget(Clear, dialog_area);

        let step_label = match self.step {
            Step::Name => "1/3",
            Step::ServerType => "2/3",
            Step::CommandOrUrl => "3/3",
        };
        let title = format!(" Add MCP Server ({}) ", step_label);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(title);

        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        if inner.width < 20 {
            return;
        }

        let mut row = 0u16;

        // ── Render completed steps as read-only summary ──

        if self.step == Step::ServerType || self.step == Step::CommandOrUrl {
            let name_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
            let name_line = Line::from(vec![
                Span::styled("\u{2713} ", theme.style(StyleKind::Success)),
                Span::styled("Name: ", theme.style(StyleKind::Muted)),
                Span::styled(
                    self.name_buf.as_str(),
                    theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(name_line), name_area);
            row += 1;
        }

        if self.step == Step::CommandOrUrl {
            let type_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let type_label = match self.server_type {
                    ServerType::Local => "local (stdio)",
                    ServerType::Remote => "remote (streamable-http)",
                };
            let type_line = Line::from(vec![
                Span::styled("\u{2713} ", theme.style(StyleKind::Success)),
                Span::styled("Type: ", theme.style(StyleKind::Muted)),
                Span::styled(
                    type_label,
                    theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(type_line), type_area);
            row += 1;
        }

        // ── Render current step: prompt line + input/selector ──

        match self.step {
            Step::Name => {
                // Prompt
                let prompt_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let prompt_line = Line::from(Span::styled(
                    "Enter MCP server name (used as identifier):",
                    theme.style(StyleKind::Info),
                ));
                frame.render_widget(Paragraph::new(prompt_line), prompt_area);
                row += 1;

                // Input
                let input_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let line = render_input_line(
                    "  > ",
                    &self.name_buf,
                    self.name_cursor,
                    "my-server",
                    inner.width as usize,
                    theme,
                );
                frame.render_widget(Paragraph::new(line), input_area);
                row += 1;
            }
            Step::ServerType => {
                // Prompt
                let prompt_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let prompt_line = Line::from(Span::styled(
                    "Select MCP server type:",
                    theme.style(StyleKind::Info),
                ));
                frame.render_widget(Paragraph::new(prompt_line), prompt_area);
                row += 1;

                // Selector
                let type_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let local_style = if self.server_type == ServerType::Local {
                    Style::default().fg(Color::White).bg(theme.primary).add_modifier(Modifier::BOLD)
                } else {
                    theme.style(StyleKind::Muted)
                };
                let remote_style = if self.server_type == ServerType::Remote {
                    Style::default().fg(Color::White).bg(theme.primary).add_modifier(Modifier::BOLD)
                } else {
                    theme.style(StyleKind::Muted)
                };
                let type_line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" \u{25b6} local (stdio) ", local_style),
                    Span::raw("   "),
                    Span::styled(" \u{25b6} remote (streamable-http) ", remote_style),
                ]);
                frame.render_widget(Paragraph::new(type_line), type_area);
                row += 1;
            }
            Step::CommandOrUrl => {
                // Prompt
                let prompt_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let (prompt_text, placeholder) = match self.server_type {
                    ServerType::Local => (
                        "Enter command to run the MCP server:",
                        "npx -y @modelcontextprotocol/server-xxx",
                    ),
                    ServerType::Remote => (
                        "Enter the remote MCP server URL:",
                        "https://example.com/mcp",
                    ),
                };
                let prompt_line = Line::from(Span::styled(
                    prompt_text,
                    theme.style(StyleKind::Info),
                ));
                frame.render_widget(Paragraph::new(prompt_line), prompt_area);
                row += 1;

                // Input
                let input_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let line = render_input_line(
                    "  > ",
                    &self.value_buf,
                    self.value_cursor,
                    placeholder,
                    inner.width as usize,
                    theme,
                );
                frame.render_widget(Paragraph::new(line), input_area);
                row += 1;
            }
        }

        // ── Error line ──

        if let Some(ref err) = self.error {
            if inner.y + row < inner.y + inner.height {
                let err_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
                let err_line = Line::from(Span::styled(err.as_str(), theme.style(StyleKind::Error)));
                frame.render_widget(Paragraph::new(err_line), err_area);
                row += 1;
            }
        }

        // ── Hint line ──

        if inner.y + row < inner.y + inner.height {
            let hint_area = Rect { x: inner.x, y: inner.y + row, width: inner.width, height: 1 };
            let hint_text = match self.step {
                Step::Name => "Enter: Next  Esc: Cancel",
                Step::ServerType => "\u{2190}\u{2192}/Tab: Switch  Enter: Next  Esc: Back",
                Step::CommandOrUrl => "Enter: Confirm  Ctrl+U: Clear  Esc: Back",
            };
            let hint = Paragraph::new(Line::from(Span::styled(
                hint_text,
                theme.style(StyleKind::Muted),
            )));
            frame.render_widget(hint, hint_area);
        }
    }
}

// ── Helper functions ──

fn insert_char(buf: &mut String, cursor: &mut usize, c: char) {
    let byte_pos = char_to_byte(buf, *cursor);
    buf.insert(byte_pos, c);
    *cursor += 1;
}

fn backspace(buf: &mut String, cursor: &mut usize) {
    if *cursor > 0 {
        *cursor -= 1;
        let byte_pos = char_to_byte(buf, *cursor);
        let next_byte = char_to_byte(buf, *cursor + 1);
        buf.replace_range(byte_pos..next_byte, "");
    }
}

fn delete_forward(buf: &mut String, cursor: &mut usize) {
    let max = buf.chars().count();
    if *cursor < max {
        let byte_pos = char_to_byte(buf, *cursor);
        let next_byte = char_to_byte(buf, *cursor + 1);
        buf.replace_range(byte_pos..next_byte, "");
    }
}

/// Render a single-line input field with cursor, placeholder, and horizontal scrolling
fn render_input_line<'a>(
    label: &'a str,
    buffer: &'a str,
    cursor: usize,
    placeholder: &'a str,
    available_width: usize,
    theme: &Theme,
) -> Line<'a> {
    let label_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);

    let label_len = label.chars().count();
    let field_width = available_width.saturating_sub(label_len);

    // Empty buffer: show placeholder with cursor at start
    if buffer.is_empty() {
        let placeholder_display: String = placeholder.chars().take(field_width.saturating_sub(1)).collect();
        return Line::from(vec![
            Span::styled(label, label_style),
            Span::styled(" ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::styled(placeholder_display, theme.style(StyleKind::Muted).add_modifier(Modifier::DIM)),
        ]);
    }

    let total_chars = buffer.chars().count();

    // Calculate scroll offset to keep cursor visible
    let scroll = if field_width == 0 {
        0
    } else if cursor < field_width / 3 {
        0
    } else {
        cursor.saturating_sub(field_width / 3)
    };

    let visible_chars: String = buffer.chars().skip(scroll).take(field_width).collect();
    let cursor_in_view = cursor.saturating_sub(scroll);

    let before: String = visible_chars.chars().take(cursor_in_view).collect();
    let cursor_char: String = visible_chars.chars().skip(cursor_in_view).take(1).collect();
    let after: String = visible_chars.chars().skip(cursor_in_view + 1).collect();

    let cursor_display = if cursor_char.is_empty() { " ".to_string() } else { cursor_char };

    let has_more_left = scroll > 0;
    let has_more_right = scroll + field_width < total_chars;

    let mut spans = vec![Span::styled(label, label_style)];

    if has_more_left {
        spans.push(Span::styled("\u{2190}", theme.style(StyleKind::Muted)));
        let before_trimmed: String = before.chars().skip(1).collect();
        spans.push(Span::styled(before_trimmed, text_style));
    } else {
        spans.push(Span::styled(before, text_style));
    }

    spans.push(Span::styled(cursor_display, Style::default().fg(Color::Black).bg(Color::White)));

    if has_more_right {
        let after_len = after.chars().count();
        if after_len > 0 {
            let after_trimmed: String = after.chars().take(after_len - 1).collect();
            spans.push(Span::styled(after_trimmed, text_style));
        }
        spans.push(Span::styled("\u{2192}", theme.style(StyleKind::Muted)));
    } else {
        spans.push(Span::styled(after, text_style));
    }

    Line::from(spans)
}

fn char_to_byte(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}
