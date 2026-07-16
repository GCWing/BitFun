/// Slash command menu rendering and state
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::actions::{slash_actions, ActionProjection, ActionState};
use crate::ui::theme::{StyleKind, Theme};

pub(super) struct CommandMenuState {
    action_state: ActionState,
    items: Vec<ActionProjection>,
    list_state: ListState,
    visible: bool,
    suppressed: bool,
    last_input: String,
    last_area: Option<Rect>,
}

impl CommandMenuState {
    pub(super) fn new(action_state: ActionState) -> Self {
        Self {
            action_state,
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            suppressed: false,
            last_input: String::new(),
            last_area: None,
        }
    }

    pub(super) fn update(&mut self, input: &str, cursor: usize) {
        if self.suppressed && input == self.last_input {
            return;
        }

        if self.suppressed && input != self.last_input {
            self.suppressed = false;
        }

        self.last_input = input.to_string();
        let selected_id = self.selected_item().map(|item| item.id.to_string());

        if !input.starts_with('/') || !self.cursor_in_command(input, cursor) {
            self.hide();
            return;
        }

        let query = input.split_whitespace().next().unwrap_or("");
        let mut commands = slash_actions(self.action_state);
        if query == "/" {
            self.items = commands;
        } else {
            let normalized = query
                .strip_prefix('/')
                .unwrap_or(query)
                .to_ascii_lowercase();
            commands.retain(|spec| {
                spec.name
                    .strip_prefix('/')
                    .unwrap_or(spec.name)
                    .to_ascii_lowercase()
                    .contains(&normalized)
            });
            self.items = commands;
        }
        self.items.sort_by_key(|spec| spec.name);

        self.visible = !self.items.is_empty();
        if self.visible {
            let selected = selected_id
                .and_then(|id| self.items.iter().position(|item| item.id == id))
                .unwrap_or_else(|| {
                    self.list_state
                        .selected()
                        .unwrap_or(0)
                        .min(self.items.len().saturating_sub(1))
                });
            self.list_state.select(Some(selected));
        } else {
            self.list_state.select(None);
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn move_up(&mut self) {
        if !self.visible {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let len = self.items.len();
        let next = (selected + len - 1) % len;
        self.list_state.select(Some(next));
    }

    pub(super) fn move_down(&mut self) {
        if !self.visible {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1) % self.items.len();
        self.list_state.select(Some(next));
    }

    /// Confirm the selected command and return its name
    pub(super) fn apply_selection(&mut self) -> Option<String> {
        if !self.visible {
            return None;
        }

        let selected = self.selected_item()?;
        let command = selected.id.to_string();
        self.suppress();
        Some(command)
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || area.height < 3 {
            self.last_area = None;
            return;
        }

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|spec| {
                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                let desc_style = theme.style(StyleKind::Muted);
                let line = Line::from(vec![
                    Span::styled(spec.name, name_style),
                    Span::raw(" - "),
                    Span::styled(spec.description, desc_style),
                ]);
                ListItem::new(line)
            })
            .collect();

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
                    .fg(theme.selection_foreground())
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut self.list_state);
    }

    /// Handle mouse events. Returns `Some(command_name)` when a command is clicked.
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<String> {
        if !self.visible {
            return None;
        }

        let area = self.last_area?;

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
    pub(super) fn captures_mouse(&self, mouse: &MouseEvent) -> bool {
        if !self.visible {
            return false;
        }
        let Some(area) = self.last_area else {
            return false;
        };
        mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height)
    }

    fn selected_item(&self) -> Option<&ActionProjection> {
        let idx = self.list_state.selected().unwrap_or(0);
        self.items.get(idx)
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

    pub(super) fn set_action_state(&mut self, action_state: ActionState) -> bool {
        if self.action_state == action_state {
            return false;
        }
        self.action_state = action_state;
        true
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn names(menu: &CommandMenuState) -> Vec<&str> {
        menu.items.iter().map(|item| item.name).collect()
    }

    #[test]
    fn chat_menu_keeps_substring_matching() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.update("/he", 3);

        assert_eq!(names(&menu), ["/help", "/theme"]);
    }

    #[test]
    fn slash_lists_all_actions_for_the_current_context() {
        let mut chat = CommandMenuState::new(ActionState::chat(false, false));
        chat.update("/", 1);
        assert!(names(&chat).contains(&"/clear"));
        assert!(names(&chat).contains(&"/new"));

        let mut startup = CommandMenuState::new(ActionState::startup(false));
        startup.update("/", 1);
        assert!(!names(&startup).contains(&"/clear"));
        assert!(!names(&startup).contains(&"/new"));
        assert!(names(&startup).contains(&"/sessions"));
    }

    #[test]
    fn processing_chat_hides_idle_only_actions() {
        let mut menu = CommandMenuState::new(ActionState::chat(true, false));
        menu.update("/", 1);

        assert!(!names(&menu).contains(&"/agents"));
        assert!(!names(&menu).contains(&"/new"));
        assert!(names(&menu).contains(&"/help"));
    }

    #[test]
    fn selection_returns_the_stable_action_id() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.update("/help", 5);

        assert_eq!(menu.apply_selection().as_deref(), Some("help"));
    }

    #[test]
    fn mouse_selection_returns_the_stable_action_id() {
        let mut menu = CommandMenuState::new(ActionState::startup(false));
        menu.update("/help", 5);
        menu.last_area = Some(Rect::new(5, 5, 30, 3));

        let selected = menu.handle_mouse_event(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 6,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(selected.as_deref(), Some("help"));
    }

    #[test]
    fn state_refresh_preserves_the_selected_action_id() {
        let mut menu = CommandMenuState::new(ActionState::chat(true, false));
        menu.update("/", 1);
        let logout_index = menu
            .items
            .iter()
            .position(|item| item.id == "logout")
            .unwrap();
        menu.list_state.select(Some(logout_index));

        assert!(menu.set_action_state(ActionState::chat(false, false)));
        menu.update("/", 1);

        assert_eq!(menu.selected_item().map(|item| item.id), Some("logout"));
    }
}
