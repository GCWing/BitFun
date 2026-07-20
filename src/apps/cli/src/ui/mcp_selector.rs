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
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::ui::theme::{StyleKind, Theme};

fn wrap_confirmation_detail(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for section in value.split("; ") {
        if section.is_empty() {
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0;
        for character in section.chars() {
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if !line.is_empty() && line_width + character_width > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push(character);
            line_width += character_width;
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

fn confirmation_review_lines(name: &str, detail: &str, width: usize) -> Vec<String> {
    let mut lines = vec![format!("Review external MCP server: {name}"), String::new()];
    lines.extend(wrap_confirmation_detail(detail, width));
    lines
}

fn confirmation_max_scroll(line_count: usize, popup_height: u16) -> u16 {
    let visible_detail_height = popup_height.saturating_sub(3).max(1) as usize;
    line_count.saturating_sub(visible_detail_height) as u16
}

/// An MCP server item for display in the selector
#[derive(Debug, Clone)]
pub(crate) struct McpItem {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub status: String,
    pub tool_count: usize,
    pub source_label: String,
    pub external: bool,
    pub detail: String,
    pub action: McpItemAction,
}

#[derive(Debug, Clone)]
pub(crate) enum McpItemAction {
    NativeToggle,
    ExternalDecision {
        candidate_id: String,
        decision_key: String,
        approved: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
    ConflictChoice {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
    ReadOnly {
        reason: String,
    },
}

impl McpItem {
    pub(crate) fn is_external(&self) -> bool {
        self.external
    }

    pub(crate) fn requires_external_confirmation(&self) -> bool {
        matches!(
            self.action,
            McpItemAction::ExternalDecision { approved: true, .. }
                | McpItemAction::ConflictChoice {
                    approve_external: true,
                    ..
                }
        )
    }
}

/// Action returned from the MCP selector
#[derive(Debug, Clone)]
pub(crate) enum McpAction {
    /// Toggle (start/stop) the selected server
    Toggle(McpItem),
    /// No action (dismiss)
    None,
}

/// MCP selector popup state
pub(super) struct McpSelectorState {
    items: Vec<McpItem>,
    list_state: ListState,
    visible: bool,
    /// Which server is currently being toggled (loading indicator)
    pub(super) loading_id: Option<String>,
    /// Server ID pending delete confirmation (double-tap 'd' to confirm)
    confirm_delete_id: Option<String>,
    confirm_external_id: Option<String>,
    confirmation_scroll: u16,
    confirmation_max_scroll: u16,
    confirmation_reviewed: bool,
    last_area: Option<Rect>,
}

impl McpSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            loading_id: None,
            confirm_delete_id: None,
            confirm_external_id: None,
            confirmation_scroll: 0,
            confirmation_max_scroll: 0,
            confirmation_reviewed: false,
            last_area: None,
        }
    }

    /// Show the MCP selector with given server list
    pub(super) fn show(&mut self, items: Vec<McpItem>) {
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
    pub(super) fn update_items(&mut self, items: Vec<McpItem>) {
        let selected_idx = self.list_state.selected().unwrap_or(0);
        let selected_id = self
            .list_state
            .selected()
            .and_then(|index| self.items.get(index))
            .map(|item| item.id.clone());
        self.items = items;
        if self.items.is_empty() {
            self.list_state.select(None);
        } else if let Some(index) = selected_id
            .as_ref()
            .and_then(|id| self.items.iter().position(|item| &item.id == id))
        {
            self.list_state.select(Some(index));
        } else {
            self.list_state
                .select(Some(selected_idx.min(self.items.len().saturating_sub(1))));
        }
        let confirm_delete_removed = self
            .confirm_delete_id
            .as_deref()
            .is_some_and(|id| !self.items.iter().any(|item| item.id == id));
        let confirm_external_removed = self
            .confirm_external_id
            .as_deref()
            .is_some_and(|id| !self.items.iter().any(|item| item.id == id));
        let loading_removed = self
            .loading_id
            .as_deref()
            .is_some_and(|id| !self.items.iter().any(|item| item.id == id));
        if confirm_delete_removed {
            self.confirm_delete_id = None;
        }
        if confirm_external_removed {
            self.cancel_confirm_external();
        }
        if loading_removed {
            self.loading_id = None;
        }
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.loading_id = None;
        self.confirm_delete_id = None;
        self.cancel_confirm_external();
        self.last_area = None;
    }

    /// Reshow the MCP selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    /// Enter confirm-delete mode for a server
    pub(super) fn start_confirm_delete(&mut self, server_id: String) {
        self.confirm_delete_id = Some(server_id);
    }

    /// Cancel confirm-delete mode
    pub(super) fn cancel_confirm_delete(&mut self) {
        self.confirm_delete_id = None;
    }

    /// Check if a server is in confirm-delete mode
    pub(super) fn is_confirm_delete(&self, server_id: &str) -> bool {
        self.confirm_delete_id.as_deref() == Some(server_id)
    }

    pub(super) fn start_confirm_external(&mut self, server_id: String) {
        self.confirm_delete_id = None;
        self.confirm_external_id = Some(server_id);
        self.confirmation_scroll = 0;
        self.confirmation_max_scroll = 0;
        self.confirmation_reviewed = false;
    }

    pub(super) fn is_confirm_external(&self, server_id: &str) -> bool {
        self.confirm_external_id.as_deref() == Some(server_id)
    }

    pub(super) fn cancel_confirm_external(&mut self) {
        self.confirm_external_id = None;
        self.confirmation_scroll = 0;
        self.confirmation_max_scroll = 0;
        self.confirmation_reviewed = false;
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn move_up(&mut self) {
        if !self.visible || self.items.is_empty() {
            return;
        }
        if self.confirm_external_id.is_some() {
            self.confirmation_scroll = self.confirmation_scroll.saturating_sub(1);
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let len = self.items.len();
        let next = (selected + len - 1) % len;
        self.list_state.select(Some(next));
    }

    pub(super) fn move_down(&mut self) {
        if !self.visible || self.items.is_empty() {
            return;
        }
        if self.confirm_external_id.is_some() {
            self.confirmation_scroll = self
                .confirmation_scroll
                .saturating_add(1)
                .min(self.confirmation_max_scroll);
            if self.confirmation_scroll >= self.confirmation_max_scroll {
                self.confirmation_reviewed = true;
            }
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1) % self.items.len();
        self.list_state.select(Some(next));
    }

    /// Get the selected MCP item (for toggle action)
    pub(super) fn confirm_selection(&self) -> Option<McpItem> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected()?;
        if self.confirm_external_id.is_some() && !self.confirmation_reviewed {
            return None;
        }
        self.items.get(idx).cloned()
    }

    /// Render the MCP selector popup as an overlay
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(72);
        // +5 for border(2) + title(1) + hint(1) + padding(1)
        let confirmation_height = self
            .confirm_external_id
            .as_ref()
            .and_then(|id| self.items.iter().find(|item| &item.id == id))
            .map(|item| {
                wrap_confirmation_detail(&item.detail, popup_width.saturating_sub(6) as usize).len()
                    as u16
                    + 2
            })
            .unwrap_or(0);
        let max_popup_height = area.height.saturating_sub(2);
        if max_popup_height < 6 || popup_width < 30 {
            self.last_area = None;
            return;
        }
        let popup_height = (self.items.len() as u16 + 5 + confirmation_height)
            .min(max_popup_height)
            .max(6);

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        self.last_area = Some(popup_area);

        if let Some(item) = self
            .confirm_external_id
            .as_ref()
            .and_then(|id| self.items.iter().find(|item| &item.id == id))
            .cloned()
        {
            let content_width = popup_width.saturating_sub(4).max(1) as usize;
            let review_lines = confirmation_review_lines(&item.name, &item.detail, content_width);
            let visible_detail_height = popup_height.saturating_sub(3).max(1) as usize;
            let max_scroll = confirmation_max_scroll(review_lines.len(), popup_height);
            self.confirmation_max_scroll = max_scroll;
            self.confirmation_scroll = self.confirmation_scroll.min(max_scroll);
            if max_scroll == 0 || self.confirmation_scroll >= max_scroll {
                self.confirmation_reviewed = true;
            }
            let start = self.confirmation_scroll as usize;
            let end = (start + visible_detail_height).min(review_lines.len());
            let mut visible_lines = review_lines[start..end]
                .iter()
                .map(|line| Line::from(Span::styled(line.clone(), theme.style(StyleKind::Warning))))
                .collect::<Vec<_>>();
            let footer = if self.confirmation_reviewed {
                "Enter:Approve and use  Up/Down:Review  Esc:Cancel"
            } else {
                "Up/Down:Review all details  Esc:Cancel"
            };
            visible_lines.push(Line::from(Span::styled(
                footer,
                theme.style(StyleKind::Muted),
            )));
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme.style(StyleKind::Warning))
                .style(Style::default().bg(theme.background))
                .title(" Review External MCP Server ");
            let list = List::new(vec![ListItem::new(visible_lines)])
                .block(block)
                .style(Style::default().bg(theme.background));
            frame.render_widget(Clear, popup_area);
            frame.render_widget(list, popup_area);
            return;
        }

        let loading_id = self.loading_id.clone();
        let confirm_delete_id = self.confirm_delete_id.clone();
        let has_confirm_delete = confirm_delete_id.is_some();

        let mut list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let is_loading = loading_id.as_ref().is_some_and(|id| id == &item.id);
                let is_confirm_delete = confirm_delete_id.as_ref().is_some_and(|id| id == &item.id);

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
                    Span::styled(
                        format!("  [{}]", item.source_label),
                        theme.style(StyleKind::Muted),
                    ),
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
        let selected_external = self
            .list_state
            .selected()
            .and_then(|index| self.items.get(index))
            .is_some_and(McpItem::is_external);
        let hint_text = if has_confirm_delete {
            " d:Confirm Delete  Any key:Cancel"
        } else if selected_external {
            " Enter:Review or change  External settings are read-only  Esc:Close"
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
                    .fg(theme.selection_foreground())
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.list_state);
    }

    /// Handle mouse events
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> McpAction {
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
                if self.confirm_external_id.is_some() {
                    return McpAction::None;
                }
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                    if let Some(item) = self.confirm_selection() {
                        if item.requires_external_confirmation()
                            && !self.is_confirm_external(&item.id)
                        {
                            self.start_confirm_external(item.id);
                            return McpAction::None;
                        }
                        self.cancel_confirm_external();
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

    pub(super) fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        confirmation_max_scroll, confirmation_review_lines, wrap_confirmation_detail, McpItem,
        McpItemAction, McpSelectorState,
    };
    use unicode_width::UnicodeWidthStr;

    fn item(action: McpItemAction, external: bool) -> McpItem {
        McpItem {
            id: "github".to_string(),
            name: "github".to_string(),
            server_type: "local".to_string(),
            status: "Confirmation required".to_string(),
            tool_count: 0,
            source_label: if external { "OpenCode" } else { "BitFun" }.to_string(),
            external,
            detail: "Safe summary".to_string(),
            action,
        }
    }

    #[test]
    fn enabling_an_external_server_requires_a_second_explicit_confirmation() {
        let item = item(
            McpItemAction::ExternalDecision {
                candidate_id: "candidate".to_string(),
                decision_key: "decision".to_string(),
                approved: true,
                expected_mcp_generation: 3,
                expected_preference_revision: 7,
            },
            true,
        );

        assert!(item.requires_external_confirmation());

        let mut state = McpSelectorState::new();
        state.show(vec![item]);
        state.start_confirm_external("github".to_string());
        assert!(state.is_confirm_external("github"));
        state.cancel_confirm_external();
        assert!(!state.is_confirm_external("github"));
    }

    #[test]
    fn native_toggle_and_external_disable_do_not_add_extra_confirmation() {
        assert!(!item(McpItemAction::NativeToggle, false).requires_external_confirmation());
        assert!(!item(
            McpItemAction::ExternalDecision {
                candidate_id: "candidate".to_string(),
                decision_key: "decision".to_string(),
                approved: false,
                expected_mcp_generation: 3,
                expected_preference_revision: 7,
            },
            true,
        )
        .requires_external_confirmation());
    }

    #[test]
    fn long_external_confirmation_requires_reviewing_to_the_end() {
        let external = item(
            McpItemAction::ExternalDecision {
                candidate_id: "candidate".to_string(),
                decision_key: "decision".to_string(),
                approved: true,
                expected_mcp_generation: 3,
                expected_preference_revision: 7,
            },
            true,
        );
        let mut state = McpSelectorState::new();
        state.show(vec![external]);
        state.start_confirm_external("github".to_string());
        state.confirmation_max_scroll = 2;
        assert!(state.confirm_selection().is_none());
        state.move_down();
        assert!(state.confirm_selection().is_none());
        state.move_down();
        assert!(state.confirm_selection().is_some());
    }

    #[test]
    fn confirmation_wrap_uses_terminal_width_for_cjk_content() {
        let lines = wrap_confirmation_detail("来源：项目配置；命令：测试工具", 10);
        assert!(lines.len() > 1);
        assert!(lines
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= 10));
    }

    #[test]
    fn forty_by_twenty_terminal_can_review_short_and_scroll_long_summaries() {
        // A 40x20 terminal yields the selector's 36x18 popup after margins.
        let short = confirmation_review_lines("server", "Safe summary", 32);
        assert_eq!(confirmation_max_scroll(short.len(), 18), 0);

        let long = confirmation_review_lines("server", &"header: value; ".repeat(80), 32);
        assert!(confirmation_max_scroll(long.len(), 18) > 0);
    }

    #[test]
    fn live_updates_preserve_selection_by_stable_id_and_clear_removed_state() {
        let mut first = item(McpItemAction::NativeToggle, false);
        first.id = "first".to_string();
        let mut selected = item(McpItemAction::NativeToggle, false);
        selected.id = "selected".to_string();
        let mut state = McpSelectorState::new();
        state.show(vec![first.clone(), selected.clone()]);
        state.move_down();
        state.loading_id = Some("selected".to_string());
        state.start_confirm_external("selected".to_string());

        let mut inserted = item(McpItemAction::NativeToggle, false);
        inserted.id = "inserted".to_string();
        state.update_items(vec![inserted, first, selected]);
        assert_eq!(
            state
                .list_state
                .selected()
                .and_then(|index| state.items.get(index))
                .map(|item| item.id.as_str()),
            Some("selected")
        );

        state.update_items(Vec::new());
        assert!(state.list_state.selected().is_none());
        assert!(state.loading_id.is_none());
        assert!(state.confirm_external_id.is_none());
    }
}
