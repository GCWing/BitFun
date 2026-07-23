/// Unified Agent popup for switching modes and opening delegated-agent management
///
/// Main Agent rows remain directly selectable. Management rows reuse the existing
/// Subagent and external-source flows instead of adding parallel slash commands.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::ui::{
    responsive_popup::{render_too_small, responsive_popup, ResponsivePopup},
    theme::{StyleKind, Theme},
};

/// An agent item for display in the selector
#[derive(Debug, Clone)]
pub(crate) struct AgentItem {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub(crate) enum AgentSelectorAction {
    SwitchMode(AgentItem),
    ManageSubagents,
    ReviewExternalSources,
}

/// Agent selector popup state
pub(super) struct AgentSelectorState {
    items: Vec<AgentItem>,
    list_state: ListState,
    visible: bool,
    /// Currently active agent ID (for highlighting)
    current_agent_id: Option<String>,
    include_external_sources: bool,
    allow_mode_switch: bool,
    last_area: Option<Rect>,
    interaction_enabled: bool,
}

impl AgentSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            current_agent_id: None,
            include_external_sources: false,
            allow_mode_switch: true,
            last_area: None,
            interaction_enabled: true,
        }
    }

    /// Show the agent selector with given agent list
    pub(super) fn show(
        &mut self,
        agents: Vec<AgentItem>,
        current_agent_id: Option<String>,
        include_external_sources: bool,
        allow_mode_switch: bool,
    ) {
        let initial_idx = current_agent_id
            .as_ref()
            .and_then(|id| agents.iter().position(|a| a.id == *id))
            .unwrap_or(0);

        self.items = agents;
        self.current_agent_id = current_agent_id;
        self.include_external_sources = include_external_sources;
        self.allow_mode_switch = allow_mode_switch;
        self.list_state.select(Some(initial_idx));
        self.visible = true;
        self.interaction_enabled = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the agent selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if self.len() > 0 {
            self.visible = true;
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn set_mode_switch_allowed(&mut self, allowed: bool) {
        self.allow_mode_switch = allowed;
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

    /// Get the selected action from the unified Agent entry.
    pub(super) fn confirm_selection(&self) -> Option<AgentSelectorAction> {
        if !self.visible || !self.interaction_enabled {
            return None;
        }
        let idx = self.list_state.selected()?;
        if let Some(agent) = self.items.get(idx) {
            return Some(AgentSelectorAction::SwitchMode(agent.clone()));
        }
        if idx == self.items.len() {
            return Some(AgentSelectorAction::ManageSubagents);
        }
        if self.include_external_sources && idx == self.items.len() + 1 {
            return Some(AgentSelectorAction::ReviewExternalSources);
        }
        None
    }

    /// Render the agent selector popup as an overlay
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        let ideal_height = self.len() as u16 + 4;
        let layout = responsive_popup(area, 60, ideal_height, 20, 5);
        let popup_area = match layout {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.last_area = None;
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Agents");
                return;
            }
        };
        self.interaction_enabled = true;
        self.last_area = Some(popup_area);
        let compact = popup_area.width < 40;

        let mut list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|agent| {
                let is_current = self
                    .current_agent_id
                    .as_ref()
                    .is_some_and(|id| id == &agent.id);

                let marker = if is_current { "● " } else { "  " };
                let marker_style = if is_current {
                    theme.style(StyleKind::Success)
                } else {
                    theme.style(StyleKind::Muted)
                };

                let name_style = if self.allow_mode_switch {
                    theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
                } else {
                    theme.style(StyleKind::Muted)
                };
                let desc_style = theme.style(StyleKind::Muted);
                let mut spans = vec![Span::styled(marker, marker_style)];
                if !self.allow_mode_switch {
                    spans.push(Span::styled(
                        if popup_area.width >= 28 {
                            "After current turn · "
                        } else {
                            "Wait · "
                        },
                        desc_style,
                    ));
                }
                spans.push(Span::styled(&agent.id, name_style));
                if !compact {
                    spans.extend([
                        Span::raw("  "),
                        Span::styled(format!("Main agent · {}", agent.description), desc_style),
                    ]);
                }
                let line = Line::from(spans);
                ListItem::new(line)
            })
            .collect();
        let mut subagent_spans = vec![
            Span::raw("  "),
            Span::styled(
                "Subagents",
                theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
            ),
        ];
        if !compact {
            subagent_spans.extend([
                Span::raw("  "),
                Span::styled(
                    "List, launch, or configure delegated agents",
                    theme.style(StyleKind::Muted),
                ),
            ]);
        }
        list_items.push(ListItem::new(Line::from(subagent_spans)));
        if self.include_external_sources {
            let mut external_spans = vec![
                Span::raw("  "),
                Span::styled(
                    "External AI applications",
                    theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
                ),
            ];
            if !compact {
                external_spans.extend([
                    Span::raw("  "),
                    Span::styled(
                        "Review imported agents and choices",
                        theme.style(StyleKind::Muted),
                    ),
                ]);
            }
            list_items.push(ListItem::new(Line::from(external_spans)));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(if compact {
                " Agents · Enter Select · Esc "
            } else {
                " Agents (↑↓ Navigate, Enter Select, Esc Cancel) "
            });

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
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<AgentSelectorAction> {
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

    fn len(&self) -> usize {
        self.items.len() + 1 + usize::from(self.include_external_sources)
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentItem, AgentSelectorAction, AgentSelectorState};
    use crate::ui::theme::Theme;
    use ratatui::{backend::TestBackend, Terminal};

    fn modes() -> Vec<AgentItem> {
        vec![
            AgentItem {
                id: "agentic".to_string(),
                description: "General purpose".to_string(),
            },
            AgentItem {
                id: "ask".to_string(),
                description: "Read only".to_string(),
            },
        ]
    }

    #[test]
    fn chat_agent_entry_keeps_modes_direct_and_adds_management_rows() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), true, true);

        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::SwitchMode(AgentItem { id, .. })) if id == "agentic"
        ));
        state.move_down();
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ManageSubagents)
        ));
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ReviewExternalSources)
        ));
    }

    #[test]
    fn startup_agent_entry_omits_session_scoped_external_sources() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), false, true);
        state.move_down();
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ManageSubagents)
        ));
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::SwitchMode(AgentItem { id, .. })) if id == "agentic"
        ));
    }

    #[test]
    fn processing_turn_keeps_management_available_and_defers_mode_guard_to_dispatch() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), true, false);

        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::SwitchMode(AgentItem { id, .. })) if id == "agentic"
        ));
        state.move_down();
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ManageSubagents)
        ));
    }

    #[test]
    fn narrow_processing_popup_keeps_mode_unavailability_reason_visible() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), true, false);
        let mut terminal = Terminal::new(TestBackend::new(32, 9)).expect("test terminal");

        terminal
            .draw(|frame| {
                let area = frame.area();
                state.render(frame, area, &Theme::dark_ansi16());
            })
            .expect("render narrow agent selector");

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
            rendered.contains("After current turn"),
            "narrow popup should explain why mode rows are disabled: {rendered:?}"
        );
    }

    #[test]
    fn open_popup_refreshes_mode_availability_when_the_turn_ends() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), true, false);

        state.set_mode_switch_allowed(true);

        assert!(state.allow_mode_switch);
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::SwitchMode(AgentItem { id, .. })) if id == "agentic"
        ));
    }

    #[test]
    fn management_rows_remain_available_when_mode_discovery_is_empty() {
        let mut state = AgentSelectorState::new();
        state.show(Vec::new(), None, true, true);

        assert!(state.is_visible());
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ManageSubagents)
        ));
        state.move_down();
        assert!(matches!(
            state.confirm_selection(),
            Some(AgentSelectorAction::ReviewExternalSources)
        ));
    }

    #[test]
    fn too_small_fallback_disables_hidden_selection() {
        let mut state = AgentSelectorState::new();
        state.show(modes(), Some("agentic".to_string()), true, true);
        let mut terminal = Terminal::new(TestBackend::new(10, 3)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render tiny agent selector");

        assert!(state.confirm_selection().is_none());
        state.move_down();
        assert!(state.confirm_selection().is_none());
    }
}
