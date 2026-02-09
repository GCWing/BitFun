/// Startup page module

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::time::Duration;

use crate::config::CliConfig;
use crate::session::Session;

/// Startup menu result
#[derive(Debug, Clone)]
pub enum StartupResult {
    /// New session (with workspace path)
    NewSession(String),
    /// Continue last session (session ID)
    ContinueSession(String),
    /// Browse and select history session (session ID)
    LoadSession(String),
    /// User cancelled exit
    Exit,
}

/// Startup page (main menu)
pub struct StartupPage {
    /// Menu item list
    menu_items: Vec<MenuItem>,
    /// Currently selected index
    selected: usize,
    /// List state
    list_state: ListState,
    /// Current page state
    page_state: PageState,
    /// Configuration
    config: CliConfig,
}

#[derive(Debug, Clone)]
struct MenuItem {
    name: String,
    description: String,
    action: MenuAction,
}

#[derive(Debug, Clone, PartialEq)]
enum MenuAction {
    NewSession,
    ContinueLastSession,
    BrowseHistory,
    Settings,
    Exit,
}

#[derive(Debug, Clone)]
enum PageState {
    /// Main menu
    MainMenu,
    /// Workspace selection
    WorkspaceSelect(WorkspaceSelectPage),
    /// Settings page
    Settings(SettingsPage),
    /// AI model management page
    AIModels(AIModelsPage),
    /// History session browsing
    History(HistoryPage),
    /// Finished (return result)
    Finished(StartupResult),
}

/// Workspace selection sub-page (input mode only)
#[derive(Debug, Clone)]
struct WorkspaceSelectPage {
    /// Custom input buffer
    custom_input: String,
    /// Custom input cursor position
    custom_cursor: usize,
}

/// Settings sub-page
#[derive(Debug, Clone)]
struct SettingsPage {
    settings: Vec<SettingItem>,
    selected: usize,
    editing: Option<usize>,
    edit_buffer: String,
}

/// AI model management sub-page
#[derive(Debug, Clone)]
struct AIModelsPage {
    models: Vec<AIModelItem>,
    selected: usize,
    default_model_id: String,
}

/// History session browsing sub-page
#[derive(Debug, Clone)]
struct HistoryPage {
    sessions: Vec<SessionItem>,
    selected: usize,
}

#[derive(Debug, Clone)]
struct SettingItem {
    key: String,
    name: String,
    value: String,
    description: String,
    editable: bool,
}

#[derive(Debug, Clone)]
struct AIModelItem {
    id: String,
    name: String,
    provider: String,
    model_name: String,
    enabled: bool,
    is_default: bool,
}

#[derive(Debug, Clone)]
struct SessionItem {
    id: String,
    title: String,
    workspace: String,
    agent: String,
    last_updated: String,
}

