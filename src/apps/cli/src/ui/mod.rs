/// TUI interface module
/// 
/// Build terminal user interface using ratatui

pub mod chat;
pub mod theme;
pub mod widgets;
pub mod startup;
pub mod tool_cards;
pub mod string_utils;
pub mod markdown;

use anyhow::Result;
use crossterm::{
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
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal
pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Render a loading/status message on the terminal (stays in alternate screen)
pub fn render_loading(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, message: &str) -> Result<()> {
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

        let text = vec![
            Line::from(Span::styled(
                msg,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[1]);
    })?;
    Ok(())
}
