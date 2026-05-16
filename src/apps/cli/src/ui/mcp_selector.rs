/// MCP server selector popup
///
/// Overlay popup that displays all configured MCP servers with their status,
/// and allows the user to toggle (start/stop) them.
///
/// Inspired by opencode's DialogMcp component:
/// - Lists all MCP servers with name, type, status, and tool count
/// - Space key toggles server on/off
/// - Enter key also toggles
/// - Status indicators: ✓ Connected (green), ○ Stopped (gray), ✗ Failed (red), ⋯ Loading (yellow)

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// An MCP server item for display in the selector
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct McpItem {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub status: String,
    pub enabled: bool,
    pub tool_count: usize,
}

/// Action returned from the MCP selector
#[derive(Debug, Clone)]
pub enum McpAction {
    /// Toggle (start/stop) the selected server
    Toggle(McpItem),
    /// No action (dismiss)
    None,
}

/// MCP selector popup state
pub struct McpSelectorState {
    items: Vec<McpItem>,
    list_state: ListState,
    visible: bool,
    /// Which server is currently being toggled (loading indicator)
    pub loading_id: Option<String>,
    /// Server ID pending delete confirmation (double-tap 'd' to confirm)
    pub confirm_delete_id: Option<String>,
    last_area: Option<Rect>,
}

impl McpSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            loading_id: None,
            confirm_delete_id: None,
            last_area: None,
        }
    }

    /// Show the MCP selector with given server list
    pub fn show(&mut self, items: Vec<McpItem>) {
        self.items = items;
        if !self.items.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
        self.loading_id = None;
        self.visible = true;
    }

    /// Update items in-place (after toggle completes) without closing
    pub fn update_items(&mut self, items: Vec<McpItem>) {
        let selected_idx = self.list_state.selected();
        self.items = items;
        // Preserve selection if possible
        if let Some(idx) = selected_idx {
            if idx >= self.items.len() {
                self.list_state.select(Some(self.items.len().saturating_sub(1)));
            }
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.loading_id = None;
        self.confirm_delete_id = None;
        self.last_area = None;
    }

    /// Reshow the MCP selector (for back navigation)
    pub fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    /// Enter confirm-delete mode for a server
    pub fn start_confirm_delete(&mut self, server_id: String) {
        self.confirm_delete_id = Some(server_id);
    }

    /// Cancel confirm-delete mode
    pub fn cancel_confirm_delete(&mut self) {
        self.confirm_delete_id = None;
    }

    /// Check if a server is in confirm-delete mode
    pub fn is_confirm_delete(&self, server_id: &str) -> bool {
        self.confirm_delete_id.as_deref() == Some(server_id)
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn move_up(&mut self) {
        if !self.visible || self.items.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = selected.saturating_sub(1);
        self.list_state.select(Some(next));
    }

    pub fn move_down(&mut self) {
        if !self.visible || self.items.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.items.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    /// Get the selected MCP item (for toggle action)
    pub fn confirm_selection(&self) -> Option<McpItem> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected()?;
        self.items.get(idx).cloned()
    }

    /// Render the MCP selector popup as an overlay
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(72);
        // +5 for border(2) + title(1) + hint(1) + padding(1)
        let popup_height = (self.items.len() as u16 + 5).min(area.height.saturating_sub(2)).max(6);
        if popup_height < 5 || popup_width < 30 {
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

        let loading_id = self.loading_id.clone();
        let confirm_delete_id = self.confirm_delete_id.clone();
        let has_confirm_delete = confirm_delete_id.is_some();

        let mut list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let is_loading = loading_id.as_ref().map_or(false, |id| id == &item.id);
                let is_confirm_delete = confirm_delete_id.as_ref().map_or(false, |id| id == &item.id);

                // If this item is pending delete confirmation, show special style
                if is_confirm_delete {
                    let line = Line::from(vec![
                        Span::styled(
                            "\u{2717} ",
                            theme.style(StyleKind::Error).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &item.name,
                            theme.style(StyleKind::Error).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "  \u{2190} Press 'd' again to delete, any other key to cancel",
                            theme.style(StyleKind::Error),
                        ),
                    ]);
                    return ListItem::new(line);
                }

                // Status indicator
                let (marker, marker_style) = if is_loading {
                    ("\u{22ef} ", theme.style(StyleKind::Warning)) // ⋯
                } else {
                    match item.status.as_str() {
                        "Connected" | "Healthy" => {
                            ("\u{2713} ", theme.style(StyleKind::Success)) // ✓
                        }
                        "Failed" => {
                            ("\u{2717} ", theme.style(StyleKind::Error)) // ✗
                        }
                        _ => {
                            ("\u{25cb} ", theme.style(StyleKind::Muted)) // ○
                        }
                    }
                };

                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let type_style = theme.style(StyleKind::Muted);
                let status_style = if is_loading {
                    theme.style(StyleKind::Warning)
                } else {
                    match item.status.as_str() {
                        "Connected" | "Healthy" => theme.style(StyleKind::Success),
                        "Failed" => theme.style(StyleKind::Error),
                        _ => theme.style(StyleKind::Muted),
                    }
                };

                let status_text = if is_loading {
                    "Loading...".to_string()
                } else {
                    item.status.clone()
                };

                let tool_text = if item.tool_count > 0 {
                    format!(" ({} tools)", item.tool_count)
                } else {
                    String::new()
                };

                let line = Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(&item.name, name_style),
                    Span::raw("  "),
                    Span::styled(&item.server_type, type_style),
                    Span::raw("  "),
                    Span::styled(status_text, status_style),
                    Span::styled(tool_text, theme.style(StyleKind::Muted)),
                ]);
                ListItem::new(line)
            })
            .collect();

        if list_items.is_empty() {
            list_items.push(ListItem::new(Line::from(Span::styled(
                "  No MCP servers configured. Press 'a' to add one.",
                theme.style(StyleKind::Muted),
            ))));
        }

        // Footer hint line — changes when in confirm-delete mode
        let hint_text = if has_confirm_delete {
            " d:Confirm Delete  Any key:Cancel"
        } else {
            " a:Add  d:Delete  e:Edit Config  Space:Toggle  Esc:Close"
        };
        list_items.push(ListItem::new(Line::from(Span::styled(
            hint_text,
            theme.style(StyleKind::Muted),
        ))));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" MCP Servers ");

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
    }

    /// Handle mouse events
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> McpAction {
        if !self.visible {
            return McpAction::None;
        }

        let area = match self.last_area {
            Some(area) => area,
            None => return McpAction::None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.move_up();
                McpAction::None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.move_down();
                McpAction::None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                McpAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                    if let Some(item) = self.confirm_selection() {
                        return McpAction::Toggle(item);
                    }
                }
                McpAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                McpAction::None
            }
            _ => McpAction::None,
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
}
