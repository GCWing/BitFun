use super::agent_selector::{AgentItem, AgentSelectorState};
use super::command_menu::CommandMenuState;
use super::command_palette::{CommandPaletteState, PaletteAction};
use super::login_form::{LoginFormAction, LoginFormState};
use super::model_config_form::{ModelConfigFormState, ModelFormAction, ModelFormResult};
use super::model_selector::{ModelItem, ModelSelectorState};
use super::provider_selector::{ProviderSelection, ProviderSelectorState};
use super::session_selector::{SessionAction, SessionItem, SessionSelectorState};
use super::skill_selector::{SkillItem, SkillSelectorAction, SkillSelectorState};
use super::subagent_selector::{SubagentItem, SubagentSelectorAction, SubagentSelectorState};
use super::text_input::{TextInput, TextInputStyle};
use super::theme::{
    builtin_theme_ids, builtin_theme_json, resolve_appearance, resolve_effective_color_scheme,
    Appearance, EffectiveColorScheme, Theme,
};
use super::theme_selector::{ThemeItem, ThemeSelectorState};
use crate::actions::{
    action_by_id, action_for_alias, ActionContext, ActionHandler, ActionSpec, ActionState,
    ResolvedKeymap,
};
use crate::config::CliConfig;
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
    widgets::{Block, Paragraph},
    Frame, Terminal,
};
use std::time::Duration;

use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_core::agentic::agents::{
    get_agent_registry, AgentInfo, SubAgentSource, SubagentListScope, SubagentQueryContext,
};
use bitfun_core::agentic::tools::implementations::skills::{
    mode_overrides::{
        load_project_mode_skills_document_local, save_project_mode_skills_document_local,
        set_mode_skill_disabled_in_document, set_user_mode_skill_state,
    },
    registry::SkillRegistry,
    ModeSkillInfo, SkillInfo,
};
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::service::config::GlobalConfigManager;

/// Types of popups that can be shown on the startup page
#[derive(Debug, Clone, PartialEq)]
enum PopupType {
    CommandPalette,
    ModelSelector,
    AgentSelector,
    SessionSelector,
    SkillSelector,
    SubagentSelector,
    ThemeSelector,
    ProviderSelector,
    ModelConfigForm,
    LoginForm,
}

/// Navigation stack for managing popup hierarchy
#[derive(Debug, Default)]
struct PopupStack {
    stack: Vec<PopupType>,
}

impl PopupStack {
    fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a popup onto the stack
    fn push(&mut self, popup: PopupType) {
        // Avoid duplicates at the top
        if self.stack.last() != Some(&popup) {
            self.stack.push(popup);
        }
    }

    /// Pop the top popup from the stack
    fn pop(&mut self) -> Option<PopupType> {
        self.stack.pop()
    }

    /// Clear all popups from the stack
    fn clear(&mut self) {
        self.stack.clear();
    }
}

/// Startup menu result
#[derive(Debug, Clone)]
pub(crate) enum StartupResult {
    /// Start a new session with an optional initial prompt
    NewSession { prompt: Option<String> },
    /// Continue last session (session ID)
    ContinueSession(String),
    /// User cancelled exit
    Exit,
}

/// Random tips shown on the startup page
const TIPS: &[&str] = &[
    "Type / for slash commands (e.g. /help, /login, /models)",
    "Press Tab to cycle between agents",
    "Use /login to sign in for Peer Device Mode / multi-device sync",
    "Use /init to explore your repo and generate AGENTS.md",
    "Press Ctrl+E to toggle browse mode for scrolling history",
    "Use /sessions to list and continue previous conversations",
    "Press Ctrl+O to expand/collapse tool output",
    "Use /skills to browse and execute available skills",
    "Use /usage inside a session to generate a usage report",
    "Use /theme to switch the CLI theme",
    "Use /acp to copy editor setup commands for ACP hosts",
    "Press Up/Down to cycle through input history",
    "Use /new to start a fresh conversation session",
];

const FANCY_LOGO: [&str; 6] = [
    "  ██████╗ ██╗████████╗███████╗██╗   ██╗███╗   ██╗",
    "  ██╔══██╗██║╚══██╔══╝██╔════╝██║   ██║████╗  ██║",
    "  ██████╔╝██║   ██║   █████╗  ██║   ██║██╔██╗ ██║",
    "  ██╔══██╗██║   ██║   ██╔══╝  ██║   ██║██║╚██╗██║",
    "  ██████╔╝██║   ██║   ██║     ╚██████╔╝██║ ╚████║",
    "  ╚═════╝ ╚═╝   ╚═╝   ╚═╝      ╚═════╝ ╚═╝  ╚═══╝",
];

const COMPACT_LOGO: [&str; 5] = [
    "  ____  _ _   _____            ",
    " | __ )(_) |_|  ___|   _ _ __  ",
    " |  _ \\| | __| |_ | | | | '_ \\ ",
    " | |_) | | |_|  _|| |_| | | | |",
    " |____/|_|\\__|_|   \\__,_|_| |_|",
];

