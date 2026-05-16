/// Theme selector popup for choosing a UI theme
///
/// Overlay popup that displays all available themes and allows the user to select one.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

#[derive(Debug, Clone)]
pub struct ThemeItem {
    pub id: String,
}

pub struct ThemeSelectorState {
    items: Vec<ThemeItem>,
    list_state: ListState,
    visible: bool,
    current_theme_id: Option<String>,
    last_area: Option<Rect>,
}

impl ThemeSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            current_theme_id: None,
            last_area: None,
        }
    }

    pub fn show(&mut self, themes: Vec<ThemeItem>, current_theme_id: Option<String>) {
        if themes.is_empty() {
            return;
        }

        let initial_idx = current_theme_id
            .as_ref()
            .and_then(|id| themes.iter().position(|t| t.id == *id))
            .unwrap_or(0);

        self.items = themes;
        self.current_theme_id = current_theme_id;
        self.list_state.select(Some(initial_idx));
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the theme selector (for back navigation)
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

    pub fn selected_item(&self) -> Option<&ThemeItem> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        self.items.get(idx)
    }

    pub fn confirm_selection(&self) -> Option<ThemeItem> {
        self.selected_item().cloned()
    }

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
            .map(|t| {
                let is_current = self
                    .current_theme_id
                    .as_ref()
                    .map_or(false, |id| id == &t.id);

                let marker = if is_current { "● " } else { "  " };
                let marker_style = if is_current {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                };

                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let line = Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(&t.id, name_style),
                ]);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Select Theme (↑↓ Navigate, Enter Select, Esc Cancel) ");

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

    pub fn captures_mouse(&self, mouse: &MouseEvent) -> bool {
        if !self.visible {
            return false;
        }
        let Some(area) = self.last_area else {
            return false;
        };
        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);
        in_popup
    }

    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<ThemeItem> {
        if !self.visible {
            return None;
        }

        let area = self.last_area?;
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
                if let Some(index) = item_index_at(mouse.column, mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = item_index_at(mouse.column, mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                None
            }
            _ => None,
        }
    }
}

fn item_index_at(col: u16, row: u16, area: Rect) -> Option<usize> {
    // Inside the block, items start at y+1 (border) and go down.
    if row <= area.y || row >= area.y.saturating_add(area.height) {
        return None;
    }
    let inner_y = row.saturating_sub(area.y + 1) as usize;
    let _ = col;
    Some(inner_y)
}
