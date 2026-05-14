/// Startup page module
///
/// Full-featured startup page with:
/// - Centered logo and input box
/// - Slash command menu with real execution
/// - Model/Agent/Session/Skill/Subagent selector popups
/// - Random tips

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use std::sync::Arc;
use std::time::Duration;
use super::text_input::{TextInput, TextInputStyle};
use super::command_menu::CommandMenuState;
use super::command_palette::{CommandPaletteState, PaletteAction};
use super::model_config_form::{ModelConfigFormState, ModelFormAction, ModelFormResult};
use super::model_selector::{ModelItem, ModelSelectorState};
use super::provider_selector::{ProviderSelection, ProviderSelectorState};
use super::agent_selector::{AgentItem, AgentSelectorState};
use super::session_selector::{SessionAction, SessionItem, SessionSelectorState};
use super::skill_selector::{SkillItem, SkillSelectorState};
use super::subagent_selector::{SubagentItem, SubagentSelectorState};
use super::theme::{
    builtin_theme_json, resolve_appearance, resolve_effective_color_scheme, EffectiveColorScheme,
    Theme,
};
use crate::commands::STARTUP_COMMAND_SPECS;
use crate::config::CliConfig;

use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::agentic::agents::{get_agent_registry, AgentInfo};
use bitfun_core::agentic::tools::implementations::skills::registry::SkillRegistry;
use bitfun_core::service::config::GlobalConfigManager;

/// Types of popups that can be shown on the startup page
#[derive(Debug, Clone, PartialEq)]
pub enum PopupType {
    CommandPalette,
    ModelSelector,
    AgentSelector,
    SessionSelector,
    SkillSelector,
    SubagentSelector,
    ProviderSelector,
    ModelConfigForm,
}

/// Navigation stack for managing popup hierarchy
#[derive(Debug, Default)]
pub struct PopupStack {
    stack: Vec<PopupType>,
}

impl PopupStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a popup onto the stack
    pub fn push(&mut self, popup: PopupType) {
        // Avoid duplicates at the top
        if self.stack.last() != Some(&popup) {
            self.stack.push(popup);
        }
    }

    /// Pop the top popup from the stack
    pub fn pop(&mut self) -> Option<PopupType> {
        self.stack.pop()
    }

    /// Peek at the top popup without removing it
    #[allow(dead_code)]
    pub fn peek(&self) -> Option<&PopupType> {
        self.stack.last()
    }

    /// Check if the stack is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Clear all popups from the stack
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

/// Startup menu result
#[derive(Debug, Clone)]
pub enum StartupResult {
    /// Start a new session with an optional initial prompt
    NewSession { prompt: Option<String> },
    /// Continue last session (session ID)
    ContinueSession(String),
    /// User cancelled exit
    Exit,
}

/// Keyboard shortcuts help text for startup page
const KEYBOARD_SHORTCUTS_HELP: &str = "\
Keyboard Shortcuts\n\
─────────────────────────────────\n\
Tab / Shift+Tab   Switch Agent\n\
Ctrl+P            Command Palette\n\
Esc               Back / Interrupt\n\
Ctrl+W            Close All Windows\n\
Ctrl+C            Exit";

/// Random tips shown on the startup page
const TIPS: &[&str] = &[
    "Type / for slash commands (e.g. /help, /models, /agents)",
    "Press Tab to cycle between agents",
    "Use /init to explore your repo and generate AGENTS.md",
    "Press Ctrl+E to toggle browse mode for scrolling history",
    "Use /sessions to list and continue previous conversations",
    "Press Ctrl+O to expand/collapse tool output",
    "Use /skills to browse and execute available skills",
    "Press Up/Down to cycle through input history",
    "Use /new to start a fresh conversation session",
];

/// Startup page
pub struct StartupPage {
    /// Multiline text input component
    text_input: TextInput,
    /// Theme
    theme: Theme,
    /// Current tip text
    tip: &'static str,

    // ── Command menu ──
    command_menu: CommandMenuState,

    // ── Command palette (Ctrl+P) ──
    command_palette: CommandPaletteState,

    // ── Selector popups ──
    model_selector: ModelSelectorState,
    agent_selector: AgentSelectorState,
    session_selector: SessionSelectorState,
    skill_selector: SkillSelectorState,
    subagent_selector: SubagentSelectorState,
    provider_selector: ProviderSelectorState,
    model_config_form: ModelConfigFormState,

    // ── System context ──
    coordinator: Arc<ConversationCoordinator>,

    // ── State ──
    /// Selected agent type (can be changed via /agents or Tab)
    agent_type: String,
    /// Display name of selected model
    model_display_name: String,
    /// Workspace path for display in bottom bar
    workspace_display: String,
    /// Status message (temporarily shown instead of tip)
    status: Option<String>,
    /// Info popup message (rendered as overlay, dismissed by any key)
    info_popup: Option<String>,

    /// Popup navigation stack for back navigation
    popup_stack: PopupStack,
}

