/// Slash command menu rendering and state
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::actions::{slash_actions, ActionState};
use crate::ui::theme::{StyleKind, Theme};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalCommandProjection {
    pub action_id: String,
    pub command_name: String,
    pub invocation_alias: String,
    pub candidate_id: String,
    pub content_version: String,
    pub description: String,
    pub restricted: bool,
    pub provider_conflict_key: Option<String>,
    pub native_collision: Option<NativeCommandCollisionProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCommandCollisionProjection {
    pub native_action_id: String,
    pub native_candidate_id: String,
    pub external_candidate_id: String,
    pub conflict_key: String,
    pub selected_candidate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandMenuItem {
    id: String,
    name: String,
    description: String,
}

pub(super) struct CommandMenuState {
    action_state: ActionState,
    items: Vec<CommandMenuItem>,
    external_commands: Vec<ExternalCommandProjection>,
    external_discovery_pending: bool,
    builtin_reconfirmations: BTreeSet<String>,
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
            external_commands: Vec::new(),
            external_discovery_pending: false,
            builtin_reconfirmations: BTreeSet::new(),
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
        let built_in = slash_actions(self.action_state);
        let built_in_names = built_in
            .iter()
            .map(|action| action.name.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>();
        let mut commands = built_in
            .into_iter()
            .map(|action| {
                let collision = self.external_commands.iter().find_map(|command| {
                    let collision = command.native_collision.as_ref()?;
                    (collision.native_action_id == action.id).then_some(collision)
                });
                let selected_external = collision.is_some_and(|collision| {
                    collision.selected_candidate_id.as_deref()
                        == Some(collision.external_candidate_id.as_str())
                });
                let unresolved =
                    collision.is_some_and(|collision| collision.selected_candidate_id.is_none());
                let command_name = action.name.trim_start_matches('/').to_ascii_lowercase();
                let reconfirmation_required = self.builtin_reconfirmations.contains(&command_name);
                let discovery_pending = self.external_discovery_pending
                    && self.action_state.context == crate::actions::ActionContext::Chat;
                let name = if selected_external
                    || unresolved
                    || reconfirmation_required
                    || discovery_pending
                {
                    format!("/builtin:{}", action.name.trim_start_matches('/'))
                } else {
                    action.name.to_string()
                };
                CommandMenuItem {
                    id: action.id.to_string(),
                    name,
                    description: if unresolved || reconfirmation_required {
                        format!("{} (choose once)", action.description)
                    } else if discovery_pending {
                        format!("{} (checking external sources)", action.description)
                    } else {
                        action.description.to_string()
                    },
                }
            })
            .collect::<Vec<_>>();
        if self.action_state.context == crate::actions::ActionContext::Chat
            && !self.action_state.is_processing
        {
            commands.extend(self.external_commands.iter().map(|command| {
                let requested_name = format!("/{}", command.command_name);
                let selected_external =
                    command.native_collision.as_ref().is_some_and(|collision| {
                        collision.selected_candidate_id.as_deref()
                            == Some(collision.external_candidate_id.as_str())
                    });
                let collides = built_in_names.contains(&requested_name.to_ascii_lowercase());
                let name = if command.provider_conflict_key.is_some() {
                    command.invocation_alias.clone()
                } else if collides && !selected_external {
                    format!("/external:{}", command.command_name)
                } else {
                    requested_name
                };
                let unresolved = command
                    .native_collision
                    .as_ref()
                    .is_some_and(|collision| collision.selected_candidate_id.is_none());
                let description = if command.restricted {
                    format!("{} (currently restricted)", command.description)
                } else if unresolved {
                    format!("{} (choose once)", command.description)
                } else if command.provider_conflict_key.is_some() {
                    format!("{} (choose this source)", command.description)
                } else {
                    command.description.clone()
                };
                CommandMenuItem {
                    id: command.action_id.clone(),
                    name,
                    description,
                }
            }));
        }
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
                    .unwrap_or(&spec.name)
                    .to_ascii_lowercase()
                    .contains(&normalized)
            });
            self.items = commands;
        }
        self.items.sort_by(|left, right| left.name.cmp(&right.name));

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
        let command = selected.id.clone();
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
                    Span::styled(spec.name.clone(), name_style),
                    Span::raw(" - "),
                    Span::styled(spec.description.clone(), desc_style),
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

    fn selected_item(&self) -> Option<&CommandMenuItem> {
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

    #[cfg(test)]
    pub(super) fn set_external_commands(&mut self, commands: Vec<ExternalCommandProjection>) {
        self.external_commands = commands;
        self.update(&self.last_input.clone(), self.last_input.chars().count());
    }

    pub(super) fn set_external_source_state(
        &mut self,
        commands: Vec<ExternalCommandProjection>,
        discovery_pending: bool,
        builtin_reconfirmations: BTreeSet<String>,
    ) {
        self.external_commands = commands;
        self.external_discovery_pending = discovery_pending;
        self.builtin_reconfirmations = builtin_reconfirmations;
        self.update(&self.last_input.clone(), self.last_input.chars().count());
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    fn names(menu: &CommandMenuState) -> Vec<&str> {
        menu.items.iter().map(|item| item.name.as_str()).collect()
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
    fn processing_chat_keeps_agent_management_and_hides_idle_only_actions() {
        let mut menu = CommandMenuState::new(ActionState::chat(true, false));
        menu.update("/", 1);

        assert!(names(&menu).contains(&"/agents"));
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

        assert_eq!(
            menu.selected_item().map(|item| item.id.as_str()),
            Some("logout")
        );
    }

    #[test]
    fn external_commands_join_chat_menu_without_entering_the_host_action_registry() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_commands(vec![ExternalCommandProjection {
            action_id: "external-command:review".to_string(),
            command_name: "review".to_string(),
            invocation_alias: "/review".to_string(),
            candidate_id: "external:review".to_string(),
            content_version: "v1".to_string(),
            description: "Review from OpenCode".to_string(),
            restricted: false,
            provider_conflict_key: None,
            native_collision: None,
        }]);
        menu.update("/rev", 4);

        assert_eq!(names(&menu), ["/review"]);
        assert_eq!(
            menu.apply_selection().as_deref(),
            Some("external-command:review")
        );
    }

    #[test]
    fn built_in_aliases_are_protected_with_an_explicit_external_fallback_alias() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_commands(vec![ExternalCommandProjection {
            action_id: "external-command:help".to_string(),
            command_name: "help".to_string(),
            invocation_alias: "/help".to_string(),
            candidate_id: "external:help".to_string(),
            content_version: "v1".to_string(),
            description: "External help".to_string(),
            restricted: false,
            provider_conflict_key: None,
            native_collision: Some(NativeCommandCollisionProjection {
                native_action_id: "help".to_string(),
                native_candidate_id: "bitfun.cli:help".to_string(),
                external_candidate_id: "external:help".to_string(),
                conflict_key: "conflict-v1".to_string(),
                selected_candidate_id: None,
            }),
        }]);
        menu.update("/", 1);

        assert!(names(&menu).contains(&"/builtin:help"));
        assert!(names(&menu).contains(&"/external:help"));
    }

    #[test]
    fn remembered_external_choice_routes_the_plain_alias_until_content_changes() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_commands(vec![ExternalCommandProjection {
            action_id: "external-command:help".to_string(),
            command_name: "help".to_string(),
            invocation_alias: "/help".to_string(),
            candidate_id: "external:help".to_string(),
            content_version: "v1".to_string(),
            description: "External help".to_string(),
            restricted: false,
            provider_conflict_key: None,
            native_collision: Some(NativeCommandCollisionProjection {
                native_action_id: "help".to_string(),
                native_candidate_id: "bitfun.cli:help".to_string(),
                external_candidate_id: "external:help".to_string(),
                conflict_key: "conflict-v1".to_string(),
                selected_candidate_id: Some("external:help".to_string()),
            }),
        }]);
        menu.update("/", 1);

        assert!(names(&menu).contains(&"/builtin:help"));
        assert!(names(&menu).contains(&"/help"));
        assert!(!names(&menu).contains(&"/external:help"));
    }

    #[test]
    fn discovery_pending_keeps_builtin_commands_available_but_explicit() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_source_state(Vec::new(), true, BTreeSet::new());
        menu.update("/help", 5);

        assert!(names(&menu).contains(&"/builtin:help"));
        assert!(!names(&menu).contains(&"/help"));
        assert!(menu.items.iter().any(|item| {
            item.name == "/builtin:help" && item.description.contains("checking external sources")
        }));
    }

    #[test]
    fn removed_external_collision_keeps_builtin_alias_explicit_until_reconfirmed() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_source_state(Vec::new(), false, BTreeSet::from(["help".to_string()]));
        menu.update("/help", 5);

        assert!(names(&menu).contains(&"/builtin:help"));
        assert!(!names(&menu).contains(&"/help"));
        assert_eq!(menu.apply_selection().as_deref(), Some("help"));
    }

    #[test]
    fn removed_external_collision_mouse_selection_returns_builtin_action_for_confirmation() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_source_state(Vec::new(), false, BTreeSet::from(["help".to_string()]));
        menu.update("/help", 5);
        menu.last_area = Some(Rect::new(5, 5, 40, 3));

        let selected = menu.handle_mouse_event(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 6,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(selected.as_deref(), Some("help"));
    }

    #[test]
    fn unresolved_provider_candidates_have_explicit_selectable_aliases() {
        let mut menu = CommandMenuState::new(ActionState::chat(false, false));
        menu.set_external_commands(vec![ExternalCommandProjection {
            action_id: "external-command-candidate:opencode-review".to_string(),
            command_name: "review".to_string(),
            invocation_alias: "/external:opencode.commands:review".to_string(),
            candidate_id: "opencode-review".to_string(),
            content_version: "v1".to_string(),
            description: "OpenCode project · opencode".to_string(),
            restricted: false,
            provider_conflict_key: Some("provider-conflict-v1".to_string()),
            native_collision: None,
        }]);
        menu.update("/external:opencode", 18);

        assert_eq!(names(&menu), ["/external:opencode.commands:review"]);
        assert_eq!(
            menu.apply_selection().as_deref(),
            Some("external-command-candidate:opencode-review")
        );
    }
}
