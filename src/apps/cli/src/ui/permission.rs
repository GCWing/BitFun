/// Permission confirmation modal panel
///
/// Inspired by opencode TUI's PermissionPrompt component.
/// Three-level permission system:
/// - Allow once: execute this tool call only
/// - Allow always: auto-approve this tool type for the session
/// - Reject: deny execution (optionally with a reason)

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme::{Theme, StyleKind, tool_icon};
use super::string_utils::truncate_str;

// ============ Data Types ============

/// Permission prompt stage
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionStage {
    /// Main permission screen: Allow once / Allow always / Reject
    Permission,
    /// Confirm "Allow always" action
    ConfirmAlways,
    /// Reject with reason input
    RejectWithReason,
}

/// Permission prompt state
#[derive(Debug, Clone)]
pub struct PermissionPrompt {
    pub tool_id: String,
    pub tool_name: String,
    pub params: serde_json::Value,
    pub stage: PermissionStage,
    /// Selected option index: 0=Allow once, 1=Allow always, 2=Reject
    pub selected_option: usize,
    /// Reject reason input buffer
    pub reject_reason: String,
}

/// Result of handling a key event in the permission prompt
#[derive(Debug, Clone)]
pub enum PermissionAction {
    /// No action, continue showing the prompt
    None,
    /// User confirmed: allow once (with optional updated input)
    AllowOnce,
    /// User confirmed: allow always
    AllowAlways,
    /// User rejected with a reason
    Reject(String),
}

impl PermissionPrompt {
    /// Create a new permission prompt from a ConfirmationNeeded event
    pub fn new(tool_id: String, tool_name: String, params: serde_json::Value) -> Self {
        Self {
            tool_id,
            tool_name,
            params,
            stage: PermissionStage::Permission,
            selected_option: 0,
            reject_reason: String::new(),
        }
    }

    /// Handle a key event. Returns a PermissionAction if the user made a decision.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> PermissionAction {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return PermissionAction::None;
        }

        match &self.stage {
            PermissionStage::Permission => self.handle_permission_key(key),
            PermissionStage::ConfirmAlways => self.handle_confirm_always_key(key),
            PermissionStage::RejectWithReason => self.handle_reject_reason_key(key),
        }
    }

    fn handle_permission_key(&mut self, key: KeyEvent) -> PermissionAction {
        match (key.code, key.modifiers) {
            // Navigate options
            (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                if self.selected_option > 0 {
                    self.selected_option -= 1;
                }
                PermissionAction::None
            }
            (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if self.selected_option < 2 {
                    self.selected_option += 1;
                }
                PermissionAction::None
            }

            // Confirm selection
            (KeyCode::Enter, _) => match self.selected_option {
                0 => PermissionAction::AllowOnce,
                1 => {
                    self.stage = PermissionStage::ConfirmAlways;
                    self.selected_option = 0; // Reset to "Confirm"
                    PermissionAction::None
                }
                2 => {
                    self.stage = PermissionStage::RejectWithReason;
                    PermissionAction::None
                }
                _ => PermissionAction::None,
            },

            // Escape = reject
            (KeyCode::Esc, _) => PermissionAction::Reject("User dismissed".to_string()),

            _ => PermissionAction::None,
        }
    }

    fn handle_confirm_always_key(&mut self, key: KeyEvent) -> PermissionAction {
        match (key.code, key.modifiers) {
            (KeyCode::Left, _) | (KeyCode::Right, _)
            | (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                self.selected_option = if self.selected_option == 0 { 1 } else { 0 };
                PermissionAction::None
            }
            (KeyCode::Enter, _) => {
                if self.selected_option == 0 {
                    PermissionAction::AllowAlways
                } else {
                    // Cancel — go back to main
                    self.stage = PermissionStage::Permission;
                    self.selected_option = 1;
                    PermissionAction::None
                }
            }
            (KeyCode::Esc, _) => {
                self.stage = PermissionStage::Permission;
                self.selected_option = 1;
                PermissionAction::None
            }
            _ => PermissionAction::None,
        }
    }

    fn handle_reject_reason_key(&mut self, key: KeyEvent) -> PermissionAction {
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => {
                let reason = if self.reject_reason.trim().is_empty() {
                    "User rejected".to_string()
                } else {
                    self.reject_reason.clone()
                };
                PermissionAction::Reject(reason)
            }
            (KeyCode::Esc, _) => {
                self.stage = PermissionStage::Permission;
                self.selected_option = 2;
                self.reject_reason.clear();
                PermissionAction::None
            }
            (KeyCode::Backspace, _) => {
                self.reject_reason.pop();
                PermissionAction::None
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                if !c.is_control() {
                    self.reject_reason.push(c);
                }
                PermissionAction::None
            }
            _ => PermissionAction::None,
        }
    }
}