fn append_styled_logo_lines(
    lines: &mut Vec<Line<'static>>,
    logo: &'static [&'static str],
    colors: &[Color],
) {
    for (index, line) in logo.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            *line,
            Style::default()
                .fg(colors[index % colors.len()])
                .add_modifier(Modifier::BOLD),
        )));
    }
}

/// Startup page
pub(crate) struct StartupPage {
    /// Multiline text input component
    text_input: TextInput,
    /// Theme
    theme: Theme,
    /// CLI config, including persisted theme preference.
    config: CliConfig,
    /// Resolved host-owned action bindings for the current config.
    keymap: ResolvedKeymap,
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
    theme_selector: ThemeSelectorState,
    provider_selector: ProviderSelectorState,
    model_config_form: ModelConfigFormState,
    login_form: LoginFormState,
    theme_preview_original: Option<Theme>,

    // ── System context ──
    agent_runtime: AgentRuntime,
    compatibility: CoreAgentRuntimeCompatibility,

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
    pub(crate) fn new(
        config: CliConfig,
        agent_runtime: AgentRuntime,
        compatibility: CoreAgentRuntimeCompatibility,
        default_agent: String,
        workspace: Option<String>,
    ) -> Self {
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

        let keymap = ResolvedKeymap::new(&config.shortcuts);
        let mut page = Self {
            text_input: TextInput::new(),
            theme,
            config,
            keymap,
            tip: TIPS[tip_index],
            command_menu: CommandMenuState::new(ActionState::startup(false)),
            command_palette: CommandPaletteState::new(),
            model_selector: ModelSelectorState::new(),
            agent_selector: AgentSelectorState::new(),
            session_selector: SessionSelectorState::new(),
            skill_selector: SkillSelectorState::new(),
            subagent_selector: SubagentSelectorState::new(),
            theme_selector: ThemeSelectorState::new(),
            provider_selector: ProviderSelectorState::new(),
            model_config_form: ModelConfigFormState::new(),
            login_form: LoginFormState::new(),
            theme_preview_original: None,
            agent_runtime,
            compatibility,
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
    pub(crate) fn agent_type(&self) -> &str {
        &self.agent_type
    }

    /// Get the current workspace path for this CLI process.
    pub(crate) fn workspace(&self) -> Option<String> {
        if self.workspace_display.is_empty() {
            None
        } else {
            Some(self.workspace_display.clone())
        }
    }

    /// Get the current CLI config after startup-page edits.
    pub(crate) fn config(&self) -> &CliConfig {
        &self.config
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
            || self.theme_selector.is_visible()
            || self.provider_selector.is_visible()
            || self.model_config_form.is_visible()
            || self.login_form.is_visible()
    }

    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<StartupResult> {
        terminal.clear()?;

        loop {
            if self.login_form.is_visible() {
                self.refresh_account_panel_live();
            }
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
                            if self.login_form.is_visible() {
                                self.login_form.insert_paste(&paste_buf);
                            } else {
                                self.text_input.insert_paste(&paste_buf);
                                self.refresh_command_menu();
                            }
                        }
                        for ev in non_key_events {
                            if let Some(result) = self.handle_non_key_event(ev, terminal)? {
                                return Ok(result);
                            }
                        }
                    } else {
                        for ev in events {
                            match ev {
                                Event::Key(key)
                                    if key.kind == KeyEventKind::Press
                                        || key.kind == KeyEventKind::Repeat =>
                                {
                                    if let Some(result) = self.handle_key(key) {
                                        return Ok(result);
                                    }
                                }
                                other => {
                                    if let Some(result) =
                                        self.handle_non_key_event(other, terminal)?
                                    {
                                        return Ok(result);
                                    }
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
    ) -> Result<Option<StartupResult>> {
        match ev {
            Event::Mouse(mouse) => {
                if self.command_palette.captures_mouse(&mouse) {
                    let action = self.command_palette.handle_mouse_event(&mouse);
                    match action {
                        PaletteAction::Execute(id) => {
                            return Ok(self.handle_palette_action(&id));
                        }
                        PaletteAction::Dismiss => self.navigate_back(),
                        PaletteAction::None => {}
                    }
                } else if self.theme_selector.captures_mouse(&mouse) {
                    self.theme_selector.handle_mouse_event(&mouse);
                    if let Some(selected) = self.theme_selector.selected_item().cloned() {
                        self.preview_theme_selection(&selected);
                    }
                } else if self.provider_selector.captures_mouse(&mouse) {
                    if let Some(selection) = self.provider_selector.handle_mouse_event(&mouse) {
                        self.handle_provider_selection(selection);
                    }
                } else if self.command_menu.captures_mouse(&mouse) {
                    if let Some(action_id) = self.command_menu.handle_mouse_event(&mouse) {
                        self.text_input.clear();
                        self.refresh_command_menu();
                        return Ok(self.handle_palette_action(&action_id));
                    }
                }
            }
            Event::Paste(text) => {
                if self.login_form.is_visible() {
                    self.login_form.insert_paste(&text);
                } else {
                    self.text_input.insert_paste(&text);
                    self.refresh_command_menu();
                }
            }
            Event::Resize(_, _) => {
                // Avoid full-screen clear on every resize event to reduce flicker.
                let _ = terminal;
            }
            _ => {}
        }
        Ok(None)
    }

    // ======================== Rendering ========================

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();
        frame.render_widget(
            Block::default().style(Style::default().bg(self.theme.background)),
            size,
        );

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
        self.theme_selector.render(frame, size, &self.theme);
        self.provider_selector.render(frame, size, &self.theme);
        self.model_config_form.render_mut(frame, size, &self.theme);

        // Overlay: command palette (Ctrl+P)
        self.command_palette.render(frame, size, &self.theme);

        // Dedicated login page (full viewport takeover)
        self.login_form.render(frame, size, &self.theme);

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
        let visual_lines =
            self.text_input
                .visual_line_count_with_prefix(input_content_width, 0) as u16;
        let content_lines = visual_lines.clamp(1, 6);
        let input_box_height = content_lines + 3; // +1 top padding, +1 gap, +1 agent label

        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),           // top space
                Constraint::Length(12),               // logo
                Constraint::Length(1),                // gap
                Constraint::Length(input_box_height), // input box
                Constraint::Length(2),                // gap + tip/status
                Constraint::Min(1),                   // bottom space
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
        let input_bg = self.input_background();

        // Split: 2 cols for left bar, rest for content
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // left bar
                Constraint::Min(1),    // content
            ])
            .split(area);

        // Left bar: full-height ┃
        let bar_lines: Vec<Line> = (0..area.height)
            .map(|_| {
                Line::from(Span::styled(
                    " ┃",
                    Style::default().fg(highlight_color).bg(input_bg),
                ))
            })
            .collect();
        let bar = Paragraph::new(bar_lines).style(Style::default().bg(input_bg));
        frame.render_widget(bar, h_chunks[0]);

        // Content area with background
        let content_area = h_chunks[1];

        // Fill background
        let bg = Paragraph::new(vec![Line::from(""); content_area.height as usize])
            .style(Style::default().bg(input_bg));
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
            text_style: Style::default().fg(self.theme.command_text).bg(input_bg),
            placeholder_style: Style::default().fg(self.theme.muted).bg(input_bg),
        };
        self.text_input.render(frame, text_area, &style, true);

        // Agent label + model name below input (with 1 line gap)
        if inner.height >= 3 {
            let mut spans = vec![Span::styled(
                &self.agent_type,
                Style::default().fg(highlight_color),
            )];
            if !self.model_display_name.is_empty() {
                spans.push(Span::styled(" | ", Style::default().fg(self.theme.muted)));
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

    fn input_background(&self) -> ratatui::style::Color {
        self.theme.input_background
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
            Span::styled(
                format!(" | {}  ", version),
                Style::default().fg(self.theme.muted),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(right, area);
    }

    fn render_logo(&self, frame: &mut Frame, area: Rect) {
        let use_fancy_logo = area.width >= 80;
        let mut lines = vec![];
        lines.push(Line::from(""));

        if use_fancy_logo {
            let colors = [
                self.theme.primary,
                self.theme.info,
                self.theme.success,
                self.theme.warning,
                self.theme.error,
                self.theme.muted,
            ];

            append_styled_logo_lines(&mut lines, &FANCY_LOGO, &colors);
        } else {
            let colors = [
                self.theme.primary,
                self.theme.info,
                self.theme.success,
                self.theme.warning,
                self.theme.error,
            ];

            append_styled_logo_lines(&mut lines, &COMPACT_LOGO, &colors);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "AI agent-driven command-line programming assistant",
            Style::default()
                .fg(self.theme.muted)
                .add_modifier(Modifier::ITALIC),
        )));

        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        lines.push(Line::from(Span::styled(
            version,
            Style::default().fg(self.theme.muted),
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

        let modal_state =
            ActionState::startup(self.info_popup.is_some() || self.any_popup_visible());
        if let Some(action) = self.keymap.resolve_modal_safe(key, modal_state) {
            return self.dispatch_action(action, modal_state);
        }

        // ── Info popup intercepts all keys ──
        if self.info_popup.is_some() {
            self.info_popup = None;
            return None;
        }

        // Host recovery keys win over configured actions while a popup is open.
        if self.any_popup_visible() {
            let state = ActionState::startup(true);
            if let Some(action) = self.keymap.resolve_reserved(key, state) {
                return self.dispatch_action(action, state);
            }
        }

        // ── Selector popups intercept all keys when active ──

        if self.theme_selector.is_visible() {
            match key.code {
                KeyCode::Up => {
                    self.theme_selector.move_up();
                    if let Some(selected) = self.theme_selector.selected_item().cloned() {
                        self.preview_theme_selection(&selected);
                    }
                }
                KeyCode::Down => {
                    self.theme_selector.move_down();
                    if let Some(selected) = self.theme_selector.selected_item().cloned() {
                        self.preview_theme_selection(&selected);
                    }
                }
                KeyCode::Enter => {
                    if let Some(selected) = self.theme_selector.confirm_selection() {
                        self.theme_selector.hide();
                        self.apply_theme_selection(&selected);
                    }
                }
                KeyCode::Esc => self.navigate_back(),
                _ => {}
            }
            return None;
        }

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
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = self.skill_selector.confirm_selection() {
                        self.handle_skill_selector_action(action);
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
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = self.subagent_selector.confirm_selection() {
                        self.handle_subagent_selector_action(action);
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

        if self.login_form.is_visible() {
            self.refresh_account_panel_live();
            let action = self.login_form.handle_key_event(key);
            return self.handle_login_form_action(action);
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
                _ => {
                    // Fall through to normal input handling, which updates the menu
                }
            }
        }

        // ── Normal key handling ──

        if let Some(action) = self.keymap.resolve(key, ActionState::startup(false)) {
            return self.dispatch_action(action, ActionState::startup(false));
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                if !self.text_input.is_empty() {
                    self.text_input.clear();
                    self.refresh_command_menu();
                }
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
        let Some(action) = action_by_id(action_id, ActionContext::Startup) else {
            self.status = Some(format!("Unknown palette action: {action_id}"));
            return None;
        };
        self.dispatch_action(action, ActionState::startup(false))
    }

    fn dispatch_action(
        &mut self,
        action: &'static ActionSpec,
        state: ActionState,
    ) -> Option<StartupResult> {
        if !action.available(state) {
            self.status = Some(action.unavailable_message(state));
            return None;
        }

        match action.handler {
            ActionHandler::Help => {
                self.info_popup = Some(self.keymap.help_text(ActionState::startup(false)));
            }
            ActionHandler::Exit => return Some(StartupResult::Exit),
            ActionHandler::NewSession => {
                return Some(StartupResult::NewSession { prompt: None });
            }
            ActionHandler::Sessions => self.show_session_selector(),
            ActionHandler::SelectModel => self.show_model_selector(),
            ActionHandler::SelectTheme => self.show_theme_selector(),
            ActionHandler::AddModel => {
                self.push_current_popup_to_stack();
                self.provider_selector.show();
            }
            ActionHandler::OpenAgentSelector => self.show_agent_selector(),
            ActionHandler::SwitchAgent => self.cycle_agent(1),
            ActionHandler::SwitchAgentReverse => self.cycle_agent(-1),
            ActionHandler::Skills => self.show_skill_selector(),
            ActionHandler::Subagents => self.show_subagent_selector(),
            ActionHandler::McpServers => {
                return Some(StartupResult::NewSession {
                    prompt: Some("/mcps".to_string()),
                });
            }
            ActionHandler::AcpHelp => {
                return Some(StartupResult::NewSession {
                    prompt: Some("/acp".to_string()),
                });
            }
            ActionHandler::Login => self.show_login_form(),
            ActionHandler::Logout => self.logout(),
            ActionHandler::Usage => {
                self.status = Some("No active session for /usage.".to_string());
            }
            ActionHandler::Init => match crate::prompts::get_cli_prompt("init") {
                Some(prompt) => {
                    return Some(StartupResult::NewSession {
                        prompt: Some(prompt.to_string()),
                    });
                }
                None => self.status = Some("Init prompt not found".to_string()),
            },
            ActionHandler::OpenPalette => {
                self.push_current_popup_to_stack();
                self.command_palette.show(ActionState::startup(false));
            }
            ActionHandler::SubmitInput => return self.submit_input(),
            ActionHandler::InsertNewline => {
                self.text_input.handle_newline();
                self.refresh_command_menu();
            }
            ActionHandler::Paste => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.text_input.insert_paste(&text);
                        self.refresh_command_menu();
                    }
                }
            }
            ActionHandler::ClosePopups => self.close_all_popups(),
            ActionHandler::NavigateBack => self.navigate_back(),
            ActionHandler::ClearConversation
            | ActionHandler::ReloadSkills
            | ActionHandler::History
            | ActionHandler::Interrupt
            | ActionHandler::ToggleFocusedTool
            | ActionHandler::PreviousTool
            | ActionHandler::NextTool
            | ActionHandler::HistoryPrevious
            | ActionHandler::HistoryNext
            | ActionHandler::JumpTop
            | ActionHandler::JumpBottom
            | ActionHandler::ClearInput
            | ActionHandler::ToggleBrowse
            | ActionHandler::ScrollUp
            | ActionHandler::ScrollDown => {
                self.status = Some("Action is unavailable on the startup page.".to_string());
            }
        }
        None
    }

    fn submit_input(&mut self) -> Option<StartupResult> {
        if let Some(action_id) = self.command_menu.apply_selection() {
            self.text_input.clear();
            self.refresh_command_menu();
            return self.handle_palette_action(&action_id);
        }
        if self.text_input.is_empty() {
            return Some(StartupResult::NewSession { prompt: None });
        }

        let trimmed = self.text_input.text().trim().to_string();
        if trimmed == "exit" || trimmed == "quit" {
            return Some(StartupResult::Exit);
        }
        if trimmed.starts_with('/') {
            return self.handle_command(&trimmed);
        }
        Some(StartupResult::NewSession {
            prompt: Some(trimmed),
        })
    }

    fn logout(&mut self) {
        let logged_in = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::account::is_logged_in())
        });
        if !logged_in {
            self.status = Some("Not logged in.".to_string());
            return;
        }
        self.status = Some(
            match tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(crate::account::logout())
            }) {
                Ok(()) => "Logged out.".to_string(),
                Err(error) => format!("Logout failed: {error}"),
            },
        );
    }

    // ======================== Command execution ========================

    fn handle_command(&mut self, command: &str) -> Option<StartupResult> {
        let cmd = command.split_whitespace().next().unwrap_or("");

        self.text_input.clear();
        self.refresh_command_menu();
        let Some(action) = action_for_alias(cmd, ActionContext::Startup) else {
            self.status = Some(format!(
                "Unknown command: {cmd}. Type /help for available commands."
            ));
            return None;
        };
        self.dispatch_action(action, ActionState::startup(false))
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
        } else if self.theme_selector.is_visible() {
            self.popup_stack.push(PopupType::ThemeSelector);
            self.theme_selector.hide();
        } else if self.provider_selector.is_visible() {
            self.popup_stack.push(PopupType::ProviderSelector);
            self.provider_selector.hide();
        } else if self.model_config_form.is_visible() {
            self.popup_stack.push(PopupType::ModelConfigForm);
            self.model_config_form.hide();
        } else if self.login_form.is_visible() {
            self.popup_stack.push(PopupType::LoginForm);
            self.login_form.hide();
        }
    }

    fn show_login_form(&mut self) {
        self.close_all_popups();
        let logged_in = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::account::is_logged_in())
        });
        if logged_in {
            self.open_account_panel();
        } else {
            self.login_form.show();
        }
    }

    fn workspace_path_for_sync(&self) -> std::path::PathBuf {
        self.workspace_path_buf()
    }

    fn open_account_panel(&mut self) {
        let (info, devices, progress) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let info = crate::account::account_info().await;
                let devices = crate::account::list_devices().await.unwrap_or_default();
                let progress = crate::account_sync::current_sync_progress().await;
                (info, devices, progress)
            })
        });
        match info {
            Ok(info) => self.login_form.show_account(info, devices, progress),
            Err(e) => {
                self.status = Some(format!("Failed to load account: {e}"));
                self.login_form.show();
            }
        }
    }

    fn refresh_account_panel_live(&mut self) {
        if !self.login_form.is_visible() {
            return;
        }
        let progress = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::account_sync::current_sync_progress())
        });
        // Refresh devices occasionally while syncing / after done.
        let devices = if matches!(
            progress.status,
            crate::account_sync::SyncStatus::Syncing | crate::account_sync::SyncStatus::Done
        ) {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(crate::account::list_devices())
                    .ok()
            })
        } else {
            None
        };
        self.login_form.update_account_progress(devices, progress);
    }

    fn start_sync_and_show_account(&mut self, is_first_login: bool) {
        let workspace = self.workspace_path_for_sync();
        crate::account_sync::start_auto_sync_background(
            self.compatibility.clone(),
            is_first_login,
            workspace,
        );
        self.open_account_panel();
        self.status = Some(if is_first_login {
            "Sync started (use local / upload settings).".to_string()
        } else {
            "Sync started (use cloud / download settings).".to_string()
        });
    }

    fn handle_login_form_action(&mut self, action: LoginFormAction) -> Option<StartupResult> {
        match action {
            LoginFormAction::Submit(creds) => {
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(
                        crate::account::login_with_credentials(
                            &creds.relay_url,
                            &creds.username,
                            &creds.password,
                        ),
                    )
                });
                match result {
                    Ok(login) => {
                        self.status = Some(login.status_message.clone());
                        if login.has_cloud_settings {
                            self.login_form
                                .show_sync_choice(&login.user_id, &login.relay_url);
                        } else {
                            self.start_sync_and_show_account(true);
                        }
                    }
                    Err(e) => {
                        self.login_form.set_error(format!("Login failed: {e}"));
                    }
                }
            }
            LoginFormAction::SyncUseLocal => {
                self.start_sync_and_show_account(true);
            }
            LoginFormAction::SyncUseCloud => {
                self.start_sync_and_show_account(false);
            }
            LoginFormAction::SyncCancel => {
                let _ = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(crate::account::logout())
                });
                self.login_form.show();
                self.status = Some("Sync cancelled; logged out.".to_string());
            }
            LoginFormAction::Logout => {
                match tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(crate::account::logout())
                }) {
                    Ok(()) => {
                        self.login_form.show();
                        self.status = Some("Logged out.".to_string());
                    }
                    Err(e) => {
                        self.login_form.set_error(format!("Logout failed: {e}"));
                    }
                }
            }
            LoginFormAction::Cancel => {
                self.status = Some("Account panel closed".to_string());
            }
            LoginFormAction::None => {}
        }
        None
    }

    fn show_session_selector(&mut self) {
        self.push_current_popup_to_stack();
        let agent_runtime = self.agent_runtime.clone();
        let sessions = tokio::task::block_in_place(|| {
            let workspace_path = self.workspace_path_buf();
            tokio::runtime::Handle::current().block_on(async {
                agent_runtime
                    .list_sessions(bitfun_runtime_ports::AgentSessionListRequest {
                        workspace_path: workspace_path.to_string_lossy().to_string(),
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
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
                    let last_activity =
                        std::time::UNIX_EPOCH + Duration::from_millis(s.last_active_at_ms);
                    let elapsed = last_activity.elapsed().unwrap_or_default();
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
        let agent_runtime = self.agent_runtime.clone();
        let sid = item.session_id.clone();

        let result = tokio::task::block_in_place(|| {
            let workspace_path = self.workspace_path_buf();
            tokio::runtime::Handle::current().block_on(async {
                agent_runtime
                    .delete_session(bitfun_runtime_ports::AgentSessionDeleteRequest {
                        workspace_path: workspace_path.to_string_lossy().to_string(),
                        session_id: sid,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
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
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let result_name = result.name.clone();
        let result_model_display = format!("{} / {}", result.model_name, result.name);

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
                    custom_headers: model
                        .custom_headers
                        .map(|h| serde_json::to_string(&h).unwrap_or_default())
                        .unwrap_or_default(),
                    custom_headers_mode: model
                        .custom_headers_mode
                        .unwrap_or_else(|| "merge".to_string()),
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
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let result_name = result.name.clone();
        let result_model_display = format!("{} / {}", result.model_name, result.name);

        let success = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service
                    .update_ai_model(&model_id, model_config)
                    .await
                {
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

        self.agent_selector
            .show(agent_items, Some(self.agent_type.clone()));
    }

    fn apply_agent_selection(&mut self, selected: &AgentItem) {
        if selected.id != self.agent_type {
            self.agent_type = selected.id.clone();
            self.status = Some(format!("Agent switched to: {}", selected.id));
            // Reload model name for new agent
            self.load_current_model_name();
        }
    }

    fn show_theme_selector(&mut self) {
        let themes = self.list_available_themes();
        if themes.is_empty() {
            self.status = Some("No themes available.".to_string());
            return;
        }

        self.push_current_popup_to_stack();
        self.begin_theme_preview();
        self.theme_selector
            .show(themes, Some(self.config.ui.theme_id.clone()));
        if let Some(selected) = self.theme_selector.selected_item().cloned() {
            self.preview_theme_selection(&selected);
        }
    }

    fn list_available_themes(&self) -> Vec<ThemeItem> {
        let mut themes: Vec<ThemeItem> = builtin_theme_ids()
            .into_iter()
            .map(|id| ThemeItem { id })
            .collect();

        themes.sort_by_cached_key(|theme| theme.id.to_ascii_lowercase());
        themes.dedup_by(|a, b| a.id == b.id);
        themes
    }

    fn current_base_theme(&self) -> (Theme, Appearance, EffectiveColorScheme) {
        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };

        (base, appearance, scheme)
    }

    fn resolve_theme_by_id(
        &self,
        base: Theme,
        appearance: Appearance,
        scheme: EffectiveColorScheme,
        id: &str,
    ) -> Theme {
        if scheme == EffectiveColorScheme::Monochrome {
            return Theme::monochrome();
        }

        let id = id.trim();
        if id.is_empty() {
            return base;
        }

        if let Some(json) = builtin_theme_json(id) {
            return base
                .apply_opencode_theme_json(json, appearance)
                .unwrap_or(base)
                .with_effective_scheme(scheme);
        }

        base
    }

    fn begin_theme_preview(&mut self) {
        if self.theme_preview_original.is_none() {
            self.theme_preview_original = Some(self.theme.clone());
        }
    }

    fn cancel_theme_preview(&mut self) {
        if let Some(original) = self.theme_preview_original.take() {
            self.theme = original;
        }
    }

    fn preview_theme_selection(&mut self, theme: &ThemeItem) {
        self.begin_theme_preview();
        let (base, appearance, scheme) = self.current_base_theme();
        self.theme = self.resolve_theme_by_id(base, appearance, scheme, &theme.id);
        self.status = Some(format!(
            "Preview theme: {} (Enter apply, Esc cancel)",
            theme.id
        ));
    }

    fn apply_theme_selection(&mut self, theme: &ThemeItem) {
        let (base, appearance, scheme) = self.current_base_theme();
        self.config.ui.theme_id = theme.id.clone();

        match self.config.save() {
            Ok(()) => {
                self.status = Some(format!("Theme set to: {}", theme.id));
            }
            Err(e) => {
                self.status = Some(format!("Failed to save config: {}", e));
            }
        }

        self.theme = self.resolve_theme_by_id(base, appearance, scheme, &theme.id);
        self.theme_preview_original = None;
    }

    fn show_skill_selector(&mut self) {
        self.push_current_popup_to_stack();
        self.skill_selector.show_menu();
    }

    fn show_available_skill_list(&mut self) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            tokio::runtime::Handle::current().block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_resolved_skills_for_workspace(Some(workspace.as_path()), Some(&agent_type))
                    .await
            })
        });

        if skills.is_empty() {
            self.status = Some(format!(
                "No enabled skills found for agent mode '{}'.",
                self.agent_type
            ));
            return;
        }

        let skill_items: Vec<SkillItem> =
            skills.into_iter().map(Self::skill_item_from_info).collect();

        if skill_items.is_empty() {
            self.status = Some("No skills found.".to_string());
            return;
        }

        self.skill_selector.show_list(skill_items);
    }

    fn show_skill_config_selector(&mut self) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            tokio::runtime::Handle::current().block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_mode_skill_infos_for_workspace(Some(workspace.as_path()), &agent_type)
                    .await
            })
        });

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(Self::skill_item_from_mode_info)
            .collect();

        if skill_items.is_empty() {
            self.status = Some("No skills found.".to_string());
            return;
        }

        self.skill_selector.show_config(skill_items);
    }

    fn handle_skill_selector_action(&mut self, action: SkillSelectorAction) {
        match action {
            SkillSelectorAction::ListSkills => self.show_available_skill_list(),
            SkillSelectorAction::ConfigureSkills => self.show_skill_config_selector(),
            SkillSelectorAction::Execute(selected) => {
                self.skill_selector.hide();
                self.set_input(&format!("Execute the {} skill.", selected.name));
            }
            SkillSelectorAction::Toggle(selected) => {
                self.set_skill_enabled(&selected, !selected.enabled);
                self.show_skill_config_selector();
            }
        }
    }

    fn set_skill_enabled(&mut self, selected: &SkillItem, enabled: bool) {
        let workspace = self.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let skill = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match skill.level.as_str() {
                    "user" => {
                        set_user_mode_skill_state(
                            &mode_id,
                            &skill.key,
                            enabled,
                            skill.default_enabled,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    }
                    "project" => {
                        let mut document = load_project_mode_skills_document_local(&workspace)
                            .await
                            .map_err(|error| error.to_string())?;
                        set_mode_skill_disabled_in_document(
                            &mut document,
                            &mode_id,
                            &skill.key,
                            !enabled,
                        )
                        .map_err(|error| error.to_string())?;
                        save_project_mode_skills_document_local(&workspace, &document)
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    other => {
                        return Err(format!("Unsupported skill level '{}'", other));
                    }
                }

                Ok(())
            })
        });

        self.status = Some(match result {
            Ok(()) => format!(
                "Skill '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            ),
            Err(error) => format!("Failed to update skill '{}': {}", selected.name, error),
        });
    }

    fn skill_item_from_info(info: SkillInfo) -> SkillItem {
        SkillItem {
            key: info.key,
            name: info.name,
            description: info.description,
            level: info.level.as_str().to_string(),
            enabled: true,
            selected_for_runtime: true,
            default_enabled: true,
            is_shadowed: info.is_shadowed,
        }
    }

    fn skill_item_from_mode_info(info: ModeSkillInfo) -> SkillItem {
        SkillItem {
            key: info.skill.key,
            name: info.skill.name,
            description: info.skill.description,
            level: info.skill.level.as_str().to_string(),
            enabled: info.effective_enabled,
            selected_for_runtime: info.selected_for_runtime,
            default_enabled: info.default_enabled,
            is_shadowed: info.skill.is_shadowed,
        }
    }

    fn show_subagent_selector(&mut self) {
        self.push_current_popup_to_stack();
        self.subagent_selector.show_menu();
    }

    fn show_available_subagent_list(&mut self) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            tokio::runtime::Handle::current().block_on(registry.get_subagents_for_query(
                &SubagentQueryContext {
                    parent_agent_type: Some(&agent_type),
                    workspace_root: Some(workspace.as_path()),
                    list_scope: SubagentListScope::TaskVisible,
                    include_disabled: false,
                },
            ))
        });

        if subagents.is_empty() {
            self.status = Some(format!(
                "No enabled subagents found for agent mode '{}'.",
                self.agent_type
            ));
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            self.status = Some("No subagents found.".to_string());
            return;
        }

        self.subagent_selector.show_list(subagent_items);
    }

    fn show_subagent_config_selector(&mut self) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            tokio::runtime::Handle::current().block_on(registry.get_subagents_for_query(
                &SubagentQueryContext {
                    parent_agent_type: Some(&agent_type),
                    workspace_root: Some(workspace.as_path()),
                    list_scope: SubagentListScope::RegistryManagement,
                    include_disabled: true,
                },
            ))
        });

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            self.status = Some("No subagents found.".to_string());
            return;
        }

        self.subagent_selector.show_config(subagent_items);
    }

    fn handle_subagent_selector_action(&mut self, action: SubagentSelectorAction) {
        match action {
            SubagentSelectorAction::ListSubagents => self.show_available_subagent_list(),
            SubagentSelectorAction::ConfigureSubagents => self.show_subagent_config_selector(),
            SubagentSelectorAction::Launch(selected) => {
                self.subagent_selector.hide();
                self.set_input(&format!(
                    "Launch subagent {} to finish task: ",
                    selected.name
                ));
            }
            SubagentSelectorAction::Toggle(selected) => {
                self.set_subagent_enabled(&selected, !selected.enabled);
                self.show_subagent_config_selector();
            }
        }
    }

    fn set_subagent_enabled(&mut self, selected: &SubagentItem, enabled: bool) {
        let registry = get_agent_registry();
        let workspace = self.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let subagent = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                registry
                    .update_subagent_override(
                        &mode_id,
                        &subagent.id,
                        enabled,
                        Some(workspace.as_path()),
                    )
                    .await
                    .map_err(|error| error.to_string())
            })
        });

        self.status = Some(match result {
            Ok(()) => format!(
                "Subagent '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            ),
            Err(error) => format!("Failed to update subagent '{}': {}", selected.name, error),
        });
    }

    fn subagent_item_from_info(info: AgentInfo) -> SubagentItem {
        let source = match info.subagent_source {
            Some(SubAgentSource::Builtin) => "builtin",
            Some(SubAgentSource::Project) => "project",
            Some(SubAgentSource::User) => "user",
            None => "builtin",
        }
        .to_string();

        SubagentItem {
            key: info.key,
            id: info.id,
            name: info.name,
            description: info.description,
            source,
            enabled: info.effective_enabled,
        }
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
        } else if self.theme_selector.is_visible() {
            self.theme_selector.hide();
            self.cancel_theme_preview();
        } else if self.provider_selector.is_visible() {
            self.provider_selector.hide();
        } else if self.model_config_form.is_visible() {
            self.model_config_form.hide();
        } else if self.login_form.is_visible() {
            self.login_form.hide();
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
                PopupType::ThemeSelector => self.theme_selector.reshow(),
                PopupType::ProviderSelector => self.provider_selector.reshow(),
                PopupType::ModelConfigForm => self.model_config_form.reshow(),
                PopupType::LoginForm => self.login_form.show(),
            }
        }
    }

    /// Close all popups and clear the navigation stack
    fn close_all_popups(&mut self) {
        self.info_popup = None;
        self.command_palette.hide();
        self.model_selector.hide();
        self.agent_selector.hide();
        self.session_selector.hide();
        self.skill_selector.hide();
        self.subagent_selector.hide();
        self.theme_selector.hide();
        self.cancel_theme_preview();
        self.provider_selector.hide();
        self.model_config_form.hide();
        self.login_form.hide();
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
        self.command_menu
            .update(&self.text_input.input, self.text_input.cursor);
    }
}

