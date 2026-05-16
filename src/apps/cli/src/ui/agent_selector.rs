/// Agent selector popup for switching agent mode
///
/// Overlay popup that displays all available agent modes
/// and allows the user to select one to switch to.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// An agent item for display in the selector
#[derive(Debug, Clone)]
pub struct AgentItem {
    pub id: String,
    pub description: String,
}

/// Agent selector popup state
pub struct AgentSelectorState {
    items: Vec<AgentItem>,
    list_state: ListState,
    visible: bool,
    /// Currently active agent ID (for highlighting)
    current_agent_id: Option<String>,
    last_area: Option<Rect>,
}

impl AgentSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            current_agent_id: None,
            last_area: None,
        }
    }

    /// Show the agent selector with given agent list
    pub fn show(&mut self, agents: Vec<AgentItem>, current_agent_id: Option<String>) {
        if agents.is_empty() {
            return;
        }

        let initial_idx = current_agent_id
            .as_ref()
            .and_then(|id| agents.iter().position(|a| a.id == *id))
            .unwrap_or(0);

        self.items = agents;
        self.current_agent_id = current_agent_id;
        self.list_state.select(Some(initial_idx));
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the agent selector (for back navigation)
    pub fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
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

    /// Get the selected agent item
    pub fn confirm_selection(&self) -> Option<AgentItem> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected()?;
        self.items.get(idx).cloned()
    }

    /// Render the agent selector popup as an overlay
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.items.is_empty() {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(60);
        let popup_height = (self.items.len() as u16 + 4).min(area.height.saturating_sub(2));
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

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|agent| {
                let is_current = self
                    .current_agent_id
                    .as_ref()
                    .map_or(false, |id| id == &agent.id);

                let marker = if is_current { "● " } else { "  " };
                let marker_style = if is_current {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                };

                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let desc_style = theme.style(StyleKind::Muted);

                let line = Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(&agent.id, name_style),
                    Span::raw("  "),
                    Span::styled(&agent.description, desc_style),
                ]);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Select Agent (↑↓ Navigate, Enter Select, Esc Cancel) ");

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
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<AgentItem> {
        if !self.visible {
            return None;
        }

        let area = match self.last_area {
            Some(area) => area,
            None => return None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.move_up();
                None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.move_down();
                None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    self.list_state.select(Some(index));
                    return self.confirm_selection();
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                None
            }
            _ => None,
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
