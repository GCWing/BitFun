/// Custom TUI widgets

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub struct Spinner {
    frame: usize,
}

impl Spinner {
    const FRAMES: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    pub fn new(_style: Style) -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % Self::FRAMES.len();
    }

    pub fn current(&self) -> &str {
        Self::FRAMES[self.frame]
    }
}

/// Render a centered info popup overlay. Press any key to dismiss.
pub fn render_info_popup(frame: &mut Frame, area: Rect, message: &str, accent: Color) {
    let lines: Vec<Line> = message
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::White))))
        .collect();

    let line_count = lines.len() as u16;
    let max_line_width = message
        .lines()
        .map(|l| l.width() as u16)
        .max()
        .unwrap_or(20);

    // +2 for border, +2 for padding; +1 for hint line below popup
    let popup_width = (max_line_width + 4).min(area.width.saturating_sub(4)).max(30);
    let popup_height = (line_count + 2).min(area.height.saturating_sub(3));

    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height + 1)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(Color::Black))
        .title(" Help ");

    let text = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(text, popup_area);

    // Hint line below
    let hint_y = popup_area.y + popup_area.height;
    if hint_y < area.y + area.height {
        let hint_area = Rect {
            x: popup_area.x,
            y: hint_y,
            width: popup_area.width,
            height: 1,
        };
        let hint = Paragraph::new(Line::from(Span::styled(
            " Press any key to dismiss ",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(hint, hint_area);
    }
}
