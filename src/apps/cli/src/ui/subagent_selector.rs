/// Subagent selector popup for browsing and selecting subagents
///
/// Overlay popup that displays all available subagents
/// and allows the user to select one to fill the input box.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::responsive_popup::{render_too_small, responsive_popup, ResponsivePopup};
use crate::ui::theme::{StyleKind, Theme};

/// A subagent item for display in the selector
#[derive(Debug, Clone)]
pub(crate) struct SubagentItem {
    pub key: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String, // "builtin", "project", "user", or "external"
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum SubagentSelectorAction {
    ListSubagents,
    ConfigureSubagents,
    Launch(SubagentItem),
    Toggle(SubagentItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubagentSelectorScreen {
    Menu,
    List,
    Configure,
}

/// Subagent selector popup state
pub(super) struct SubagentSelectorState {
    items: Vec<SubagentItem>,
    list_state: ListState,
    visible: bool,
    last_area: Option<Rect>,
    interaction_enabled: bool,
    screen: SubagentSelectorScreen,
}

impl SubagentSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            last_area: None,
            interaction_enabled: true,
            screen: SubagentSelectorScreen::Menu,
        }
    }

    pub(super) fn show_menu(&mut self) {
        self.items.clear();
        self.screen = SubagentSelectorScreen::Menu;
        self.list_state.select(Some(0));
        self.visible = true;
        self.interaction_enabled = true;
    }

    /// Show subagents available to the current parent mode.
    pub(super) fn show_list(&mut self, subagents: Vec<SubagentItem>) {
        if subagents.is_empty() {
            return;
        }

        self.items = subagents;
        self.screen = SubagentSelectorScreen::List;
        self.list_state.select(Some(0));
        self.visible = true;
        self.interaction_enabled = true;
    }