// ============ Rendering ============

/// Render the permission overlay on top of the message area.
///
/// This renders at the bottom of the given area, taking up a fixed height.
pub fn render_permission_overlay(
    frame: &mut Frame,
    prompt: &PermissionPrompt,
    theme: &Theme,
    area: Rect,
) {
    match &prompt.stage {
        PermissionStage::Permission => render_permission_main(frame, prompt, theme, area),
        PermissionStage::ConfirmAlways => render_confirm_always(frame, prompt, theme, area),
        PermissionStage::RejectWithReason => render_reject_reason(frame, prompt, theme, area),
    }
}

/// Render the main permission prompt (Allow once / Allow always / Reject)
fn render_permission_main(
    frame: &mut Frame,
    prompt: &PermissionPrompt,
    theme: &Theme,
    area: Rect,
) {
    // Calculate overlay height based on content
    let overlay_height = 8u16.min(area.height.saturating_sub(2));
    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };

    // Clear the area
    frame.render_widget(Clear, overlay_area);

    // Split into content + button bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // content
            Constraint::Length(2), // button bar
        ])
        .split(overlay_area);

    // Content block with warning left border
    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.background_panel));

    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    // Build content lines
    let mut lines = vec![
        Line::from(vec![
            Span::styled("\u{25b3} ", theme.style(StyleKind::Warning)), // △
            Span::styled("Permission required", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    // Tool details
    let icon = tool_icon(&prompt.tool_name);
    let detail = build_tool_detail(prompt);
    lines.push(Line::from(vec![
        Span::styled(format!("{} ", icon), theme.style(StyleKind::Muted)),
        Span::styled(detail, Style::default()),
    ]));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    // Button bar
    render_button_bar(
        frame,
        chunks[1],
        theme,
        &["Allow once", "Allow always", "Reject"],
        prompt.selected_option,
        "\u{21c6} select  Enter confirm  Esc reject",
    );
}

/// Render the "Confirm Always" stage
fn render_confirm_always(
    frame: &mut Frame,
    prompt: &PermissionPrompt,
    theme: &Theme,
    area: Rect,
) {
    let overlay_height = 6u16.min(area.height.saturating_sub(2));
    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };

    frame.render_widget(Clear, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .split(overlay_area);

    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.background_panel));

    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    let lines = vec![
        Line::from(vec![
            Span::styled("\u{25b3} ", theme.style(StyleKind::Warning)),
            Span::styled("Always allow", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("This will auto-approve '{}' tool calls for this session.", prompt.tool_name),
            theme.style(StyleKind::Muted),
        )),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    render_button_bar(
        frame,
        chunks[1],
        theme,
        &["Confirm", "Cancel"],
        prompt.selected_option,
        "Enter confirm  Esc cancel",
    );
}

/// Render the "Reject with reason" stage
fn render_reject_reason(
    frame: &mut Frame,
    prompt: &PermissionPrompt,
    theme: &Theme,
    area: Rect,
) {
    let overlay_height = 7u16.min(area.height.saturating_sub(2));
    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };

    frame.render_widget(Clear, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(overlay_area);

    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.error))
        .style(Style::default().bg(theme.background_panel));

    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    let reason_display = if prompt.reject_reason.is_empty() {
        "(optional reason)".to_string()
    } else {
        format!("{}\u{2588}", prompt.reject_reason) // cursor block
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("\u{25b3} ", theme.style(StyleKind::Error)),
            Span::styled("Reject permission", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tell the AI what to do differently:",
            theme.style(StyleKind::Muted),
        )),
        Line::from(Span::styled(
            reason_display,
            if prompt.reject_reason.is_empty() {
                theme.style(StyleKind::Muted)
            } else {
                Style::default()
            },
        )),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    // Bottom hint bar
    let hint_block = Block::default()
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(hint_block, chunks[1]);

    let hint = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled("Enter", Style::default()),
        Span::styled(" confirm  ", theme.style(StyleKind::Muted)),
        Span::styled("Esc", Style::default()),
        Span::styled(" cancel", theme.style(StyleKind::Muted)),
    ]))
    .style(Style::default().bg(theme.background_element));
    frame.render_widget(hint, chunks[1]);
}

