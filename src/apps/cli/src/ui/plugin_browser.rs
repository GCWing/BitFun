/// Plugin browser popup
///
/// Overlay popup that lists managed plugin packages with their activation
/// state. Lets the user search by `package_id`, scroll the list, and toggle
/// activation with Space. Activation/deactivation is dispatched back to the
/// ChatMode owner, which runs the async toggle and refreshes the snapshot.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::plugin_ops::{PluginDisplayStatus, PluginInstallScope, PluginItem};
use crate::ui::{
    responsive_popup::{render_too_small, responsive_popup, ResponsivePopup},
    theme::{StyleKind, Theme},
};

/// Action returned from the plugin browser.
#[derive(Debug, Clone)]
pub(crate) enum PluginBrowserAction {
    /// User pressed Space/Enter on the selected plugin; ChatMode should toggle it.
    Toggle(PluginItem),
    /// User submitted the install dialog (Shift+I) with a package spec and
    /// scope. ChatMode/startup should run the install flow.
    Install {
        spec: String,
        scope: PluginInstallScope,
    },
    /// No action (key consumed or no-op).
    None,
    /// User dismissed the browser (Esc / Ctrl+C).
    Dismiss,
}

/// Plugin browser popup state.
pub(super) struct PluginBrowserState {
    items: Vec<PluginItem>,
    /// Indices into `items` that match the current search query.
    filtered_indices: Vec<usize>,
    list_state: ListState,
    visible: bool,
    search_query: String,
    /// Which plugin is currently being toggled (loading indicator).
    loading_id: Option<String>,
    last_area: Option<Rect>,
    interaction_enabled: bool,
    /// Install sub-dialog state (Shift+I). When active, the browser renders an
    /// input form instead of the list and routes keys to the install handler.
    install_active: bool,
    install_input: String,
    install_scope: PluginInstallScope,
    /// True while an install task is in flight (input suspended, "Installing…").
    install_busy: bool,
    /// Last install outcome message; cleared on the next keystroke.
    install_message: Option<String>,
}

