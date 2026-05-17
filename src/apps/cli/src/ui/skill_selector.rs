/// Skill selector popup for browsing and selecting skills
///
/// Overlay popup that displays all available skills
/// and allows the user to select one to fill the input box.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// A skill item for display in the selector.
#[derive(Debug, Clone)]
pub struct SkillItem {
    pub key: String,
    pub name: String,
    pub description: String,
    pub level: String, // "project" or "user"
    pub enabled: bool,
    pub selected_for_runtime: bool,
    pub default_enabled: bool,
    pub is_shadowed: bool,
}

#[derive(Debug, Clone)]
pub enum SkillSelectorAction {
    ListSkills,
    ConfigureSkills,
    Execute(SkillItem),
    Toggle(SkillItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillSelectorScreen {
    Menu,
    List,
    Configure,
}

/// Skill selector popup state
pub struct SkillSelectorState {
    items: Vec<SkillItem>,
    list_state: ListState,
    visible: bool,
    last_area: Option<Rect>,
    screen: SkillSelectorScreen,
}

impl SkillSelectorState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            last_area: None,
            screen: SkillSelectorScreen::Menu,
        }
    }

    pub fn show_menu(&mut self) {
        self.items.clear();
        self.screen = SkillSelectorScreen::Menu;
        self.list_state.select(Some(0));
        self.visible = true;
    }

    /// Show the current mode's runtime-visible skills.
    pub fn show_list(&mut self, skills: Vec<SkillItem>) {
        if skills.is_empty() {
            return;
        }

        self.items = skills;
        self.screen = SkillSelectorScreen::List;
        self.list_state.select(Some(0));
        self.visible = true;
    }

    /// Show all discovered skills with mode-specific enablement checkboxes.
    pub fn show_config(&mut self, skills: Vec<SkillItem>) {
        if skills.is_empty() {
            return;
        }

        let selected_key = if self.screen == SkillSelectorScreen::Configure {
            self.list_state
                .selected()
                .and_then(|index| self.items.get(index))
                .map(|item| item.key.clone())
        } else {
            None
        };
        let selected_index = self.list_state.selected().unwrap_or(0);
        let next_index = selected_key
            .and_then(|key| skills.iter().position(|item| item.key == key))
            .unwrap_or_else(|| selected_index.min(skills.len().saturating_sub(1)));

        self.items = skills;
        self.screen = SkillSelectorScreen::Configure;
        self.list_state.select(Some(next_index));
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the skill selector (for back navigation)
    pub fn reshow(&mut self) {
        if self.screen == SkillSelectorScreen::Menu || !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn move_up(&mut self) {
        if !self.visible || self.len() == 0 {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = selected.saturating_sub(1);
        self.list_state.select(Some(next));
    }

    pub fn move_down(&mut self) {
        if !self.visible || self.len() == 0 {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    /// Get the selected action.
    pub fn confirm_selection(&self) -> Option<SkillSelectorAction> {
        if !self.visible {
            return None;
        }
        let idx = self.list_state.selected()?;
        match self.screen {
            SkillSelectorScreen::Menu => match idx {
                0 => Some(SkillSelectorAction::ListSkills),
                1 => Some(SkillSelectorAction::ConfigureSkills),
                _ => None,
            },
            SkillSelectorScreen::List => self
                .items
                .get(idx)
                .cloned()
                .map(SkillSelectorAction::Execute),
            SkillSelectorScreen::Configure => self
                .items
                .get(idx)
                .cloned()
                .map(SkillSelectorAction::Toggle),
        }
    }

    fn len(&self) -> usize {
        match self.screen {
            SkillSelectorScreen::Menu => 2,
            SkillSelectorScreen::List | SkillSelectorScreen::Configure => self.items.len(),
        }
    }

    /// Render the skill selector popup as an overlay
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.len() == 0 {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(92);
        let popup_height = (self.len() as u16 + 4).min(area.height.saturating_sub(2));
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

        let list_items = self.render_items(theme);
        let title = match self.screen {
            SkillSelectorScreen::Menu => " Skills ",
            SkillSelectorScreen::List => " List Skills (current mode) ",
            SkillSelectorScreen::Configure => " Enable/Disable Skills (Space/Enter Toggle) ",
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
    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<SkillSelectorAction> {
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
        if index >= self.len() {
            return None;
        }

        Some(index)
    }

    fn render_items(&self, theme: &Theme) -> Vec<ListItem<'static>> {
        match self.screen {
            SkillSelectorScreen::Menu => vec![
                ListItem::new(Line::from(vec![
                    Span::styled(
                        "List skills",
                        theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "Show skills available to the current mode",
                        theme.style(StyleKind::Muted),
                    ),
                ])),
                ListItem::new(Line::from(vec![
                    Span::styled(
                        "Enable/disable skills",
                        theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        "Toggle all discovered skills for this mode",
                        theme.style(StyleKind::Muted),
                    ),
                ])),
            ],
            SkillSelectorScreen::List => self
                .items
                .iter()
                .map(|skill| self.render_skill_line(skill, theme, false))
                .collect(),
            SkillSelectorScreen::Configure => self
                .items
                .iter()
                .map(|skill| self.render_skill_line(skill, theme, true))
                .collect(),
        }
    }

    fn render_skill_line(
        &self,
        skill: &SkillItem,
        theme: &Theme,
        include_checkbox: bool,
    ) -> ListItem<'static> {
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
        let status = if skill.is_shadowed {
            " shadowed"
        } else if include_checkbox && skill.enabled && !skill.selected_for_runtime {
            " enabled"
        } else {
            ""
        };

        let mut spans = Vec::new();
        if include_checkbox {
            spans.push(Span::styled(
                if skill.enabled { "[x] " } else { "[ ] " },
                if skill.enabled {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                },
            ));
        }
        spans.push(Span::styled(format!("[{}] ", level_marker), level_style));
        spans.push(Span::styled(skill.name.clone(), name_style));
        if !status.is_empty() {
            spans.push(Span::styled(
                status.to_string(),
                theme.style(StyleKind::Muted),
            ));
        }
        spans.push(Span::raw("  "));
        spans.push(Span::styled(skill.description.clone(), desc_style));

        ListItem::new(Line::from(spans))
    }
}
