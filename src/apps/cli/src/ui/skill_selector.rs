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
use std::collections::HashMap;

use crate::ui::{
    responsive_popup::{render_too_small, responsive_popup, ResponsivePopup},
    theme::{StyleKind, Theme},
};

/// A skill item for display in the selector.
#[derive(Debug, Clone)]
pub(crate) struct SkillItem {
    pub key: String,
    pub name: String,
    pub description: String,
    pub level: String, // "project" or "user"
    pub source_slot: String,
    pub source_label: String,
    pub enabled: bool,
    pub selected_for_runtime: bool,
    pub default_enabled: bool,
    pub is_shadowed: bool,
    pub shadowed_by_key: Option<String>,
}

impl SkillItem {
    fn display_source_label(&self) -> &str {
        let label = self.source_label.trim();
        if !label.is_empty() {
            return label;
        }

        match self.source_slot.trim().trim_start_matches("home.") {
            "bitfun" | "bitfun-system" => "BitFun",
            "claude" => "Claude Code",
            "codex" => "Codex",
            "cursor" => "Cursor",
            "opencode" | "config.opencode" => "OpenCode",
            "agents" => "Agent Skills",
            _ => "Other source",
        }
    }
}

fn build_coverage_source_map(items: &[SkillItem]) -> HashMap<String, String> {
    let source_by_key: HashMap<&str, &str> = items
        .iter()
        .map(|item| (item.key.as_str(), item.display_source_label()))
        .collect();

    items
        .iter()
        .filter_map(|item| {
            let winner_key = item.shadowed_by_key.as_deref()?;
            let winner_source = source_by_key.get(winner_key)?;
            Some((item.key.clone(), (*winner_source).to_string()))
        })
        .collect()
}