impl PluginBrowserState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            search_query: String::new(),
            loading_id: None,
            last_area: None,
            interaction_enabled: true,
            install_active: false,
            install_input: String::new(),
            install_scope: PluginInstallScope::User,
            install_busy: false,
            install_message: None,
        }
    }

    /// Show the plugin browser with the given item list.
    pub(super) fn show(&mut self, mut items: Vec<PluginItem>) {
        // Sort: builtin source_scope first (like deveco-code's internal-first),
        // then alphabetical by id within each group.
        items.sort_by(|a, b| {
            let a_builtin = a.source_scope == "builtin";
            let b_builtin = b.source_scope == "builtin";
            match (a_builtin, b_builtin) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.id.cmp(&b.id),
            }
        });
        self.items = items;
        self.search_query.clear();
        self.rebuild_filtered();
        if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
        self.loading_id = None;
        self.visible = true;
        self.interaction_enabled = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.loading_id = None;
        self.search_query.clear();
        self.last_area = None;
        self.install_active = false;
        self.install_input.clear();
        self.install_busy = false;
        self.install_message = None;
    }

    /// Reshow the plugin browser (for back navigation).
    pub(super) fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn set_loading(&mut self, id: Option<String>) {
        self.loading_id = id;
    }

    // ============ Install sub-dialog ============

    pub(super) fn set_install_busy(&mut self, busy: bool) {
        self.install_busy = busy;
    }

    pub(super) fn set_install_message(&mut self, msg: Option<String>) {
        self.install_message = msg;
    }

    fn open_install(&mut self) {
        self.install_active = true;
        self.install_input.clear();
        self.install_scope = PluginInstallScope::User;
        self.install_busy = false;
        self.install_message = None;
    }

    fn close_install(&mut self) {
        self.install_active = false;
        self.install_input.clear();
        self.install_message = None;
    }

    /// Key handler for the install sub-dialog. The dialog owns input, scope
    /// toggling, and busy/message display; on Enter it returns
    /// `Install { spec, scope }` for the caller to dispatch.
    fn handle_install_key(&mut self, key: KeyEvent) -> PluginBrowserAction {
        if self.install_busy {
            // Only Esc is honored while installing; the in-flight task reports
            // completion through the poll loop, which clears busy/message.
            if key.code == KeyCode::Esc {
                self.close_install();
            }
            return PluginBrowserAction::None;
        }
        match key.code {
            KeyCode::Esc => {
                self.close_install();
                PluginBrowserAction::None
            }
            KeyCode::Tab => {
                self.install_scope = self.install_scope.toggle();
                self.install_message = None;
                PluginBrowserAction::None
            }
            KeyCode::Backspace => {
                self.install_input.pop();
                self.install_message = None;
                PluginBrowserAction::None
            }
            KeyCode::Enter => {
                let spec = self.install_input.trim().to_string();
                if spec.is_empty() {
                    self.install_message = Some("Package name or path is required".to_string());
                    return PluginBrowserAction::None;
                }
                let scope = self.install_scope;
                self.install_busy = true;
                self.install_message = None;
                PluginBrowserAction::Install { spec, scope }
            }
            KeyCode::Char(ch) if ch.is_ascii_graphic() => {
                self.install_input.push(ch);
                self.install_message = None;
                PluginBrowserAction::None
            }
            _ => PluginBrowserAction::None,
        }
    }

    /// Replace items in-place (after a toggle completes), preserving selection by id.
    pub(super) fn update_items(&mut self, items: Vec<PluginItem>) {
        let selected_id = self
            .list_state
            .selected()
            .and_then(|idx| self.filtered_indices.get(idx).copied())
            .and_then(|idx| self.items.get(idx))
            .map(|item| item.id.clone());
        self.items = items;
        self.rebuild_filtered();
        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else if let Some(id) = selected_id.as_ref() {
            let pos = self
                .filtered_indices
                .iter()
                .position(|&idx| &self.items[idx].id == id);
            match pos {
                Some(p) => self.list_state.select(Some(p)),
                None => self.list_state.select(Some(0)),
            }
        } else {
            self.list_state.select(Some(0));
        }
        let loading_removed = self
            .loading_id
            .as_deref()
            .is_some_and(|id| !self.items.iter().any(|item| item.id == id));
        if loading_removed {
            self.loading_id = None;
        }
    }

    fn rebuild_filtered(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.is_empty() || item.id.to_lowercase().contains(&query))
            .map(|(idx, _)| idx)
            .collect();
    }

    fn move_up(&mut self) {
        if !self.visible || !self.interaction_enabled || self.filtered_indices.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let len = self.filtered_indices.len();
        let next = (selected + len - 1) % len;
        self.list_state.select(Some(next));
    }

    fn move_down(&mut self) {
        if !self.visible || !self.interaction_enabled || self.filtered_indices.is_empty() {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let next = (selected + 1) % self.filtered_indices.len();
        self.list_state.select(Some(next));
    }

    fn confirm_selection(&self) -> Option<PluginItem> {
        if !self.visible || !self.interaction_enabled {
            return None;
        }
        let idx = self.list_state.selected()?;
        let &item_idx = self.filtered_indices.get(idx)?;
        self.items.get(item_idx).cloned()
    }

    /// Handle a key event. Returns an action the caller is responsible for
    /// dispatching (e.g. toggling the plugin). Search input is owned by the
    /// popup itself: typing any printable ASCII graphic character filters the
    /// list by `package_id` (case-insensitive substring); `Backspace` removes
    /// the last search character.
    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> PluginBrowserAction {
        if !self.visible {
            return PluginBrowserAction::None;
        }
        if !self.interaction_enabled {
            if key.code == KeyCode::Esc {
                self.hide();
                return PluginBrowserAction::Dismiss;
            }
            return PluginBrowserAction::None;
        }
        if self.install_active {
            return self.handle_install_key(key);
        }
        match key.code {
            KeyCode::Esc => {
                self.hide();
                PluginBrowserAction::Dismiss
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.hide();
                PluginBrowserAction::Dismiss
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                PluginBrowserAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                PluginBrowserAction::None
            }
            KeyCode::PageUp => {
                if !self.filtered_indices.is_empty() {
                    let selected = self.list_state.selected().unwrap_or(0);
                    let next = selected.saturating_sub(10);
                    self.list_state.select(Some(next));
                }
                PluginBrowserAction::None
            }
            KeyCode::PageDown => {
                if !self.filtered_indices.is_empty() {
                    let selected = self.list_state.selected().unwrap_or(0);
                    let len = self.filtered_indices.len();
                    let next = (selected + 10).min(len - 1);
                    self.list_state.select(Some(next));
                }
                PluginBrowserAction::None
            }
            KeyCode::Home => {
                if !self.filtered_indices.is_empty() {
                    self.list_state.select(Some(0));
                }
                PluginBrowserAction::None
            }
            KeyCode::End => {
                if !self.filtered_indices.is_empty() {
                    let last = self.filtered_indices.len() - 1;
                    self.list_state.select(Some(last));
                }
                PluginBrowserAction::None
            }
            KeyCode::Char(' ') | KeyCode::Enter => match self.confirm_selection() {
                Some(item) => PluginBrowserAction::Toggle(item),
                None => PluginBrowserAction::None,
            },
            KeyCode::Char('I') if key.modifiers == KeyModifiers::SHIFT => {
                self.open_install();
                PluginBrowserAction::None
            }
            KeyCode::Backspace => {
                if self.search_query.pop().is_some() {
                    self.rebuild_filtered();
                    if !self.filtered_indices.is_empty() {
                        self.list_state.select(Some(0));
                    } else {
                        self.list_state.select(None);
                    }
                }
                PluginBrowserAction::None
            }
            KeyCode::Char(ch) if ch.is_ascii_graphic() => {
                self.search_query.push(ch);
                self.rebuild_filtered();
                if !self.filtered_indices.is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
                PluginBrowserAction::None
            }
            _ => PluginBrowserAction::None,
        }
    }

    /// Render the plugin browser popup as an overlay.
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        let ideal_height = (self.filtered_indices.len() as u16 + 6).max(8);
        let layout = responsive_popup(area, 72, ideal_height, 18, 6);
        let popup_area = match layout {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.last_area = None;
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Plugins");
                return;
            }
        };
        self.interaction_enabled = true;
        self.last_area = Some(popup_area);
        let popup_width = popup_area.width;

        if self.install_active {
            self.render_install(frame, popup_area, theme);
            return;
        }

        let loading_id = self.loading_id.clone();

        let mut list_items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&idx| {
                let item = &self.items[idx];
                let is_loading = loading_id.as_ref().is_some_and(|id| id == &item.id);
                let (marker, marker_style, status_style) = if is_loading {
                    (
                        "\u{22ef} ",
                        theme.style(StyleKind::Warning),
                        theme.style(StyleKind::Warning),
                    )
                } else {
                    let style = match item.status {
                        PluginDisplayStatus::Active => theme.style(StyleKind::Success),
                        PluginDisplayStatus::Inactive => theme.style(StyleKind::Error),
                        PluginDisplayStatus::Disabled => theme.style(StyleKind::Muted),
                        PluginDisplayStatus::Unreviewed => theme.style(StyleKind::Warning),
                    };
                    let marker = match item.status {
                        PluginDisplayStatus::Active => "\u{2713} ",
                        PluginDisplayStatus::Inactive => "\u{25cb} ",
                        PluginDisplayStatus::Disabled => "\u{2717} ",
                        PluginDisplayStatus::Unreviewed => "? ",
                    };
                    (marker, style, style)
                };
                let status_label = if is_loading {
                    "Loading...".to_string()
                } else {
                    item.status.label().to_string()
                };
                let name_style = theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                if popup_width < 50 {
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(marker, marker_style),
                            Span::styled(&item.id, name_style),
                        ]),
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(status_label, status_style),
                            Span::raw("  "),
                            Span::styled(&item.trust_label, theme.style(StyleKind::Muted)),
                        ]),
                    ])
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled(marker, marker_style),
                        Span::styled(&item.id, name_style),
                        Span::raw("  "),
                        Span::styled(status_label, status_style),
                        Span::raw("  "),
                        Span::styled(
                            format!("({})", item.source_scope),
                            theme.style(StyleKind::Muted),
                        ),
                        Span::raw("  "),
                        Span::styled(&item.trust_label, theme.style(StyleKind::Muted)),
                        Span::raw("  "),
                        Span::styled(format!("v{}", item.version), theme.style(StyleKind::Muted)),
                    ]))
                }
            })
            .collect();

        if list_items.is_empty() {
            let empty_msg = if self.search_query.is_empty() {
                "  No plugins found".to_string()
            } else {
                format!("  No plugins match '{}'", self.search_query)
            };
            list_items.push(ListItem::new(Line::from(Span::styled(
                empty_msg,
                theme.style(StyleKind::Muted),
            ))));
        }

        if popup_width >= 50 {
            list_items.push(ListItem::new(Line::from(Span::styled(
                " Space:Toggle  Up/Down:Nav  Type:Search  Shift+I:Install  Esc:Close",
                theme.style(StyleKind::Muted),
            ))));
        }

        let search_display = if self.search_query.is_empty() {
            String::from("")
        } else {
            self.search_query.clone()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(format!(" Plugins  search: {} ", search_display));

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

    /// Render the install sub-dialog form over the popup area.
    fn render_install(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let input_display = if self.install_input.is_empty() {
            String::from("npm name, @scope/pkg, pkg@version, or file://path")
        } else {
            self.install_input.clone()
        };
        let scope_hint = match self.install_scope {
            PluginInstallScope::User => "user plugins dir",
            PluginInstallScope::Project => "project plugins dir",
        };

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("> ", theme.style(StyleKind::Primary)),
            Span::styled(input_display, theme.style(StyleKind::Primary)),
            Span::styled("_", theme.style(StyleKind::Muted)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("Scope: {} ({})", self.install_scope.label(), scope_hint),
                theme.style(StyleKind::Muted),
            ),
            Span::raw("  "),
            Span::styled("(Tab: toggle)", theme.style(StyleKind::Muted)),
        ]));

        if self.install_busy {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Installing…",
                theme.style(StyleKind::Warning),
            )));
        } else if let Some(msg) = &self.install_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(msg, theme.style(StyleKind::Error))));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter:Install  Tab:Scope  Esc:Back",
            theme.style(StyleKind::Muted),
        )));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Install plugin ");

        let content = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.background));

        frame.render_widget(Clear, area);
        frame.render_widget(content, area);
    }
}