impl StartupPage {
    pub fn new(
        coordinator: Arc<ConversationCoordinator>,
        default_agent: String,
        workspace: Option<String>,
    ) -> Self {
        let config = CliConfig::load().unwrap_or_default();
        let appearance = resolve_appearance(&config.ui.theme);
        let scheme = resolve_effective_color_scheme(&config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };
        let theme = if scheme == EffectiveColorScheme::Monochrome {
            Theme::monochrome()
        } else {
            let id = config.ui.theme_id.trim();
            if id.is_empty() {
                base
            } else if let Some(json) = builtin_theme_json(id) {
                base.apply_opencode_theme_json(json, appearance)
                    .unwrap_or(base)
                    .with_effective_scheme(scheme)
            } else {
                base
            }
        };

        let tip_index = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize
            % TIPS.len();

        let mut page = Self {
            text_input: TextInput::new(),
            theme,
            tip: TIPS[tip_index],
            command_menu: CommandMenuState::new(),
            command_palette: CommandPaletteState::new(),
            model_selector: ModelSelectorState::new(),
            agent_selector: AgentSelectorState::new(),
            session_selector: SessionSelectorState::new(),
            skill_selector: SkillSelectorState::new(),
            subagent_selector: SubagentSelectorState::new(),
            provider_selector: ProviderSelectorState::new(),
            model_config_form: ModelConfigFormState::new(),
            coordinator,
            agent_type: default_agent,
            model_display_name: String::new(),
            workspace_display: workspace.unwrap_or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|p| dunce::canonicalize(&p).ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
            }),
            status: None,
            info_popup: None,
            popup_stack: PopupStack::new(),
        };

        // Load current model name
        page.load_current_model_name();
        page
    }

    /// Get the currently selected agent type
    pub fn agent_type(&self) -> &str {
        &self.agent_type
    }

    /// Get the current workspace path for this CLI process.
    pub fn workspace(&self) -> Option<String> {
        if self.workspace_display.is_empty() {
            None
        } else {
            Some(self.workspace_display.clone())
        }
    }

    fn workspace_path_buf(&self) -> std::path::PathBuf {
        self.workspace()
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Check if any popup is currently visible
    fn any_popup_visible(&self) -> bool {
        self.command_palette.is_visible()
            || self.model_selector.is_visible()
            || self.agent_selector.is_visible()
            || self.session_selector.is_visible()
            || self.skill_selector.is_visible()
            || self.subagent_selector.is_visible()
            || self.provider_selector.is_visible()
            || self.model_config_form.is_visible()
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<StartupResult> {
        terminal.clear()?;

        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(Duration::from_millis(50))? {
                if let Ok(first_event) = event::read() {
                    let mut events = vec![first_event];
                    // Short wait to let rapid paste events arrive in the same batch.
                    // Duration::ZERO would split pastes across loop iterations.
                    while event::poll(Duration::from_millis(5))? {
                        if let Ok(ev) = event::read() {
                            events.push(ev);
                        } else {
                            break;
                        }
                    }

                    // Paste detection: multiple key events with Enter + printable chars
                    let key_count = events
                        .iter()
                        .filter(|e| matches!(e, Event::Key(k) if k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat))
                        .count();
                    let has_enter = events.iter().any(|e| {
                        matches!(e, Event::Key(k) if (k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat) && k.code == KeyCode::Enter)
                    });
                    let has_printable = events.iter().any(|e| {
                        matches!(e, Event::Key(k) if (k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat) && matches!(k.code, KeyCode::Char(_)))
                    });
                    let is_paste_batch = key_count > 1 && has_enter && has_printable;

                    if is_paste_batch {
                        let mut paste_buf = String::new();
                        let mut non_key_events = Vec::new();
                        for ev in events {
                            match ev {
                                Event::Key(k)
                                    if k.kind == KeyEventKind::Press
                                        || k.kind == KeyEventKind::Repeat =>
                                {
                                    match k.code {
                                        KeyCode::Char(c) => paste_buf.push(c),
                                        KeyCode::Enter => paste_buf.push('\n'),
                                        _ => {}
                                    }
                                }
                                other => non_key_events.push(other),
                            }
                        }
                        if !paste_buf.is_empty() {
                            self.text_input.insert_paste(&paste_buf);
                            self.refresh_command_menu();
                        }
                        for ev in non_key_events {
                            self.handle_non_key_event(ev, terminal)?;
                        }
                    } else {
                        for ev in events {
                            match ev {
                                Event::Key(key) if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat => {
                                    if let Some(result) = self.handle_key(key) {
                                        return Ok(result);
                                    }
                                }
                                other => {
                                    self.handle_non_key_event(other, terminal)?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_non_key_event<B: Backend>(
        &mut self,
        ev: Event,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        match ev {
            Event::Mouse(mouse) => {
                if self.command_palette.captures_mouse(&mouse) {
                    let action = self.command_palette.handle_mouse_event(&mouse);
                    if let PaletteAction::Execute(id) = action {
                        let _ = self.handle_palette_action(&id);
                    }
                } else if self.provider_selector.captures_mouse(&mouse) {
                    if let Some(selection) = self.provider_selector.handle_mouse_event(&mouse) {
                        self.handle_provider_selection(selection);
                    }
                }
            }
            Event::Paste(text) => {
                self.text_input.insert_paste(&text);
                self.refresh_command_menu();
            }
            Event::Resize(_, _) => {
                // Avoid full-screen clear on every resize event to reduce flicker.
                let _ = terminal;
            }
            _ => {}
        }
        Ok(())
    }

    // ======================== Rendering ========================

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // main content
                Constraint::Length(1), // bottom bar
            ])
            .split(size);

        let main_area = chunks[0];
        let input_area = self.render_main(frame, main_area);
        self.render_bottom_bar(frame, chunks[1]);

        // Overlay: command menu (above input area)
        if self.command_menu.is_visible() {
            let menu_area = Rect {
                x: input_area.x,
                y: main_area.y,
                width: input_area.width,
                height: input_area.y.saturating_sub(main_area.y),
            };
            self.command_menu.render(frame, menu_area, &self.theme);
        }

        // Overlay: selector popups (centered on full screen)
        self.model_selector.render(frame, size, &self.theme);
        self.agent_selector.render(frame, size, &self.theme);
        self.session_selector.render(frame, size, &self.theme);
        self.skill_selector.render(frame, size, &self.theme);
        self.subagent_selector.render(frame, size, &self.theme);
        self.provider_selector.render(frame, size, &self.theme);
        self.model_config_form.render_mut(frame, size, &self.theme);

        // Overlay: command palette (Ctrl+P)
        self.command_palette.render(frame, size, &self.theme);

        // Overlay: info popup (highest priority)
        if let Some(ref msg) = self.info_popup {
            super::widgets::render_info_popup(frame, size, msg, self.theme.primary);
        }
    }

    /// Render main content, returns the input box area (for command menu positioning)
    fn render_main(&mut self, frame: &mut Frame, area: Rect) -> Rect {
        let max_width = 75u16.min(area.width.saturating_sub(4));

        // Dynamic input height: content lines (1..6) + 2 (padding top + agent label row) + 1 (gap)
        let input_content_width = max_width.saturating_sub(2 + 4); // left bar(2) + inner padding(4)
        let visual_lines = self.text_input.visual_line_count_with_prefix(input_content_width, 0) as u16;
        let content_lines = visual_lines.max(1).min(6);
        let input_box_height = content_lines + 3; // +1 top padding, +1 gap, +1 agent label

        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20), // top space
                Constraint::Length(12),      // logo
                Constraint::Length(1),       // gap
                Constraint::Length(input_box_height), // input box
                Constraint::Length(2),       // gap + tip/status
                Constraint::Min(1),          // bottom space
            ])
            .split(area);

        // Logo
        self.render_logo(frame, v_chunks[1]);

        // Input box - centered horizontally
        let h_pad = area.width.saturating_sub(max_width) / 2;
        let input_area = Rect {
            x: area.x + h_pad,
            y: v_chunks[3].y,
            width: max_width,
            height: v_chunks[3].height,
        };
        self.render_input(frame, input_area);

        // Tip / status
        let tip_area = Rect {
            x: area.x + h_pad,
            y: v_chunks[4].y + 1,
            width: max_width,
            height: 1,
        };
        self.render_tip_or_status(frame, tip_area);

        input_area
    }

    fn render_input(&mut self, frame: &mut Frame, area: Rect) {
        let highlight_color = self.theme.primary;

        // Split: 2 cols for left bar, rest for content
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // left bar
                Constraint::Min(1),   // content
            ])
            .split(area);

        // Left bar: full-height ┃
        let bar_lines: Vec<Line> = (0..area.height)
            .map(|_| {
                Line::from(Span::styled(
                    " ┃",
                    Style::default().fg(highlight_color),
                ))
            })
            .collect();
        let bar = Paragraph::new(bar_lines);
        frame.render_widget(bar, h_chunks[0]);

        // Content area with background
        let content_area = h_chunks[1];

        // Fill background
        let bg = Paragraph::new(
            vec![Line::from(""); content_area.height as usize],
        )
        .style(Style::default().bg(self.theme.background_element));
        frame.render_widget(bg, content_area);

        // Inner content with padding
        let inner = Rect {
            x: content_area.x + 2,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(4),
            height: content_area.height.saturating_sub(1),
        };

        // Calculate how many lines are available for text input
        // Reserve 2 lines at the bottom: 1 gap + 1 agent label
        let text_height = inner.height.saturating_sub(2).max(1);
        let text_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: text_height,
        };

        // Render text input using shared TextInput component
        let style = TextInputStyle {
            first_line_prefix: "",
            continuation_prefix: "",
            placeholder: "Ask anything... or type / for commands".to_string(),
            text_style: Style::default().fg(Color::White),
            placeholder_style: Style::default().fg(self.theme.muted),
        };
        self.text_input.render(frame, text_area, &style, true);

        // Agent label + model name below input (with 1 line gap)
        if inner.height >= 3 {
            let mut spans = vec![Span::styled(
                &self.agent_type,
                Style::default().fg(highlight_color),
            )];
            if !self.model_display_name.is_empty() {
                spans.push(Span::styled(
                    " | ",
                    Style::default().fg(self.theme.muted),
                ));
                spans.push(Span::styled(
                    &self.model_display_name,
                    Style::default().fg(self.theme.muted),
                ));
            }
            let agent_line = Line::from(spans);
            let agent_area = Rect {
                x: inner.x,
                y: inner.y + text_height + 1,
                width: inner.width,
                height: 1,
            };
            frame.render_widget(Paragraph::new(agent_line), agent_area);
        }
    }

    fn render_tip_or_status(&self, frame: &mut Frame, area: Rect) {
        let line = if let Some(ref status) = self.status {
            Line::from(vec![
                Span::styled("● ", Style::default().fg(self.theme.success)),
                Span::styled(status.as_str(), Style::default().fg(self.theme.muted)),
            ])
        } else {
            Line::from(vec![
                Span::styled("● ", Style::default().fg(self.theme.warning)),
                Span::styled("Tip ", Style::default().fg(self.theme.warning)),
                Span::styled(self.tip, Style::default().fg(self.theme.muted)),
            ])
        };
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_bottom_bar(&self, frame: &mut Frame, area: Rect) {
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        let mcp_status = crate::get_mcp_status_text();

        // Determine MCP status color
        let mcp_color = if mcp_status.contains("Ready") {
            self.theme.success
        } else if mcp_status.contains("Failed") {
            self.theme.error
        } else {
            self.theme.warning
        };

        // Left: workspace path
        let left = Paragraph::new(Line::from(Span::styled(
            format!("  {}", self.workspace_display),
            Style::default().fg(self.theme.muted),
        )));
        frame.render_widget(left, area);

        // Right: MCP status | version
        let right = Paragraph::new(Line::from(vec![
            Span::styled(&mcp_status, Style::default().fg(mcp_color)),
            Span::styled(format!(" | {}  ", version), Style::default().fg(self.theme.muted)),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(right, area);
    }

    fn render_logo(&self, frame: &mut Frame, area: Rect) {
        let use_fancy_logo = area.width >= 80;
        let mut lines = vec![];
        lines.push(Line::from(""));

        if use_fancy_logo {
            let logo = vec![
                "  ██████╗ ██╗████████╗███████╗██╗   ██╗███╗   ██╗",
                "  ██╔══██╗██║╚══██╔══╝██╔════╝██║   ██║████╗  ██║",
                "  ██████╔╝██║   ██║   █████╗  ██║   ██║██╔██╗ ██║",
                "  ██╔══██╗██║   ██║   ██╔══╝  ██║   ██║██║╚██╗██║",
                "  ██████╔╝██║   ██║   ██║     ╚██████╔╝██║ ╚████║",
                "  ╚═════╝ ╚═╝   ╚═╝   ╚═╝      ╚═════╝ ╚═╝  ╚═══╝",
            ];

            let colors = [
                Color::Rgb(255, 0, 100),
                Color::Rgb(255, 100, 0),
                Color::Rgb(255, 200, 0),
                Color::Rgb(100, 255, 0),
                Color::Rgb(0, 255, 200),
                Color::Rgb(100, 100, 255),
            ];

            for (i, line) in logo.iter().enumerate() {
                lines.push(Line::from(Span::styled(
                    *line,
                    Style::default()
                        .fg(colors[i % colors.len()])
                        .add_modifier(Modifier::BOLD),
                )));
            }
        } else {
            let logo = vec![
                "  ____  _ _   _____            ",
                " | __ )(_) |_|  ___|   _ _ __  ",
                " |  _ \\| | __| |_ | | | | '_ \\ ",
                " | |_) | | |_|  _|| |_| | | | |",
                " |____/|_|\\__|_|   \\__,_|_| |_|",
            ];

            let colors = [
                Color::Cyan,
                Color::Blue,
                Color::Magenta,
                Color::Red,
                Color::Yellow,
            ];

            for (i, line) in logo.iter().enumerate() {
                lines.push(Line::from(Span::styled(
                    *line,
                    Style::default()
                        .fg(colors[i % colors.len()])
                        .add_modifier(Modifier::BOLD),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "AI agent-driven command-line programming assistant",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )));

        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        lines.push(Line::from(Span::styled(
            version,
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    // ======================== Input handling ========================

    fn handle_key(&mut self, key: KeyEvent) -> Option<StartupResult> {
        if key.kind != KeyEventKind::Press {
            return None;
        }

        // Clear transient status on any key press
        self.status = None;

        // ── Info popup intercepts all keys ──
        if self.info_popup.is_some() {
            self.info_popup = None;
            return None;
        }

        // ── Global popup navigation: Ctrl+W closes all popups ──
        if self.any_popup_visible() {
            match (key.code, key.modifiers) {
                (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                    self.close_all_popups();
                    return None;
                }
                _ => {}
            }
        }

        // ── Selector popups intercept all keys when active ──

        if self.model_selector.is_visible() {
            match key.code {
                KeyCode::Up => self.model_selector.move_up(),
                KeyCode::Down => self.model_selector.move_down(),
                KeyCode::Enter => {
                    if let Some(selected) = self.model_selector.confirm_selection() {
                        self.model_selector.hide();
                        self.apply_model_selection(&selected);
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(selected) = self.model_selector.confirm_selection() {
                        self.model_selector.hide();
                        self.edit_model(&selected);
                    }
                }
                KeyCode::Esc => self.navigate_back(),
                _ => {}
            }
            return None;
        }

        if self.agent_selector.is_visible() {
            match key.code {
                KeyCode::Up => self.agent_selector.move_up(),
                KeyCode::Down => self.agent_selector.move_down(),
                KeyCode::Enter => {
                    if let Some(selected) = self.agent_selector.confirm_selection() {
                        self.agent_selector.hide();
                        self.apply_agent_selection(&selected);
                    }
                }
                KeyCode::Esc => self.navigate_back(),
                _ => {}
            }
            return None;
        }

        if self.session_selector.is_visible() {
            let action = self.session_selector.handle_key_event(key);
            match action {
                SessionAction::Switch(item) => {
                    return Some(StartupResult::ContinueSession(item.session_id));
                }
                SessionAction::Delete(item) => {
                    self.handle_session_delete(&item);
                }
                SessionAction::Close => {
                    self.navigate_back();
                }
                SessionAction::None => {}
            }
            return None;
        }

        if self.skill_selector.is_visible() {
            match key.code {
                KeyCode::Up => self.skill_selector.move_up(),
                KeyCode::Down => self.skill_selector.move_down(),
                KeyCode::Enter => {
                    if let Some(selected) = self.skill_selector.confirm_selection() {
                        self.skill_selector.hide();
                        self.set_input(&format!("Execute the {} skill.", selected.name));
                    }
                }
                KeyCode::Esc => self.navigate_back(),
                _ => {}
            }
            return None;
        }

        if self.subagent_selector.is_visible() {
            match key.code {
                KeyCode::Up => self.subagent_selector.move_up(),
                KeyCode::Down => self.subagent_selector.move_down(),
                KeyCode::Enter => {
                    if let Some(selected) = self.subagent_selector.confirm_selection() {
                        self.subagent_selector.hide();
                        self.set_input(&format!("Launch subagent {} to finish task: ", selected.name));
                    }
                }
                KeyCode::Esc => self.navigate_back(),
                _ => {}
            }
            return None;
        }

        if self.provider_selector.is_visible() {
            if let Some(selection) = self.provider_selector.handle_key_event(key) {
                self.handle_provider_selection(selection);
            }
            return None;
        }

        if self.model_config_form.is_visible() {
            let action = self.model_config_form.handle_key_event(key);
            match action {
                ModelFormAction::Save(result) => {
                    if result.editing_model_id.is_some() {
                        self.update_existing_model(result);
                    } else {
                        self.save_new_model(result);
                    }
                }
                ModelFormAction::Cancel => {
                    self.navigate_back();
                    self.status = Some("Model form cancelled".to_string());
                }
                ModelFormAction::None => {}
            }
            return None;
        }

        // ── Command palette intercepts all keys when visible ──

        if self.command_palette.is_visible() {
            let action = self.command_palette.handle_key_event(key);
            match action {
                PaletteAction::Execute(id) => {
                    return self.handle_palette_action(&id);
                }
                PaletteAction::Dismiss => {
                    self.navigate_back();
                }
                PaletteAction::None => {}
            }
            return None;
        }

        // ── Command menu navigation ──

        if self.command_menu.is_visible() {
            match key.code {
                KeyCode::Up => {
                    self.command_menu.move_up();
                    return None;
                }
                KeyCode::Down => {
                    self.command_menu.move_down();
                    return None;
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self.command_menu.apply_selection() {
                        return self.handle_command(&cmd);
                    }
                    return None;
                }
                KeyCode::Esc => {
                    self.text_input.clear();
                    self.command_menu.update_with_commands("", 0, STARTUP_COMMAND_SPECS);
                    return None;
                }
                _ => {
                    // Fall through to normal input handling, which updates the menu
                }
            }
        }

        // ── Normal key handling ──

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                return Some(StartupResult::Exit);
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.push_current_popup_to_stack();
                self.command_palette.show();
                return None;
            }
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.text_input.insert_paste(&text);
                        self.refresh_command_menu();
                    }
                }
            }
            (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => {
                self.text_input.handle_newline();
                self.refresh_command_menu();
            }
            (KeyCode::Enter, _) => {
                if let Some(cmd) = self.command_menu.apply_selection() {
                    return self.handle_command(&cmd);
                }

                if self.text_input.is_empty() {
                    return Some(StartupResult::NewSession { prompt: None });
                }
                let trimmed = self.text_input.text().trim().to_string();
                if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
                    return Some(StartupResult::Exit);
                }
                if trimmed.starts_with('/') {
                    return self.handle_command(&trimmed);
                }
                return Some(StartupResult::NewSession {
                    prompt: Some(trimmed),
                });
            }
            (KeyCode::Esc, _) => {
                if !self.text_input.is_empty() {
                    self.text_input.clear();
                    self.refresh_command_menu();
                }
            }
            (KeyCode::Tab, _) => {
                self.cycle_agent(1);
            }
            (KeyCode::BackTab, _) => {
                self.cycle_agent(-1);
            }
            (KeyCode::Up, KeyModifiers::NONE) => {
                if !self.text_input.move_cursor_up() {
                    self.text_input.set_cursor_home();
                }
                self.refresh_command_menu();
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                if !self.text_input.move_cursor_down() {
                    self.text_input.set_cursor_end();
                }
                self.refresh_command_menu();
            }
            (KeyCode::Char(c), _) => {
                self.text_input.handle_char(c);
                self.refresh_command_menu();
            }
            (KeyCode::Backspace, _) => {
                self.text_input.handle_backspace();
                self.refresh_command_menu();
            }
            (KeyCode::Delete, _) => {
                self.text_input.handle_delete();
                self.refresh_command_menu();
            }
            (KeyCode::Left, _) => {
                self.text_input.move_cursor_left();
            }
            (KeyCode::Right, _) => {
                self.text_input.move_cursor_right();
            }
            (KeyCode::Home, _) => {
                self.text_input.set_cursor_home();
            }
            (KeyCode::End, _) => {
                self.text_input.set_cursor_end();
            }
            _ => {}
        }
        None
    }

    // ======================== Palette action execution ========================

    fn handle_palette_action(&mut self, action_id: &str) -> Option<StartupResult> {
        match action_id {
            // Session group
            "new_session" => {
                return Some(StartupResult::NewSession { prompt: None });
            }
            "sessions" => {
                self.show_session_selector();
            }
            // Prompt group
            "skills" => {
                self.show_skill_selector();
            }
            "subagents" => {
                self.show_subagent_selector();
            }
            // Models group
            "select_model" => {
                self.show_model_selector();
            }
            "add_model" => {
                self.push_current_popup_to_stack();
                self.provider_selector.show();
            }
            // Agent group
            "switch_agent" => {
                self.show_agent_selector();
            }
            // MCP group
            "mcp_servers" => {
                return Some(StartupResult::NewSession {
                    prompt: Some("/mcps".to_string()),
                });
            }
            // System group
            "help" => {
                self.info_popup = Some(KEYBOARD_SHORTCUTS_HELP.to_string());
            }
            "exit" => {
                return Some(StartupResult::Exit);
            }
            _ => {
                self.status = Some(format!("Unknown palette action: {}", action_id));
            }
        }
        None
    }

    // ======================== Command execution ========================

    fn handle_command(&mut self, command: &str) -> Option<StartupResult> {
        let cmd = command.split_whitespace().next().unwrap_or("");

        self.text_input.clear();
        self.refresh_command_menu();

        match cmd {
            "/help" => {
                self.info_popup = Some(KEYBOARD_SHORTCUTS_HELP.to_string());
            }
            "/exit" => {
                return Some(StartupResult::Exit);
            }
            "/sessions" => {
                self.show_session_selector();
            }
            "/models" => {
                self.show_model_selector();
            }
            "/connect" => {
                self.push_current_popup_to_stack();
                self.provider_selector.show();
            }
            "/agents" => {
                self.show_agent_selector();
            }
            "/skills" => {
                self.show_skill_selector();
            }
            "/subagents" => {
                self.show_subagent_selector();
            }
            "/mcps" => {
                // Enter chat mode and auto-trigger /mcps command
                return Some(StartupResult::NewSession {
                    prompt: Some("/mcps".to_string()),
                });
            }
            "/init" => {
                match crate::prompts::get_cli_prompt("init") {
                    Some(prompt) => {
                        return Some(StartupResult::NewSession {
                            prompt: Some(prompt.to_string()),
                        });
                    }
                    None => {
                        self.status = Some("Init prompt not found".to_string());
                    }
                }
            }
            _ => {
                self.status = Some(format!("Unknown command: {}. Type /help for available commands.", cmd));
            }
        }

        None
    }

    // ======================== Selectors ========================

    /// Push the currently visible popup onto the navigation stack and hide it
    fn push_current_popup_to_stack(&mut self) {
        if self.command_palette.is_visible() {
            self.popup_stack.push(PopupType::CommandPalette);
            self.command_palette.hide();
        } else if self.model_selector.is_visible() {
            self.popup_stack.push(PopupType::ModelSelector);
            self.model_selector.hide();
        } else if self.agent_selector.is_visible() {
            self.popup_stack.push(PopupType::AgentSelector);
            self.agent_selector.hide();
        } else if self.session_selector.is_visible() {
            self.popup_stack.push(PopupType::SessionSelector);
            self.session_selector.hide();
        } else if self.skill_selector.is_visible() {
            self.popup_stack.push(PopupType::SkillSelector);
            self.skill_selector.hide();
        } else if self.subagent_selector.is_visible() {
            self.popup_stack.push(PopupType::SubagentSelector);
            self.subagent_selector.hide();
        } else if self.provider_selector.is_visible() {
            self.popup_stack.push(PopupType::ProviderSelector);
            self.provider_selector.hide();
        } else if self.model_config_form.is_visible() {
            self.popup_stack.push(PopupType::ModelConfigForm);
            self.model_config_form.hide();
        }
    }

    fn show_session_selector(&mut self) {
        self.push_current_popup_to_stack();
        let coordinator = self.coordinator.clone();
        let sessions = tokio::task::block_in_place(|| {
            let workspace_path = self.workspace_path_buf();
            tokio::runtime::Handle::current().block_on(async {
                coordinator
                    .list_sessions(&workspace_path)
                    .await
                    .unwrap_or_default()
            })
        });

        if sessions.is_empty() {
            self.status = Some("No sessions found.".to_string());
            return;
        }

        let session_items: Vec<SessionItem> = sessions
            .into_iter()
            .map(|s| {
                let last_activity = {
                    let elapsed = s.last_activity_at.elapsed().unwrap_or_default();
                    if elapsed.as_secs() < 60 {
                        "just now".to_string()
                    } else if elapsed.as_secs() < 3600 {
                        format!("{}m ago", elapsed.as_secs() / 60)
                    } else if elapsed.as_secs() < 86400 {
                        format!("{}h ago", elapsed.as_secs() / 3600)
                    } else {
                        format!("{}d ago", elapsed.as_secs() / 86400)
                    }
                };
                SessionItem {
                    session_id: s.session_id,
                    session_name: s.session_name,
                    last_activity,
                    workspace: Some(self.workspace_display.clone()),
                }
            })
            .collect();

        self.session_selector.show(session_items, None);
    }

    fn handle_session_delete(&mut self, item: &SessionItem) {
        let coordinator = self.coordinator.clone();
        let sid = item.session_id.clone();

        let result = tokio::task::block_in_place(|| {
            let workspace_path = self.workspace_path_buf();
            tokio::runtime::Handle::current().block_on(async {
                coordinator.delete_session(&workspace_path, &sid).await
            })
        });

        match result {
            Ok(()) => {
                self.session_selector.remove_item(&item.session_id);
                self.status = Some(format!("Session deleted: {}", item.session_name));
            }
            Err(e) => {
                self.status = Some(format!("Failed to delete session: {}", e));
            }
        }
    }

    fn show_model_selector(&mut self) {
        self.push_current_popup_to_stack();

        let agent_type = self.agent_type.clone();
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                let current_model_id = global_config
                    .ai
                    .agent_models
                    .get(&agent_type)
                    .cloned()
                    .or_else(|| global_config.ai.default_models.primary.clone());

                let model_items: Vec<ModelItem> = models
                    .into_iter()
                    .filter(|m| m.enabled)
                    .map(|m| ModelItem {
                        id: m.id,
                        name: m.name,
                        provider: m.provider,
                        model_name: m.model_name,
                    })
                    .collect();

                Some((model_items, current_model_id))
            })
        });

        match result {
            Some((models, current_id)) if !models.is_empty() => {
                self.model_selector.show(models, current_id);
            }
            _ => {
                self.status = Some("No available models found.".to_string());
            }
        }
    }

    fn apply_model_selection(&mut self, selected: &ModelItem) {
        let selected_id = selected.id.clone();
        let selected_display_name = format!("{} / {}", selected.model_name, selected.name);
        let modes = self.get_mode_agents();

        let success = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(_) => return false,
                };

                if let Err(e) = config_service
                    .set_config("ai.default_models.primary", &selected_id)
                    .await
                {
                    tracing::error!("Failed to set default primary model: {}", e);
                    return false;
                }

                for mode in &modes {
                    let path = format!("ai.agent_models.{}", mode.id);
                    if let Err(e) = config_service.set_config(&path, &selected_id).await {
                        tracing::error!("Failed to set model for mode '{}': {}", mode.id, e);
                    }
                }

                true
            })
        });

        if success {
            self.model_display_name = selected_display_name.clone();
            self.status = Some(format!("Model switched to: {}", selected_display_name));
        } else {
            self.status = Some("Failed to switch model".to_string());
        }
    }

    /// Handle provider selection result (step 1 → step 2 of add model)
    fn handle_provider_selection(&mut self, selection: ProviderSelection) {
        match selection {
            ProviderSelection::Provider(template) => {
                let default_model = template.models.first().cloned().unwrap_or_default();
                self.model_config_form.show_from_provider(
                    &template.name,
                    &template.base_url,
                    &template.format,
                    &default_model,
                );
            }
            ProviderSelection::Custom => {
                self.model_config_form.show_custom();
            }
        }
    }

    /// Save new model to global config
    fn save_new_model(&mut self, result: ModelFormResult) {
        let model_id = format!(
            "model_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty() || result.custom_headers_mode == "merge" {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let result_name = result.name.clone();
        let result_model_display =
            format!("{} / {}", result.model_name, result.name);

        let success = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service.add_ai_model(model_config).await {
                    tracing::error!("Failed to add AI model: {}", e);
                    return false;
                }

                // Auto-set as primary model if no primary model exists
                match config_service
                    .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                    .await
                {
                    Ok(global_config) => {
                        let has_primary = global_config
                            .ai
                            .default_models
                            .primary
                            .as_ref()
                            .map(|p| !p.is_empty())
                            .unwrap_or(false);
                        if !has_primary {
                            if let Err(e) = config_service
                                .set_config("ai.default_models.primary", &model_id)
                                .await
                            {
                                tracing::warn!("Failed to auto-set primary model: {}", e);
                            } else {
                                tracing::info!("Auto-set primary model: {}", model_id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read config for auto-primary: {}", e);
                    }
                }

                true
            })
        });

        if success {
            self.model_display_name = result_model_display;
            self.status = Some(format!("Model added: {}", result_name));
            tracing::info!("Added new AI model: {}", model_id);
            // Reload model name display
            self.load_current_model_name();
        } else {
            self.status = Some("Failed to add model".to_string());
        }
    }

    /// Fetch full model config and open the edit form
    fn edit_model(&mut self, selected: &ModelItem) {
        let model_id = selected.id.clone();
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                models.into_iter().find(|m| m.id == model_id)
            })
        });

        match result {
            Some(model) => {
                let form_data = ModelFormResult {
                    editing_model_id: Some(model.id.clone()),
                    name: model.name,
                    model_name: model.model_name,
                    base_url: model.base_url,
                    api_key: model.api_key,
                    provider_format: model.provider.clone(),
                    context_window: model.context_window.unwrap_or(128000),
                    max_tokens: model.max_tokens.unwrap_or(8192),
                    enable_thinking: model.enable_thinking_process,
                    support_preserved_thinking: model.inline_think_in_text,
                    skip_ssl_verify: model.skip_ssl_verify,
                    custom_headers: model.custom_headers
                        .map(|h| serde_json::to_string(&h).unwrap_or_default())
                        .unwrap_or_default(),
                    custom_headers_mode: model.custom_headers_mode.unwrap_or_else(|| "merge".to_string()),
                    custom_request_body: model.custom_request_body.unwrap_or_default(),
                };
                self.model_config_form.show_for_edit(&model.id, &form_data);
            }
            None => {
                self.status = Some("Failed to load model configuration".to_string());
            }
        }
    }

    /// Update an existing model in global config
    fn update_existing_model(&mut self, result: ModelFormResult) {
        let model_id = match &result.editing_model_id {
            Some(id) => id.clone(),
            None => return,
        };

        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty() || result.custom_headers_mode == "merge" {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let result_name = result.name.clone();
        let result_model_display =
            format!("{} / {}", result.model_name, result.name);

        let success = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service.update_ai_model(&model_id, model_config).await {
                    tracing::error!("Failed to update AI model: {}", e);
                    return false;
                }

                true
            })
        });

        if success {
            self.model_display_name = result_model_display;
            self.status = Some(format!("Model updated: {}", result_name));
            tracing::info!("Updated AI model: {}", model_id);
            self.load_current_model_name();
        } else {
            self.status = Some("Failed to update model".to_string());
        }
    }

    fn show_agent_selector(&mut self) {
        self.push_current_popup_to_stack();

        let modes = self.get_mode_agents();
        if modes.is_empty() {
            self.status = Some("No mode agents available".to_string());
            return;
        }

        let agent_items: Vec<AgentItem> = modes
            .into_iter()
            .map(|m| AgentItem {
                id: m.id,
                description: m.description,
            })
            .collect();

        self.agent_selector.show(agent_items, Some(self.agent_type.clone()));
    }

    fn apply_agent_selection(&mut self, selected: &AgentItem) {
        if selected.id != self.agent_type {
            self.agent_type = selected.id.clone();
            self.status = Some(format!("Agent switched to: {}", selected.id));
            // Reload model name for new agent
            self.load_current_model_name();
        }
    }

    fn show_skill_selector(&mut self) {
        self.push_current_popup_to_stack();

        let skills = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let registry = SkillRegistry::global();
                registry.refresh().await;
                registry.get_all_skills().await
            })
        });

        if skills.is_empty() {
            self.status = Some("No skills found.".to_string());
            return;
        }

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(|s| SkillItem {
                name: s.name,
                description: s.description,
                level: s.level.as_str().to_string(),
            })
            .collect();

        if skill_items.is_empty() {
            self.status = Some("No skills found.".to_string());
            return;
        }

        self.skill_selector.show(skill_items);
    }

    fn show_subagent_selector(&mut self) {
        self.push_current_popup_to_stack();

        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.workspace_path_buf();
            tokio::runtime::Handle::current()
                .block_on(registry.get_subagents_info(Some(workspace.as_path())))
        });

        if subagents.is_empty() {
            self.status = Some("No subagents found.".to_string());
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(|s| {
                let source = match s.subagent_source {
                    Some(bitfun_core::agentic::agents::SubAgentSource::Builtin) => "builtin".to_string(),
                    Some(bitfun_core::agentic::agents::SubAgentSource::Project) => "project".to_string(),
                    Some(bitfun_core::agentic::agents::SubAgentSource::User) => "user".to_string(),
                    None => "builtin".to_string(),
                };
                SubagentItem {
                    id: s.id,
                    name: s.name,
                    description: s.description,
                    source,
                }
            })
            .collect();

        if subagent_items.is_empty() {
            self.status = Some("No subagents found.".to_string());
            return;
        }

        self.subagent_selector.show(subagent_items);
    }

    // ======================== Helpers ========================

    /// Navigate back to the previous popup in the stack, or close current if at the root
    fn navigate_back(&mut self) {
        // First hide the currently visible popup
        if self.command_palette.is_visible() {
            self.command_palette.hide();
        } else if self.model_selector.is_visible() {
            self.model_selector.hide();
        } else if self.agent_selector.is_visible() {
            self.agent_selector.hide();
        } else if self.session_selector.is_visible() {
            self.session_selector.hide();
        } else if self.skill_selector.is_visible() {
            self.skill_selector.hide();
        } else if self.subagent_selector.is_visible() {
            self.subagent_selector.hide();
        } else if self.provider_selector.is_visible() {
            self.provider_selector.hide();
        } else if self.model_config_form.is_visible() {
            self.model_config_form.hide();
        }

        // If there's a previous popup in the stack, re-show it
        if let Some(previous) = self.popup_stack.pop() {
            match previous {
                PopupType::CommandPalette => self.command_palette.reshow(),
                PopupType::ModelSelector => self.model_selector.reshow(),
                PopupType::AgentSelector => self.agent_selector.reshow(),
                PopupType::SessionSelector => self.session_selector.reshow(),
                PopupType::SkillSelector => self.skill_selector.reshow(),
                PopupType::SubagentSelector => self.subagent_selector.reshow(),
                PopupType::ProviderSelector => self.provider_selector.reshow(),
                PopupType::ModelConfigForm => self.model_config_form.reshow(),
            }
        }
    }

    /// Close all popups and clear the navigation stack
    fn close_all_popups(&mut self) {
        self.command_palette.hide();
        self.model_selector.hide();
        self.agent_selector.hide();
        self.session_selector.hide();
        self.skill_selector.hide();
        self.subagent_selector.hide();
        self.provider_selector.hide();
        self.model_config_form.hide();
        self.popup_stack.clear();
    }

    fn get_mode_agents(&self) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        let modes = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(registry.get_modes_info())
        });
        modes
    }

    fn cycle_agent(&mut self, offset: isize) {
        let modes = self.get_mode_agents();
        if modes.len() <= 1 {
            return;
        }

        let current_idx = modes
            .iter()
            .position(|m| m.id == self.agent_type)
            .unwrap_or(0);

        let len = modes.len() as isize;
        let next_idx = ((current_idx as isize + offset) % len + len) % len;
        let next = &modes[next_idx as usize];

        self.agent_type = next.id.clone();
        self.load_current_model_name();
    }

    fn load_current_model_name(&mut self) {
        let agent_type = self.agent_type.clone();
        let result: Option<String> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                let model_id = global_config
                    .ai
                    .agent_models
                    .get(&agent_type)
                    .cloned()
                    .or_else(|| global_config.ai.default_models.primary.clone())
                    .unwrap_or_else(|| "primary".to_string());

                fn provider_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    let raw_name = model.name.trim();
                    let model_name = model.model_name.trim();
                    if !raw_name.is_empty() && !model_name.is_empty() {
                        let dashed_suffix = format!(" - {}", model_name);
                        let slash_suffix = format!("/{}", model_name);
                        if let Some(provider) = raw_name.strip_suffix(&dashed_suffix) {
                            return provider.trim().to_string();
                        }
                        if let Some(provider) = raw_name.strip_suffix(&slash_suffix) {
                            return provider.trim().to_string();
                        }
                    }
                    if raw_name.is_empty() {
                        model.provider.clone()
                    } else {
                        raw_name.to_string()
                    }
                }

                fn model_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    format!("{} / {}", model.model_name, provider_display_name(model))
                }

                if model_id == "primary" {
                    let primary_id = global_config.ai.default_models.primary.as_deref()?;
                    models
                        .iter()
                        .find(|m| m.id == primary_id)
                        .map(model_display_name)
                } else {
                    models
                        .iter()
                        .find(|m| m.id == model_id)
                        .map(model_display_name)
                }
            })
        });

        self.model_display_name = result.unwrap_or_default();
    }

    fn set_input(&mut self, text: &str) {
        self.text_input.set_text(text);
        self.refresh_command_menu();
    }

    fn refresh_command_menu(&mut self) {
        self.command_menu.update_with_commands(
            &self.text_input.input,
            self.text_input.cursor,
            STARTUP_COMMAND_SPECS,
        );
    }
}
