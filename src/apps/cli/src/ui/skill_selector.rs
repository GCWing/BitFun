/// Skill selector popup for browsing and selecting skills
///
/// Overlay popup that displays all available skills
/// and allows the user to select one to fill the input box.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// A skill item for display in the selector
#[derive(Debug, Clone)]
pub struct SkillItem {
    pub name: String,
    pub description: String,
    pub level: String, // "project" or "user"
}

/// Skill selector popup state
pub struct SkillSelectorState {
    items: Vec<SkillItem>,
    list_state: ListState,
    visible: bool,
    last_area: Option<Rect>,
}

impl SkillSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            last_area: None,
        }
    }

    /// Show the skill selector with given skill list
    pub fn show(&mut self, skills: Vec<SkillItem>) {
        if skills.is_empty() {
            return;
        }

        self.items = skills;
        self.list_state.select(Some(0));
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the skill selector (for back navigation)
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

    /// Get the selected skill item
    pub fn confirm_selection(&self) -> Option<SkillItem> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected()?;
        self.items.get(idx).cloned()
    }

    /// Render the skill selector popup as an overlay
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.items.is_empty() {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(70);
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
            .map(|skill| {
                let level_marker = match skill.level.as_str() {
                    "project" => "P",
                    "user" => "U",
                    _ => "?",
                };
                let level_style = match skill.level.as_str() {
                    "project" => theme.style(StyleKind::Info),
                    _ => theme.style(StyleKind::Muted),
                };

                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let desc_style = theme.style(StyleKind::Muted);

                let line = Line::from(vec![
                    Span::styled(format!("[{}] ", level_marker), level_style),
                    Span::styled(&skill.name, name_style),
                    Span::raw("  "),
                    Span::styled(&skill.description, desc_style),
                ]);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Select Skill (↑↓ Navigate, Enter Select, Esc Cancel) ");

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
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<SkillItem> {
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
