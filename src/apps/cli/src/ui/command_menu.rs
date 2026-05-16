/// Slash command menu rendering and state

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::commands::{match_prefix_in, CommandSpec, COMMAND_SPECS};
use crate::ui::theme::{StyleKind, Theme};

pub struct CommandMenuState {
    items: Vec<&'static CommandSpec>,
    list_state: ListState,
    visible: bool,
    suppressed: bool,
    last_input: String,
    last_area: Option<Rect>,
}

impl CommandMenuState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            suppressed: false,
            last_input: String::new(),
            last_area: None,
        }
    }

    pub fn update(&mut self, input: &str, cursor: usize) {
        self.update_with_commands(input, cursor, COMMAND_SPECS);
    }

    pub fn update_with_commands(&mut self, input: &str, cursor: usize, commands: &'static [CommandSpec]) {
        if self.suppressed && input == self.last_input {
            return;
        }

        if self.suppressed && input != self.last_input {
            self.suppressed = false;
        }

        self.last_input = input.to_string();

        if !input.starts_with('/') || !self.cursor_in_command(input, cursor) {
            self.hide();
            return;
        }

        let query = input.split_whitespace().next().unwrap_or("");
        if query == "/" {
            self.items = commands.iter().collect();
        } else {
            self.items = match_prefix_in(query, commands);
        }
        self.items.sort_by_key(|spec| spec.name);

        self.visible = !self.items.is_empty();
        if self.visible {
            let selected = self.list_state.selected().unwrap_or(0);
            let clamped = selected.min(self.items.len().saturating_sub(1));
            self.list_state.select(Some(clamped));
        } else {
            self.list_state.select(None);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn move_up(&mut self) {
        if !self.visible {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = selected.saturating_sub(1);
        self.list_state.select(Some(next));
    }

    pub fn move_down(&mut self) {
        if !self.visible {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.items.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    /// Confirm the selected command and return its name
    pub fn apply_selection(&mut self) -> Option<String> {
        if !self.visible {
            return None;
        }

        let selected = self.selected_item()?;
        let command = selected.name.to_string();
        self.suppress();
        Some(command)
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || area.height < 3 {
            self.last_area = None;
            return;
        }

        let items: Vec<ListItem> = self.items.iter().map(|spec| {
            let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
            let desc_style = theme.style(StyleKind::Muted);
            let line = Line::from(vec![
                Span::styled(spec.name, name_style),
                Span::raw(" - "),
                Span::styled(spec.description, desc_style),
            ]);
            ListItem::new(line)
        }).collect();

        let desired_height = (items.len() as u16).saturating_add(2);
        let height = desired_height.min(area.height);
        if height < 3 {
            self.last_area = None;
            return;
        }

        let menu_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(height),
            width: area.width,
            height,
        };
        self.last_area = Some(menu_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Border))
            .style(Style::default().bg(theme.background))
            .title(" Commands ");

        let list = List::new(items)
            .block(block)
            .style(Style::default().bg(theme.background))
            .highlight_style(
                Style::default()
                    .bg(theme.primary)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut self.list_state);
    }

    /// Handle mouse events. Returns `Some(command_name)` when a command is clicked.
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<String> {
        if !self.visible {
            return None;
        }

        let area = match self.last_area {
            Some(area) => area,
            None => return None,
        };

        let in_menu = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_menu => {
                self.move_up();
                None
            }
            MouseEventKind::ScrollDown if in_menu => {
                self.move_down();
                None
            }
            MouseEventKind::Moved if in_menu => {
                if let Some(index) = self.item_index_at(mouse.column, mouse.row, area) {
                    self.list_state.select(Some(index));
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if in_menu => {
                if let Some(index) = self.item_index_at(mouse.column, mouse.row, area) {
                    self.list_state.select(Some(index));
                    return self.apply_selection();
                }
                None
            }
            _ => None,
        }
    }

    /// Whether the menu captures this mouse event (prevents passthrough)
    pub fn captures_mouse(&self, mouse: &MouseEvent) -> bool {
        if !self.visible {
            return false;
        }
        let Some(area) = self.last_area else {
            return false;
        };
        let in_menu = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);
        in_menu
    }

    fn selected_item(&self) -> Option<&CommandSpec> {
        let idx = self.list_state.selected().unwrap_or(0);
        self.items.get(idx).copied()
    }

    fn suppress(&mut self) {
        self.visible = false;
        self.suppressed = true;
        self.items.clear();
        self.list_state.select(None);
        self.last_area = None;
    }

    fn hide(&mut self) {
        self.visible = false;
        self.items.clear();
        self.list_state.select(None);
        self.last_area = None;
    }

    fn item_index_at(&self, column: u16, row: u16, area: Rect) -> Option<usize> {
        if area.width < 3 || area.height < 3 {
            return None;
        }

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if column < inner.x
            || column >= inner.x.saturating_add(inner.width)
            || row < inner.y
            || row >= inner.y.saturating_add(inner.height)
        {
            return None;
        }

        let index = row.saturating_sub(inner.y) as usize;
        if index >= self.items.len() {
            return None;
        }

        Some(index)
    }

    fn cursor_in_command(&self, input: &str, cursor: usize) -> bool {
        match input.chars().position(|c| c.is_whitespace()) {
            Some(space_idx) => cursor <= space_idx,
            None => true,
        }
    }
}