#[cfg(test)]
mod logo_contract_tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn fancy_logo_keeps_line_order_and_color_style_mapping() {
        let expected = [
            "  ██████╗ ██╗████████╗███████╗██╗   ██╗███╗   ██╗",
            "  ██╔══██╗██║╚══██╔══╝██╔════╝██║   ██║████╗  ██║",
            "  ██████╔╝██║   ██║   █████╗  ██║   ██║██╔██╗ ██║",
            "  ██╔══██╗██║   ██║   ██╔══╝  ██║   ██║██║╚██╗██║",
            "  ██████╔╝██║   ██║   ██║     ╚██████╔╝██║ ╚████║",
            "  ╚═════╝ ╚═╝   ╚═╝   ╚═╝      ╚═════╝ ╚═╝  ╚═══╝",
        ];
        let colors = [
            Color::Red,
            Color::Green,
            Color::Blue,
            Color::Yellow,
            Color::Magenta,
            Color::Cyan,
        ];
        let mut rendered = Vec::new();

        append_styled_logo_lines(&mut rendered, &FANCY_LOGO, &colors);

        assert_logo_contract(&rendered, &expected, &colors);
    }

    #[test]
    fn compact_logo_keeps_line_order_and_color_style_mapping() {
        let expected = [
            "  ____  _ _   _____            ",
            " | __ )(_) |_|  ___|   _ _ __  ",
            " |  _ \\| | __| |_ | | | | '_ \\ ",
            " | |_) | | |_|  _|| |_| | | | |",
            " |____/|_|\\__|_|   \\__,_|_| |_|",
        ];
        let colors = [
            Color::Red,
            Color::Green,
            Color::Blue,
            Color::Yellow,
            Color::Magenta,
        ];
        let mut rendered = Vec::new();

        append_styled_logo_lines(&mut rendered, &COMPACT_LOGO, &colors);

        assert_logo_contract(&rendered, &expected, &colors);
    }

    fn assert_logo_contract(lines: &[Line<'_>], expected: &[&str], colors: &[Color]) {
        assert_eq!(lines.len(), expected.len());
        for (index, ((line, expected_text), expected_color)) in
            lines.iter().zip(expected).zip(colors).enumerate()
        {
            assert_eq!(line.spans.len(), 1, "logo line {index} span count");
            let span = &line.spans[0];
            assert_eq!(span.content.as_ref(), *expected_text, "logo line {index}");
            assert_eq!(span.style.fg, Some(*expected_color), "logo line {index}");
            assert!(
                span.style.add_modifier.contains(Modifier::BOLD),
                "logo line {index} must stay bold"
            );
        }
    }
}
