/// TUI interface module
///
/// Build terminal user interface using ratatui
pub(crate) mod agent_selector;
pub(crate) mod chat;
mod command_menu;
pub(crate) mod command_palette;
mod diff_render;
pub(crate) mod login_form;
mod markdown;
pub(crate) mod mcp_add_dialog;
pub(crate) mod mcp_selector;
pub(crate) mod model_config_form;
pub(crate) mod model_selector;
pub(crate) mod permission;
pub(crate) mod provider_selector;
pub(crate) mod question;
pub(crate) mod session_selector;
pub(crate) mod skill_selector;
pub(crate) mod startup;
pub(crate) mod string_utils;
pub(crate) mod subagent_selector;
mod syntax_highlight;
mod text_input;
pub(crate) mod theme;
pub(crate) mod theme_selector;
mod tool_cards;
mod widgets;

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use std::io;

/// Initialize terminal
pub(crate) fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal
pub(crate) fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Render a loading/status message on the terminal (stays in alternate screen)
pub(crate) fn render_loading(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    message: &str,
) -> Result<()> {
    let msg = message.to_string();
    terminal.draw(|frame| {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(3),
                Constraint::Percentage(45),
            ])
            .split(area);

        let text = vec![Line::from(Span::styled(
            msg,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))];

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[1]);
    })?;
    Ok(())
}