    /// Show all discovered subagents with mode-specific enablement checkboxes.
    pub(super) fn show_config(&mut self, subagents: Vec<SubagentItem>) {
        if subagents.is_empty() {
            return;
        }

        let selected_key = if self.screen == SubagentSelectorScreen::Configure {
            self.list_state
                .selected()
                .and_then(|index| self.items.get(index))
                .map(|item| item.key.clone())
        } else {
            None
        };
        let selected_index = self.list_state.selected().unwrap_or(0);
        let next_index = selected_key
            .and_then(|key| subagents.iter().position(|item| item.key == key))
            .unwrap_or_else(|| selected_index.min(subagents.len().saturating_sub(1)));

        self.items = subagents;
        self.screen = SubagentSelectorScreen::Configure;
        self.list_state.select(Some(next_index));
        self.visible = true;
        self.interaction_enabled = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the subagent selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if self.screen == SubagentSelectorScreen::Menu || !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn move_up(&mut self) {
        if !self.visible || !self.interaction_enabled || self.len() == 0 {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let len = self.len();
        let next = (selected + len - 1) % len;
        self.list_state.select(Some(next));
    }

    pub(super) fn move_down(&mut self) {
        if !self.visible || !self.interaction_enabled || self.len() == 0 {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1) % self.len();
        self.list_state.select(Some(next));
    }

    /// Get the selected action.
    pub(super) fn confirm_selection(&self) -> Option<SubagentSelectorAction> {
        if !self.visible || !self.interaction_enabled {
            return None;
        }
        let idx = self.list_state.selected()?;
        match self.screen {
            SubagentSelectorScreen::Menu => match idx {
                0 => Some(SubagentSelectorAction::ListSubagents),
                1 => Some(SubagentSelectorAction::ConfigureSubagents),
                _ => None,
            },
            SubagentSelectorScreen::List => self
                .items
                .get(idx)
                .cloned()
                .map(SubagentSelectorAction::Launch),
            SubagentSelectorScreen::Configure => self
                .items
                .get(idx)
                .cloned()
                .map(SubagentSelectorAction::Toggle),
        }
    }

    fn len(&self) -> usize {
        match self.screen {
            SubagentSelectorScreen::Menu => 2,
            SubagentSelectorScreen::List | SubagentSelectorScreen::Configure => self.items.len(),
        }
    }

    /// Render the subagent selector popup as an overlay
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.len() == 0 {
            self.last_area = None;
            return;
        }

        let ideal_height = self.len() as u16 + 4;
        let layout = responsive_popup(area, 92, ideal_height, 20, 5);
        let popup_area = match layout {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.last_area = None;
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Subagents");
                return;
            }
        };
        self.interaction_enabled = true;
        self.last_area = Some(popup_area);

        let list_items = self.render_items(theme);
        let title = match self.screen {
            SubagentSelectorScreen::Menu => " Subagents ",
            SubagentSelectorScreen::List => " List Subagents (current mode) ",
            SubagentSelectorScreen::Configure => " Enable/Disable Subagents (Space/Enter Toggle) ",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(title);

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
    pub(super) fn handle_mouse_event(
        &mut self,
        mouse: &MouseEvent,
    ) -> Option<SubagentSelectorAction> {
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
        if index >= self.len() {
            return None;
        }

        Some(index)
    }

    fn render_items(&self, theme: &Theme) -> Vec<ListItem<'static>> {
        match self.screen {
            SubagentSelectorScreen::Menu => vec![
                ListItem::new(Line::from(vec![
                    Span::styled(
                        "List subagents",
                        theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "Show subagents available to the current mode",
                        theme.style(StyleKind::Muted),
                    ),
                ])),
                ListItem::new(Line::from(vec![
                    Span::styled(
                        "Enable/disable subagents",
                        theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "Toggle all discovered subagents for this mode",
                        theme.style(StyleKind::Muted),
                    ),
                ])),
            ],
            SubagentSelectorScreen::List => self
                .items
                .iter()
                .map(|subagent| self.render_subagent_line(subagent, theme, false))
                .collect(),
            SubagentSelectorScreen::Configure => self
                .items
                .iter()
                .map(|subagent| self.render_subagent_line(subagent, theme, true))
                .collect(),
        }
    }

    fn render_subagent_line(
        &self,
        subagent: &SubagentItem,
        theme: &Theme,
        include_checkbox: bool,
    ) -> ListItem<'static> {
        let source_marker = match subagent.source.as_str() {
            "builtin" => "B",
            "project" => "P",
            "user" => "U",
            "external" => "E",
            _ => "?",
        };
        let source_style = match subagent.source.as_str() {
            "builtin" => theme.style(StyleKind::Success),
            "project" => theme.style(StyleKind::Info),
            "external" => theme.style(StyleKind::Warning),
            _ => theme.style(StyleKind::Muted),
        };
        let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
        let desc_style = theme.style(StyleKind::Muted);

        let mut spans = Vec::new();
        if include_checkbox {
            spans.push(Span::styled(
                if subagent.enabled { "[x] " } else { "[ ] " },
                if subagent.enabled {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                },
            ));
        }
        spans.push(Span::styled(format!("[{}] ", source_marker), source_style));
        spans.push(Span::styled(subagent.name.clone(), name_style));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(subagent.description.clone(), desc_style));

        ListItem::new(Line::from(spans))
    }
}

#[cfg(test)]
mod tests {
    use super::{SubagentItem, SubagentSelectorState};
    use crate::ui::theme::Theme;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn too_small_fallback_disables_hidden_selection() {
        let mut state = SubagentSelectorState::new();
        state.show_list(vec![SubagentItem {
            key: "review".to_string(),
            id: "review".to_string(),
            name: "Review".to_string(),
            description: String::new(),
            source: "builtin".to_string(),
            enabled: true,
        }]);
        let mut terminal = Terminal::new(TestBackend::new(10, 3)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render tiny subagent selector");

        assert!(state.confirm_selection().is_none());
        state.move_down();
        assert!(state.confirm_selection().is_none());
    }
}