impl StartupPage {
    pub fn new() -> Self {
        let config = CliConfig::load().unwrap_or_default();
        
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        let menu_items = vec![
            MenuItem {
                name: "New Session".to_string(),
                description: "Select workspace and start a new chat session".to_string(),
                action: MenuAction::NewSession,
            },
            MenuItem {
                name: "Continue Last Session".to_string(),
                description: "Resume the most recent chat session".to_string(),
                action: MenuAction::ContinueLastSession,
            },
            MenuItem {
                name: "Browse History".to_string(),
                description: "View and select history sessions".to_string(),
                action: MenuAction::BrowseHistory,
            },
            MenuItem {
                name: "Settings".to_string(),
                description: "Configure AI models, API, and other options".to_string(),
                action: MenuAction::Settings,
            },
            MenuItem {
                name: "Exit".to_string(),
                description: "Exit the program".to_string(),
                action: MenuAction::Exit,
            },
        ];
        
        Self {
            menu_items,
            selected: 0,
            list_state,
            page_state: PageState::MainMenu,
            config,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<Option<String>> {
        terminal.clear()?;
        
        loop {
            terminal.draw(|f| self.render(f))?;

                    // Check if finished
            if let PageState::Finished(result) = &self.page_state {
                return match result {
                    StartupResult::NewSession(ws) => Ok(Some(ws.clone())),
                    StartupResult::Exit => Ok(None),
                    StartupResult::ContinueSession(_id) => {
                        // TODO: Implement session resume logic
                        Ok(Some(".".to_string()))
                    }
                    StartupResult::LoadSession(_id) => {
                        // TODO: Implement session loading logic
                        Ok(Some(".".to_string()))
                    }
                };
            }

            // Wait for event
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                    self.handle_key(key)?;
                }
                    Event::Resize(_, _) => {
                        terminal.clear()?;
                    }
                    _ => {}
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Clone page_state to avoid borrow conflicts
        match self.page_state.clone() {
            PageState::MainMenu => self.render_main_menu(frame, size),
            PageState::WorkspaceSelect(page) => self.render_workspace_select(frame, size, &page),
            PageState::Settings(page) => self.render_settings(frame, size, &page),
            PageState::AIModels(page) => self.render_ai_models(frame, size, &page),
            PageState::History(page) => self.render_history(frame, size, &page),
            PageState::Finished(_) => {}
        }
    }

    fn render_main_menu(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12), // Logo area
                Constraint::Min(10),     // Menu area
                Constraint::Length(3),   // Hints area
            ])
            .split(area);

        // Render Logo
        self.render_logo(frame, chunks[0]);

