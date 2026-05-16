/// Session selector popup for switching between sessions
///
/// Overlay popup that displays all available sessions
/// and allows the user to select one to switch to.
/// Supports switching and deleting current-project sessions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// A session item for display in the selector
#[derive(Debug, Clone)]
pub struct SessionItem {
    pub session_id: String,
    pub session_name: String,
    pub last_activity: String,
    pub workspace: Option<String>,
}

/// Actions emitted by the session selector back to the caller
#[derive(Debug, Clone)]
pub enum SessionAction {
    /// No action, selector consumed the event
    None,
    /// User selected a session to switch to
    Switch(SessionItem),
    /// User wants to delete the selected session
    Delete(SessionItem),
    /// User cancelled / closed the popup
    Close,
}

/// Session selector popup state
pub struct SessionSelectorState {
    items: Vec<SessionItem>,
    list_state: ListState,
    visible: bool,
    /// Currently active session ID (for highlighting)
    current_session_id: Option<String>,
    last_area: Option<Rect>,
    /// Inline rename state
    rename_editing: bool,
    rename_buffer: String,
    rename_cursor: usize,
}

impl SessionSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            current_session_id: None,
            last_area: None,
            rename_editing: false,
            rename_buffer: String::new(),
            rename_cursor: 0,
        }
    }

    /// Show the session selector with given session list
    pub fn show(&mut self, sessions: Vec<SessionItem>, current_session_id: Option<String>) {
        if sessions.is_empty() {
            return;
        }

        let initial_idx = current_session_id
            .as_ref()
            .and_then(|id| sessions.iter().position(|s| s.session_id == *id))
            .unwrap_or(0);

        self.items = sessions;
        self.current_session_id = current_session_id;
        self.list_state.select(Some(initial_idx));
        self.visible = true;
        self.rename_editing = false;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
        self.rename_editing = false;
    }

    /// Reshow the session selector (for back navigation)
    pub fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Remove item by session_id (after external deletion succeeds)
    pub fn remove_item(&mut self, session_id: &str) {
        self.items.retain(|s| s.session_id != session_id);
        if self.items.is_empty() {
            self.hide();
            return;
        }
        // Clamp selection
        let selected = self.list_state.selected().unwrap_or(0);
        let clamped = selected.min(self.items.len().saturating_sub(1));
        self.list_state.select(Some(clamped));
    }

    fn selected_item(&self) -> Option<&SessionItem> {
        let idx = self.list_state.selected()?;
        self.items.get(idx)
    }

    /// Handle a key event while the selector is visible.
    /// Returns a SessionAction describing what happened.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> SessionAction {
        if !self.visible {
            return SessionAction::None;
        }

        // ── Rename editing mode ──
        if self.rename_editing {
            return self.handle_rename_key(key);
        }

        // ── Normal navigation mode ──
        match (key.code, key.modifiers) {
            (KeyCode::Up, _) => {
                self.move_up();
                SessionAction::None
            }
            (KeyCode::Down, _) => {
                self.move_down();
                SessionAction::None
            }
            (KeyCode::Enter, _) => {
                if let Some(item) = self.selected_item().cloned() {
                    self.hide();
                    SessionAction::Switch(item)
                } else {
                    SessionAction::None
                }
            }
            (KeyCode::Esc, _) => {
                self.hide();
                SessionAction::Close
            }
            // Ctrl+D: delete selected session
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if let Some(item) = self.selected_item().cloned() {
                    SessionAction::Delete(item)
                } else {
                    SessionAction::None
                }
            }
            _ => SessionAction::None,
        }
    }

    /// Handle keys while in rename editing mode.
    /// This path is unreachable while rename is disabled, but keeps stale state harmless.
    fn handle_rename_key(&mut self, key: KeyEvent) -> SessionAction {
        match key.code {
            KeyCode::Enter => {
                self.rename_editing = false;
                SessionAction::None
            }
            KeyCode::Esc => {
                self.rename_editing = false;
                SessionAction::None
            }
            KeyCode::Char(c) => {
                let byte_pos = self.char_to_byte(&self.rename_buffer, self.rename_cursor);
                self.rename_buffer.insert(byte_pos, c);
                self.rename_cursor += 1;
                SessionAction::None
            }
            KeyCode::Backspace => {
                if self.rename_cursor > 0 {
                    self.rename_cursor -= 1;
                    let byte_pos = self.char_to_byte(&self.rename_buffer, self.rename_cursor);
                    let next = self.char_to_byte(&self.rename_buffer, self.rename_cursor + 1);
                    self.rename_buffer.replace_range(byte_pos..next, "");
                }
                SessionAction::None
            }
            KeyCode::Left => {
                self.rename_cursor = self.rename_cursor.saturating_sub(1);
                SessionAction::None
            }
            KeyCode::Right => {
                let max = self.rename_buffer.chars().count();
                self.rename_cursor = (self.rename_cursor + 1).min(max);
                SessionAction::None
            }
            KeyCode::Home => {
                self.rename_cursor = 0;
                SessionAction::None
            }
            KeyCode::End => {
                self.rename_cursor = self.rename_buffer.chars().count();
                SessionAction::None
            }
            _ => SessionAction::None,
        }
    }

    fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(selected.saturating_sub(1)));
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.items.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    /// Render the session selector popup as an overlay
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.items.is_empty() {
            self.last_area = None;
            return;
        }

        // +2 for border, +2 for hint line at bottom
        let popup_width = area.width.saturating_sub(4).min(70);
        let popup_height = (self.items.len() as u16 + 2 + 2).min(area.height.saturating_sub(2));
        if popup_height < 5 || popup_width < 20 {
            self.last_area = None;
            return;
        }

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        self.last_area = Some(popup_area);

        let selected_idx = self.list_state.selected();

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let is_current = self
                    .current_session_id
                    .as_ref()
                    .map_or(false, |id| id == &session.session_id);

                let marker = if is_current { "● " } else { "  " };
                let marker_style = if is_current {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                };

                // If this row is being renamed, show the edit buffer
                if self.rename_editing && selected_idx == Some(i) {
                    let edit_style = Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                    let line = Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::styled(&self.rename_buffer, edit_style),
                        Span::styled("_", Style::default().fg(Color::Yellow)),
                    ]);
                    return ListItem::new(line);
                }

                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let time_style = theme.style(StyleKind::Muted);
                let workspace_style = Style::default().fg(Color::DarkGray);

                let mut spans = vec![
                    Span::styled(marker, marker_style),
                    Span::styled(&session.session_name, name_style),
                ];

                // Show workspace path if available
                if let Some(ref ws) = session.workspace {
                    // Show only the last component for brevity
                    let short_ws = std::path::Path::new(ws)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| ws.clone());
                    spans.push(Span::styled(format!("  [{}]", short_ws), workspace_style));
                }

                spans.push(Span::styled(format!("  {}", session.last_activity), time_style));

                let line = Line::from(spans);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Switch Session ");

        let list = List::new(list_items)
            .block(block)
            .style(Style::default().bg(theme.background))
            .highlight_style(
                Style::default()
                    .bg(theme.primary)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.list_state);

        // Render hint bar below the list
        let hint_y = popup_area.y + popup_area.height;
        if hint_y < area.y + area.height {
            let hint_area = Rect {
                x: popup_area.x,
                y: hint_y,
                width: popup_area.width,
                height: 1,
            };
            let hint_text = if self.rename_editing {
                " Enter: Save  Esc: Cancel "
            } else {
                " Up/Down: Navigate  Enter: Switch  Ctrl+D: Delete  Esc: Close "
            };
            let hint = Paragraph::new(Line::from(Span::styled(
                hint_text,
                theme.style(StyleKind::Muted),
            )));
            frame.render_widget(hint, hint_area);
        }
    }

    /// Handle mouse events
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> SessionAction {
        if !self.visible || self.rename_editing {
            return SessionAction::None;
        }

        let area = match self.last_area {
            Some(area) => area,
            None => return SessionAction::None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.move_up();
                SessionAction::None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.move_down();
                SessionAction::None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                SessionAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                    if let Some(item) = self.selected_item().cloned() {
                        self.hide();
                        return SessionAction::Switch(item);
                    }
                }
                SessionAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                SessionAction::Close
            }
            _ => SessionAction::None,
        }
    }

    pub fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
        self.visible
    }

    fn item_index_at(&self, row: u16, area: Rect) -> Option<usize> {
        if area.height < 3 {
            return None;
        }
        let inner_y = area.y.saturating_add(1);
        let inner_height = area.height.saturating_sub(2);

        if row < inner_y || row >= inner_y.saturating_add(inner_height) {
            return None;
        }

        let offset = self.list_state.offset();
        let index = (row - inner_y) as usize + offset;
        if index >= self.items.len() {
            return None;
        }

        Some(index)
    }

    fn char_to_byte(&self, s: &str, char_pos: usize) -> usize {
        s.char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }
}