fn skill_checkbox_marker(skill: &SkillItem) -> &'static str {
    if !skill.enabled {
        "[ ] "
    } else if skill.selected_for_runtime {
        "[x] "
    } else {
        "[~] "
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SkillSelectorAction {
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
pub(super) struct SkillSelectorState {
    items: Vec<SkillItem>,
    coverage_source_by_key: HashMap<String, String>,
    list_state: ListState,
    visible: bool,
    last_area: Option<Rect>,
    interaction_enabled: bool,
    screen: SkillSelectorScreen,
}

impl SkillSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            coverage_source_by_key: HashMap::new(),
            list_state: ListState::default(),
            visible: false,
            last_area: None,
            interaction_enabled: true,
            screen: SkillSelectorScreen::Menu,
        }
    }

    pub(super) fn show_menu(&mut self) {
        self.items.clear();
        self.coverage_source_by_key.clear();
        self.screen = SkillSelectorScreen::Menu;
        self.list_state.select(Some(0));
        self.visible = true;
        self.interaction_enabled = true;
    }

    /// Show the current mode's runtime-visible skills.
    pub(super) fn show_list(&mut self, skills: Vec<SkillItem>) {
        if skills.is_empty() {
            return;
        }

        self.coverage_source_by_key = build_coverage_source_map(&skills);
        self.items = skills;
        self.screen = SkillSelectorScreen::List;
        self.list_state.select(Some(0));
        self.visible = true;
        self.interaction_enabled = true;
    }

    /// Show all discovered skills with mode-specific enablement checkboxes.
    pub(super) fn show_config(&mut self, skills: Vec<SkillItem>) {
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

        self.coverage_source_by_key = build_coverage_source_map(&skills);
        self.items = skills;
        self.screen = SkillSelectorScreen::Configure;
        self.list_state.select(Some(next_index));
        self.visible = true;
        self.interaction_enabled = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the skill selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if self.screen == SkillSelectorScreen::Menu || !self.items.is_empty() {
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
    pub(super) fn confirm_selection(&self) -> Option<SkillSelectorAction> {
        if !self.visible || !self.interaction_enabled {
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
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.len() == 0 {
            self.last_area = None;
            return;
        }

        let initial_layout = responsive_popup(area, 92, self.len() as u16 + 4, 20, 5);
        let initial_area = match initial_layout {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.last_area = None;
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Skills");
                return;
            }
        };
        let row_height = self.row_height(initial_area.width);
        let ideal_height = self.len() as u16 * row_height + 4;
        let layout = responsive_popup(area, 92, ideal_height, 20, 5);
        let popup_area = match layout {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.last_area = None;
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Skills");
                return;
            }
        };
        self.interaction_enabled = true;
        self.last_area = Some(popup_area);

        let content_width = popup_area.width.saturating_sub(2);
        let list_items = self.render_items(theme, content_width);
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
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<SkillSelectorAction> {
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
        let row_height = self.row_height(area.width);
        let index = ((row - inner_y) / row_height) as usize + offset;
        if index >= self.len() {
            return None;
        }

        Some(index)
    }

    fn row_height(&self, popup_width: u16) -> u16 {
        match self.screen {
            SkillSelectorScreen::Menu => 1,
            SkillSelectorScreen::List | SkillSelectorScreen::Configure
                if popup_width.saturating_sub(2) < 40 =>
            {
                2
            }
            SkillSelectorScreen::List | SkillSelectorScreen::Configure => 1,
        }
    }

    fn render_items(&self, theme: &Theme, content_width: u16) -> Vec<ListItem<'static>> {
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
                .map(|skill| self.render_skill_line(skill, theme, false, content_width))
                .collect(),
            SkillSelectorScreen::Configure => self
                .items
                .iter()
                .map(|skill| self.render_skill_line(skill, theme, true, content_width))
                .collect(),
        }
    }

    fn render_skill_line(
        &self,
        skill: &SkillItem,
        theme: &Theme,
        include_checkbox: bool,
        content_width: u16,
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
        let name_style = if skill.is_shadowed {
            theme
                .style(StyleKind::Muted)
                .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT)
        } else {
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
        };
        let desc_style = theme.style(StyleKind::Muted);
        let compact = content_width < 40;
        let status = if skill.is_shadowed {
            self.coverage_source_by_key
                .get(&skill.key)
                .map(|source| {
                    if compact {
                        format!(" < {}", compact_source_label(source))
                    } else {
                        format!(" covered by {}", source)
                    }
                })
                .unwrap_or_else(|| " covered".to_string())
        } else if include_checkbox && skill.enabled && skill.selected_for_runtime {
            " active".to_string()
        } else if include_checkbox && skill.enabled {
            " enabled, not selected".to_string()
        } else {
            String::new()
        };

        let mut spans = Vec::new();
        if include_checkbox {
            let checkbox_style = if !skill.enabled {
                theme.style(StyleKind::Muted)
            } else if skill.selected_for_runtime {
                theme.style(StyleKind::Success)
            } else {
                theme.style(StyleKind::Warning)
            };
            spans.push(Span::styled(skill_checkbox_marker(skill), checkbox_style));
        }
        if !compact {
            spans.push(Span::styled(
                format!("[{}/{}] ", level_marker, skill.display_source_label()),
                level_style,
            ));
        }
        spans.push(Span::styled(skill.name.clone(), name_style));
        if !status.is_empty() {
            spans.push(Span::styled(status, theme.style(StyleKind::Muted)));
        }
        if !compact {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(skill.description.clone(), desc_style));
        }

        if compact {
            let scope = match skill.level.as_str() {
                "project" => "project",
                "user" => "user",
                _ => "other",
            };
            return ListItem::new(vec![
                Line::from(spans),
                Line::from(Span::styled(
                    format!(
                        "  {scope} · {}",
                        compact_source_label(skill.display_source_label())
                    ),
                    level_style,
                )),
            ]);
        }

        ListItem::new(Line::from(spans))
    }
}