        // Render menu
        let items: Vec<ListItem> = self
            .menu_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == self.selected;
                let icon = if is_selected { "▶" } else { " " };
                
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(icon, Style::default().fg(Color::Green)),
                        Span::raw("  "),
                        Span::styled(&item.name, style),
                    ]),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(&item.description, Style::default().fg(Color::Gray)),
                    ]),
                    Line::from(""),
                ];

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" BitFun CLI - Main Menu ")
                    .title_alignment(Alignment::Center)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        frame.render_stateful_widget(list, chunks[1], &mut self.list_state);

        // Render hints
        let hints = Line::from(vec![
            Span::styled(" ↑/↓ ", Style::default().fg(Color::Green)),
            Span::raw("Select  "),
            Span::styled(" Enter ", Style::default().fg(Color::Green)),
            Span::raw("Confirm  "),
            Span::styled(" Esc/q ", Style::default().fg(Color::Red)),
            Span::raw("Exit"),
        ]);

        let paragraph = Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(paragraph, chunks[2]);
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

    fn render_workspace_select(&mut self, frame: &mut Frame, area: Rect, page: &WorkspaceSelectPage) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Title
                Constraint::Length(3),   // Input box
                Constraint::Min(5),      // Help
                Constraint::Length(5),   // Hints
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Enter workspace path")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        // Input box
        let input_display = if page.custom_input.is_empty() {
            "(Enter path, e.g.: /path/to/workspace or . for current directory)"
        } else {
            &page.custom_input
        };
        
        let input_style = if page.custom_input.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)
        };
        
        let input = Paragraph::new(input_display)
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).title(" Workspace Path ").border_style(Style::default().fg(Color::Yellow)));
        frame.render_widget(input, chunks[1]);

        // Help
        let help_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Tips:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  • You can enter relative or absolute path"),
            ]),
            Line::from(vec![
                Span::raw("  • Use "),
                Span::styled(".", Style::default().fg(Color::Green)),
                Span::raw(" for current directory"),
            ]),
            Line::from(vec![
                Span::raw("  • Use "),
                Span::styled("..", Style::default().fg(Color::Green)),
                Span::raw(" for parent directory"),
            ]),
            Line::from(vec![
                Span::raw("  • Path supports "),
                Span::styled("~", Style::default().fg(Color::Green)),
                Span::raw(" for home directory (e.g.: ~/projects)"),
            ]),
            Line::from(vec![
                Span::raw("  • Leave empty and press Enter for current directory"),
            ]),
        ];
        let help = Paragraph::new(help_lines)
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, chunks[2]);

        // Hints
        let hints_text = vec![
            Line::from(vec![
                Span::styled(" Enter ", Style::default().fg(Color::Green)),
                Span::raw("Confirm  "),
                Span::styled(" Esc ", Style::default().fg(Color::Red)),
                Span::raw("Back to menu  "),
                Span::styled(" Backspace ", Style::default().fg(Color::Yellow)),
                Span::raw("Delete"),
            ]),
            Line::from(vec![
                Span::styled(" Type characters... ", Style::default().fg(Color::DarkGray)),
            ]),
        ];
        
        let paragraph = Paragraph::new(hints_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(paragraph, chunks[3]);
    }

    fn render_settings(&mut self, frame: &mut Frame, area: Rect, page: &SettingsPage) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Title
                Constraint::Min(10),     // Settings list
                Constraint::Length(5),   // Hints
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Settings")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        // Settings list
        let items: Vec<ListItem> = page
            .settings
            .iter()
            .enumerate()
            .map(|(i, setting)| {
                let is_selected = i == page.selected;
                let is_editing = page.editing == Some(i);
                
                let icon = if is_selected { "▶" } else { " " };
                
                let style = if is_selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let value_style = if is_editing {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)
                } else if setting.editable {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let display_value = if is_editing {
                    &page.edit_buffer
                } else {
                    &setting.value
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(icon, Style::default().fg(Color::Green)),
                        Span::raw("  "),
                        Span::styled(&setting.name, style),
                        Span::raw(": "),
                        Span::styled(display_value, value_style),
                    ]),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(&setting.description, Style::default().fg(Color::Gray)),
                    ]),
                    Line::from(""),
                ];

                ListItem::new(content)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(page.selected));

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

        frame.render_stateful_widget(list, chunks[1], &mut list_state);

        // Hints
        let hints_text = if page.editing.is_some() {
            vec![
                Line::from(vec![
                    Span::styled(" Enter ", Style::default().fg(Color::Green)),
                    Span::raw("Save  "),
                    Span::styled(" Esc ", Style::default().fg(Color::Red)),
                    Span::raw("Cancel"),
                ]),
                Line::from(vec![
                    Span::styled(" Enter new value... ", Style::default().fg(Color::Yellow)),
                ]),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled(" ↑/↓ ", Style::default().fg(Color::Green)),
                    Span::raw("Select  "),
                    Span::styled(" Enter ", Style::default().fg(Color::Green)),
                    Span::raw("Edit  "),
                    Span::styled(" Esc ", Style::default().fg(Color::Red)),
                    Span::raw("Back"),
                ]),
                Line::from(vec![
                    Span::styled(" Changes will be auto-saved to config file ", Style::default().fg(Color::DarkGray)),
                ]),
            ]
        };

        let paragraph = Paragraph::new(hints_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(paragraph, chunks[2]);
    }

    fn render_history(&mut self, frame: &mut Frame, area: Rect, page: &HistoryPage) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Title
                Constraint::Min(10),     // Session list
                Constraint::Length(3),   // Hints
            ])
            .split(area);

        // Title
        let title_text = format!("History Sessions (total {})", page.sessions.len());
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        if page.sessions.is_empty() {
            // Empty state
            let empty_text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No history sessions yet",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Select \"New Session\" to start your first conversation",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let paragraph = Paragraph::new(empty_text)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
            frame.render_widget(paragraph, chunks[1]);
        } else {
            // Session list
            let items: Vec<ListItem> = page
                .sessions
                .iter()
                .enumerate()
                .map(|(i, session)| {
                    let is_selected = i == page.selected;
                    let icon = if is_selected { "▶" } else { " " };
                    
                    let style = if is_selected {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let content = vec![
                        Line::from(vec![
                            Span::styled(icon, Style::default().fg(Color::Green)),
                            Span::raw("  "),
                            Span::styled(&session.title, style),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled("Agent: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(&session.agent, Style::default().fg(Color::Blue)),
                            Span::raw("  |  "),
                            Span::styled("Workspace: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(&session.workspace, Style::default().fg(Color::Green)),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(&session.last_updated, Style::default().fg(Color::Gray)),
                        ]),
                        Line::from(""),
                    ];

                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(page.selected));

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

            frame.render_stateful_widget(list, chunks[1], &mut list_state);
        }

        // Hints
        let hints = Line::from(vec![
            Span::styled(" ↑/↓ ", Style::default().fg(Color::Green)),
            Span::raw("Select  "),
            Span::styled(" Enter ", Style::default().fg(Color::Green)),
            Span::raw("Load  "),
            Span::styled(" Esc ", Style::default().fg(Color::Red)),
            Span::raw("Back"),
        ]);

        let paragraph = Paragraph::new(hints)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(paragraph, chunks[2]);
    }

    fn render_ai_models(&mut self, frame: &mut Frame, area: Rect, page: &AIModelsPage) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Title
                Constraint::Min(10),     // Model list
                Constraint::Length(5),   // Hints
            ])
            .split(area);

        // Title
        let title_text = format!("AI Model Configuration (total {})", page.models.len());
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        if page.models.is_empty() {
            // Empty state
            let empty_text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No models configured yet",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press N to create your first model configuration",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let paragraph = Paragraph::new(empty_text)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
            frame.render_widget(paragraph, chunks[1]);
        } else {
            // Model list
            let items: Vec<ListItem> = page
                .models
                .iter()
                .enumerate()
                .map(|(i, model)| {
                    let is_selected = i == page.selected;
                    let icon = if is_selected { "▶" } else { " " };
                    
                    let style = if is_selected {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    // Status marker
                    let status_icon = if model.is_default {
                        "*"
                    } else if model.enabled {
                        "+"
                    } else {
                        "-"
                    };

                    let content = vec![
                        Line::from(vec![
                            Span::styled(icon, Style::default().fg(Color::Green)),
                            Span::raw("  "),
                            Span::styled(status_icon, Style::default().fg(if model.is_default { Color::Yellow } else { Color::Green })),
                            Span::raw(" "),
                            Span::styled(&model.name, style),
                        ]),
                        Line::from(vec![
                            Span::raw("      "),
                            Span::styled("Provider: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(&model.provider, Style::default().fg(Color::Blue)),
                            Span::raw("  |  "),
                            Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(&model.model_name, Style::default().fg(Color::Magenta)),
                        ]),
                        Line::from(""),
                    ];

                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(page.selected));

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

            frame.render_stateful_widget(list, chunks[1], &mut list_state);
        }

        // Hints
        let hints_text = vec![
            Line::from(vec![
                Span::styled(" ↑/↓ ", Style::default().fg(Color::Green)),
                Span::raw("Select  "),
                Span::styled(" Enter ", Style::default().fg(Color::Green)),
                Span::raw("Set default  "),
                Span::styled(" E ", Style::default().fg(Color::Yellow)),
                Span::raw("Edit  "),
                Span::styled(" N ", Style::default().fg(Color::Cyan)),
                Span::raw("New"),
            ]),
            Line::from(vec![
                Span::styled(" Esc ", Style::default().fg(Color::Red)),
                Span::raw("Back  "),
                Span::styled(" * ", Style::default().fg(Color::Yellow)),
                Span::raw("Default model  "),
                Span::styled(" + ", Style::default().fg(Color::Green)),
                Span::raw("Enabled  "),
                Span::styled(" - ", Style::default().fg(Color::DarkGray)),
                Span::raw("Disabled"),
            ]),
        ];

        let paragraph = Paragraph::new(hints_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(paragraph, chunks[2]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        let page_state = std::mem::replace(&mut self.page_state, PageState::Finished(StartupResult::Exit));
        
        let result = match page_state {
            PageState::MainMenu => {
                self.page_state = PageState::MainMenu;
                self.handle_main_menu_key(key)
            }
            PageState::WorkspaceSelect(mut page) => {
                let old_state = std::mem::replace(&mut self.page_state, PageState::Finished(StartupResult::Exit));
                let result = self.handle_workspace_key(key, &mut page);
                if matches!(self.page_state, PageState::Finished(StartupResult::Exit)) {
                    if !matches!(old_state, PageState::Finished(_)) {
                        self.page_state = old_state;
                    } else {
                        self.page_state = PageState::WorkspaceSelect(page);
                    }
                }
                result
            }
            PageState::Settings(mut page) => {
                let old_state = std::mem::replace(&mut self.page_state, PageState::Finished(StartupResult::Exit));
                let result = self.handle_settings_key(key, &mut page);
                if matches!(self.page_state, PageState::Finished(StartupResult::Exit)) {
                    if !matches!(old_state, PageState::Finished(_)) {
                        self.page_state = old_state;
                    } else {
                        self.page_state = PageState::Settings(page);
                    }
                }
                result
            }
            PageState::AIModels(mut page) => {
                let old_state = std::mem::replace(&mut self.page_state, PageState::Finished(StartupResult::Exit));
                let result = self.handle_ai_models_key(key, &mut page);
                if matches!(self.page_state, PageState::Finished(StartupResult::Exit)) {
                    if !matches!(old_state, PageState::Finished(_)) {
                        self.page_state = old_state;
                    } else {
                        self.page_state = PageState::AIModels(page);
                    }
                }
                result
            }
            PageState::History(mut page) => {
                let old_state = std::mem::replace(&mut self.page_state, PageState::Finished(StartupResult::Exit));
                let result = self.handle_history_key(key, &mut page);
                if matches!(self.page_state, PageState::Finished(StartupResult::Exit)) {
                    if !matches!(old_state, PageState::Finished(_)) {
                        self.page_state = old_state;
                    } else {
                        self.page_state = PageState::History(page);
                    }
                }
                result
            }
            PageState::Finished(result) => {
                self.page_state = PageState::Finished(result);
                Ok(())
            }
        };
        
        result
    }

    fn handle_main_menu_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected < self.menu_items.len() - 1 {
                    self.selected += 1;
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Enter => {
                let action = &self.menu_items[self.selected].action;
                match action {
                    MenuAction::NewSession => {
                        // Enter workspace input page
                        self.page_state = PageState::WorkspaceSelect(WorkspaceSelectPage {
                            custom_input: String::new(),
                            custom_cursor: 0,
                        });
                    }
                    MenuAction::ContinueLastSession => {
                        // Load last session
                        if let Ok(Some(session)) = Session::get_last() {
                            self.page_state = PageState::Finished(StartupResult::ContinueSession(session.id));
                        } else {
                            // No history session, enter new session
                            self.page_state = PageState::WorkspaceSelect(WorkspaceSelectPage {
                                custom_input: String::new(),
                                custom_cursor: 0,
                            });
                        }
                    }
                    MenuAction::BrowseHistory => {
                        // Enter history session browsing
                        let sessions = Self::load_sessions();
                        self.page_state = PageState::History(HistoryPage {
                            sessions,
                            selected: 0,
                        });
                    }
                    MenuAction::Settings => {
                        // Enter settings page
                        let settings = Self::load_settings(&self.config);
                        self.page_state = PageState::Settings(SettingsPage {
                            settings,
                            selected: 0,
                            editing: None,
                            edit_buffer: String::new(),
                        });
                    }
                    MenuAction::Exit => {
                        self.page_state = PageState::Finished(StartupResult::Exit);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.page_state = PageState::Finished(StartupResult::Exit);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_workspace_key(&mut self, key: KeyEvent, page: &mut WorkspaceSelectPage) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                // If input is empty, use current directory
                let path = if page.custom_input.is_empty() {
                    ".".to_string()
                } else {
                    // Handle path expansion (~ and relative paths)
                    self.expand_path(&page.custom_input)
                };
                self.page_state = PageState::Finished(StartupResult::NewSession(path));
            }
            KeyCode::Esc => {
                // Return to main menu
                self.page_state = PageState::MainMenu;
                self.selected = 0;
                self.list_state.select(Some(0));
            }
            KeyCode::Backspace => {
                if page.custom_cursor > 0 && page.custom_cursor <= page.custom_input.len() {
                    page.custom_input.remove(page.custom_cursor - 1);
                    page.custom_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if page.custom_cursor < page.custom_input.len() {
                    page.custom_input.remove(page.custom_cursor);
                }
            }
            KeyCode::Left => {
                if page.custom_cursor > 0 {
                    page.custom_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if page.custom_cursor < page.custom_input.len() {
                    page.custom_cursor += 1;
                }
            }
            KeyCode::Home => {
                page.custom_cursor = 0;
            }
            KeyCode::End => {
                page.custom_cursor = page.custom_input.len();
            }
            KeyCode::Char(c) => {
                page.custom_input.insert(page.custom_cursor, c);
                page.custom_cursor += 1;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn expand_path(&self, path: &str) -> String {
        let path = path.trim();
        
        // Handle paths starting with ~
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let rest = &path[1..];
                return home.join(rest.trim_start_matches('/')).to_string_lossy().to_string();
            }
        }
        
        // Handle relative and absolute paths
        if let Ok(absolute) = std::fs::canonicalize(path) {
            absolute.to_string_lossy().to_string()
        } else {
            // If path doesn't exist, still return original path (let subsequent code handle it)
            path.to_string()
        }
    }

    fn handle_settings_key(&mut self, key: KeyEvent, page: &mut SettingsPage) -> Result<()> {
        if let Some(editing_idx) = page.editing {
            match key.code {
                KeyCode::Enter => {
                    let setting = &mut page.settings[editing_idx];
                    setting.value = page.edit_buffer.clone();
                    self.update_config_value(&setting.key, &setting.value)?;
                    page.editing = None;
                    page.edit_buffer.clear();
                }
                KeyCode::Esc => {
                    page.editing = None;
                    page.edit_buffer.clear();
                }
                KeyCode::Char(c) => {
                    page.edit_buffer.push(c);
                }
                KeyCode::Backspace => {
                    page.edit_buffer.pop();
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if page.selected > 0 {
                        page.selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if page.selected < page.settings.len() - 1 {
                        page.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if page.settings[page.selected].key == "ai_models" {
                        let models = Self::load_ai_models_sync();
                        let default_model_id = models.iter()
                            .find(|m| m.is_default)
                            .map(|m| m.id.clone())
                            .unwrap_or_default();
                        
                        self.page_state = PageState::AIModels(AIModelsPage {
                            models,
                            selected: 0,
                            default_model_id,
                        });
                    } else if page.settings[page.selected].editable {
                        page.editing = Some(page.selected);
                        page.edit_buffer = page.settings[page.selected].value.clone();
                    }
                }
                KeyCode::Esc => {
                    self.page_state = PageState::MainMenu;
                    self.selected = 0;
                    self.list_state.select(Some(0));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_history_key(&mut self, key: KeyEvent, page: &mut HistoryPage) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if page.selected > 0 {
                    page.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !page.sessions.is_empty() && page.selected < page.sessions.len() - 1 {
                    page.selected += 1;
                }
            }
            KeyCode::Enter => {
                if !page.sessions.is_empty() {
                    let session_id = page.sessions[page.selected].id.clone();
                    self.page_state = PageState::Finished(StartupResult::LoadSession(session_id));
                }
            }
            KeyCode::Esc => {
                // Return to main menu
                self.page_state = PageState::MainMenu;
                self.selected = 0;
                self.list_state.select(Some(0));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_ai_models_key(&mut self, key: KeyEvent, page: &mut AIModelsPage) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if page.selected > 0 {
                    page.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !page.models.is_empty() && page.selected < page.models.len() - 1 {
                    page.selected += 1;
                }
            }
            KeyCode::Enter => {
                if !page.models.is_empty() {
                    let selected_model_id = page.models[page.selected].id.clone();
                    let result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            use bitfun_core::service::config::GlobalConfigManager;
                            use bitfun_core::service::config::types::GlobalConfig;
                            
                            match GlobalConfigManager::get_service().await {
                                Ok(config_service) => {
                                    let mut global_config = config_service.get_config::<GlobalConfig>(None).await?;
                                    global_config.ai.default_models.primary = Some(selected_model_id.clone());
                                    config_service.set_config("ai.default_models.primary", &global_config.ai.default_models.primary).await
                                }
                                Err(e) => Err(e),
                            }
                        })
                    });
                    
                    if result.is_ok() {
                        page.models = Self::load_ai_models_sync();
                        page.default_model_id = selected_model_id;
                    }
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {}
            KeyCode::Char('n') | KeyCode::Char('N') => {}
            KeyCode::Esc => {
                let settings = Self::load_settings(&self.config);
                self.page_state = PageState::Settings(SettingsPage {
                    settings,
                    selected: 0,
                    editing: None,
                    edit_buffer: String::new(),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn load_settings(config: &CliConfig) -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "ai_models".to_string(),
                name: "AI Model Configuration".to_string(),
                value: "Manage AI models".to_string(),
                description: "View and manage all AI model configurations (press Enter to enter)".to_string(),
                editable: false, // Not directly editable, enters sub-page
            },
            SettingItem {
                key: "behavior.default_agent".to_string(),
                name: "Default Agent".to_string(),
                value: config.behavior.default_agent.clone(),
                description: "Default Agent type to use".to_string(),
                editable: true,
            },
            SettingItem {
                key: "ui.theme".to_string(),
                name: "Theme".to_string(),
                value: config.ui.theme.clone(),
                description: "Interface theme (dark, light)".to_string(),
                editable: true,
            },
            SettingItem {
                key: "ui.show_tips".to_string(),
                name: "Show Tips".to_string(),
                value: config.ui.show_tips.to_string(),
                description: "Whether to show operation tips".to_string(),
                editable: true,
            },
            SettingItem {
                key: "behavior.auto_save".to_string(),
                name: "Auto Save".to_string(),
                value: config.behavior.auto_save.to_string(),
                description: "Whether to auto-save sessions".to_string(),
                editable: true,
            },
        ]
    }

    async fn load_ai_models() -> Vec<AIModelItem> {
        use bitfun_core::service::config::GlobalConfigManager;
        use bitfun_core::service::config::types::GlobalConfig;
        
        match GlobalConfigManager::get_service().await {
            Ok(config_service) => {
                match config_service.get_config::<GlobalConfig>(None).await {
                    Ok(global_config) => {
                        let default_model_id = global_config.ai.default_models.primary
                            .unwrap_or_default();
                        
                        global_config.ai.models
                            .iter()
                            .map(|m| AIModelItem {
                                id: m.id.clone(),
                                name: m.name.clone(),
                                provider: m.provider.clone(),
                                model_name: m.model_name.clone(),
                                enabled: m.enabled,
                                is_default: m.id == default_model_id,
                            })
                            .collect()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get GlobalConfig: {}", e);
                        vec![]
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to get config service: {}", e);
                vec![]
            }
        }
    }
    
    fn load_ai_models_sync() -> Vec<AIModelItem> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(Self::load_ai_models())
        })
    }

    fn load_sessions() -> Vec<SessionItem> {
        Session::list_all()
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|s| SessionItem {
                id: s.id,
                title: s.title,
                workspace: s.workspace.unwrap_or_else(|| "None".to_string()),
                agent: s.agent,
                last_updated: s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect()
    }

    fn update_config_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "behavior.default_agent" => self.config.behavior.default_agent = value.to_string(),
            "ui.theme" => self.config.ui.theme = value.to_string(),
            "ui.show_tips" => {
                if let Ok(v) = value.parse::<bool>() {
                    self.config.ui.show_tips = v;
                }
            }
            "behavior.auto_save" => {
                if let Ok(v) = value.parse::<bool>() {
                    self.config.behavior.auto_save = v;
                }
            }
            "ai_models" => {}
            _ => {}
        }
        
        self.config.save()?;
        Ok(())
    }
}