/// Render a horizontal button bar with selectable options
fn render_button_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    options: &[&str],
    selected: usize,
    hint_text: &str,
) {
    let bar_block = Block::default()
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(bar_block, area);

    // Build button spans
    let mut spans = vec![Span::raw(" ")];
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if i == selected {
            spans.push(Span::styled(
                format!(" {} ", option),
                Style::default()
                    .fg(theme.background)
                    .bg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", option),
                Style::default()
                    .fg(theme.muted)
                    .bg(theme.background_element),
            ));
        }
    }

    // Add hint text on the right side if there's room
    let buttons_width: usize = spans.iter().map(|s| s.width()).sum();
    let hint_width = hint_text.len() + 2;
    if buttons_width + hint_width < area.width as usize {
        let padding = area.width as usize - buttons_width - hint_width;
        spans.push(Span::raw(" ".repeat(padding)));
        spans.push(Span::styled(hint_text, theme.style(StyleKind::Muted)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(paragraph, area);
}

/// Build a tool detail string for the permission prompt body
fn build_tool_detail(prompt: &PermissionPrompt) -> String {
    match prompt.tool_name.as_str() {
        "Bash" | "bash_tool" | "run_terminal_cmd" => {
            let cmd = prompt.params.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let desc = prompt.params.get("description")
                .and_then(|v| v.as_str());
            match desc {
                Some(d) => format!("{}\n$ {}", d, cmd),
                None => format!("$ {}", cmd),
            }
        }
        "Edit" | "search_replace" => {
            let path = prompt.params.get("file_path")
                .or_else(|| prompt.params.get("target_file"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("Edit {}", path)
        }
        "Write" | "write_file" | "write_file_tool" => {
            let path = prompt.params.get("file_path")
                .or_else(|| prompt.params.get("target_file"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("Write {}", path)
        }
        "Delete" => {
            let path = prompt.params.get("file_path")
                .or_else(|| prompt.params.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("Delete {}", path)
        }
        "Task" => {
            let desc = prompt.params.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Task");
            let subagent = prompt.params.get("subagent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("{} Task: {}", subagent, desc)
        }
        _ => {
            // Generic: show tool name + key param
            let key_param = extract_first_param(&prompt.params);
            if key_param.is_empty() {
                format!("Call tool {}", prompt.tool_name)
            } else {
                format!("{} {}", prompt.tool_name, truncate_str(&key_param, 60))
            }
        }
    }
}

/// Extract the first meaningful string parameter from JSON
fn extract_first_param(params: &serde_json::Value) -> String {
    if let Some(obj) = params.as_object() {
        let priority = ["command", "path", "file_path", "query", "pattern", "url", "description"];
        for key in &priority {
            if let Some(v) = obj.get(*key).and_then(|v| v.as_str()) {
                return v.to_string();
            }
        }
        for (_, value) in obj.iter() {
            if let Some(s) = value.as_str() {
                if s.len() < 100 {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}