fn compact_source_label(source: &str) -> &str {
    match source {
        "Claude Code" => "Claude",
        "Agent Skills" => "Agents",
        _ => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn skill_item(key: &str, source_label: &str) -> SkillItem {
        SkillItem {
            key: key.to_string(),
            name: "pdf".to_string(),
            description: String::new(),
            level: "project".to_string(),
            source_slot: "bitfun".to_string(),
            source_label: source_label.to_string(),
            enabled: true,
            selected_for_runtime: true,
            default_enabled: true,
            is_shadowed: false,
            shadowed_by_key: None,
        }
    }

    #[test]
    fn skill_coverage_uses_winner_source_label() {
        let winner = skill_item("project::bitfun::pdf", "BitFun");
        let mut covered = skill_item("user::home.codex::pdf", "Codex");
        covered.is_shadowed = true;
        covered.shadowed_by_key = Some(winner.key.clone());

        let coverage = build_coverage_source_map(&[covered.clone(), winner]);
        assert_eq!(
            coverage.get(&covered.key).map(String::as_str),
            Some("BitFun")
        );
        assert!(!build_coverage_source_map(&[covered.clone()]).contains_key(&covered.key));
    }

    #[test]
    fn covered_enabled_skill_uses_an_indeterminate_checkbox_marker() {
        let selected = skill_item("project::bitfun::pdf", "BitFun");
        let mut covered = skill_item("user::home.codex::pdf", "Codex");
        covered.selected_for_runtime = false;
        covered.is_shadowed = true;
        covered.shadowed_by_key = Some(selected.key.clone());

        assert_eq!(skill_checkbox_marker(&selected), "[x] ");
        assert_eq!(skill_checkbox_marker(&covered), "[~] ");

        let mut disabled = covered;
        disabled.enabled = false;
        assert_eq!(skill_checkbox_marker(&disabled), "[ ] ");
    }

    #[test]
    fn narrow_configuration_popup_keeps_name_and_coverage_visible() {
        let selected = skill_item("project::bitfun::pdf", "BitFun");
        let mut covered = skill_item("user::home.claude::pdf", "Claude Code");
        covered.level = "user".to_string();
        covered.selected_for_runtime = false;
        covered.is_shadowed = true;
        covered.shadowed_by_key = Some(selected.key.clone());

        let mut state = SkillSelectorState::new();
        state.show_config(vec![covered, selected]);
        let mut terminal = Terminal::new(TestBackend::new(24, 8)).expect("test terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                state.render(frame, area, &Theme::dark_ansi16());
            })
            .expect("render narrow skill selector");

        let buffer = terminal.backend().buffer();
        let rendered = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            rendered.contains("[~] pdf < BitFun"),
            "narrow configuration should prioritize the skill name and coverage: {rendered:?}"
        );
        assert!(
            rendered.contains("user · Claude"),
            "source and scope missing: {rendered:?}"
        );
        assert!(
            rendered.contains("project · BitFun"),
            "source and scope missing: {rendered:?}"
        );
    }

    #[test]
    fn too_small_fallback_disables_hidden_selection() {
        let mut state = SkillSelectorState::new();
        state.show_list(vec![skill_item("project::bitfun::pdf", "BitFun")]);
        let mut terminal = Terminal::new(TestBackend::new(10, 3)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render tiny skill selector");

        assert!(state.confirm_selection().is_none());
        state.move_down();
        assert!(state.confirm_selection().is_none());
    }

    #[test]
    fn compact_boundary_uses_two_line_height_for_render_and_mouse_hit_testing() {
        let first = skill_item("project::bitfun::pdf", "BitFun");
        let second = skill_item("user::home.claude::pdf", "Claude Code");
        let mut state = SkillSelectorState::new();
        state.show_config(vec![first, second]);
        let mut terminal = Terminal::new(TestBackend::new(44, 8)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render boundary-width skill selector");

        let area = state.last_area.expect("popup area");
        assert_eq!(state.item_index_at(area.y + 2, area), Some(0));
        assert_eq!(state.item_index_at(area.y + 4, area), Some(1));
    }

    #[test]
    fn compact_menu_keeps_single_line_mouse_hit_testing() {
        let mut state = SkillSelectorState::new();
        state.show_menu();
        let mut terminal = Terminal::new(TestBackend::new(44, 8)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render compact skill menu");

        let area = state.last_area.expect("popup area");
        assert_eq!(state.item_index_at(area.y + 1, area), Some(0));
        assert_eq!(state.item_index_at(area.y + 2, area), Some(1));
    }
}
