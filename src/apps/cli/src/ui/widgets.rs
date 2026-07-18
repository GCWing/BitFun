/// Custom TUI widgets
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

const INFO_POPUP_DISMISS_HINT: &str = " Up/Down or PgUp/PgDn to scroll - Esc to dismiss ";
const INFO_POPUP_NARROW_DISMISS_HINT: &str = " Esc close ";

fn info_popup_dismiss_hint(width: u16) -> &'static str {
    if INFO_POPUP_DISMISS_HINT.width() <= usize::from(width) {
        INFO_POPUP_DISMISS_HINT
    } else {
        INFO_POPUP_NARROW_DISMISS_HINT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InfoPopupLayout {
    area: Rect,
    scroll: u16,
    max_scroll: u16,
}

fn wrapped_line_count(message: &str, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    message
        .lines()
        .map(|line| {
            let line_width = line.width().max(1);
            line_width.div_ceil(width).min(usize::from(u16::MAX)) as u16
        })
        .fold(0_u16, u16::saturating_add)
        .max(1)
}

fn info_popup_layout(area: Rect, message: &str, requested_scroll: u16) -> InfoPopupLayout {
    if area.width == 0 || area.height == 0 {
        return InfoPopupLayout {
            area,
            scroll: 0,
            max_scroll: 0,
        };
    }

    let horizontal_margin: u16 = if area.width >= 8 { 2 } else { 0 };
    let max_width = area
        .width
        .saturating_sub(horizontal_margin.saturating_mul(2))
        .max(1);
    let max_line_width = message
        .lines()
        .map(|line| line.width().min(usize::from(u16::MAX)) as u16)
        .max()
        .unwrap_or(20);
    let minimum_width = 30.min(max_width);
    let popup_width = max_line_width
        .saturating_add(4)
        .clamp(minimum_width, max_width);

    let hint_height = u16::from(area.height >= 4);
    let max_height = area.height.saturating_sub(hint_height).max(1);
    let unwrapped_height = message.lines().count().min(usize::from(u16::MAX)) as u16;
    let minimum_height = 3.min(max_height);
    let popup_height = unwrapped_height
        .saturating_add(2)
        .clamp(minimum_height, max_height);
    let content_width = popup_width.saturating_sub(2).max(1);
    let visible_lines = popup_height.saturating_sub(2).max(1);
    let max_scroll = wrapped_line_count(message, content_width).saturating_sub(visible_lines);
    let scroll = requested_scroll.min(max_scroll);

    let popup_x = area.x + area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.y
        + area
            .height
            .saturating_sub(popup_height.saturating_add(hint_height))
            / 2;
    InfoPopupLayout {
        area: Rect::new(popup_x, popup_y, popup_width, popup_height),
        scroll,
        max_scroll,
    }
}

pub(super) struct Spinner {
    frame: usize,
}

impl Spinner {
    const FRAMES: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    pub(super) fn new(_style: Style) -> Self {
        Self { frame: 0 }
    }

    pub(super) fn tick(&mut self) {
        self.frame = (self.frame + 1) % Self::FRAMES.len();
    }

    pub(super) fn current(&self) -> &str {
        Self::FRAMES[self.frame]
    }
}

/// Render a centered info popup overlay. Esc always dismisses it.
pub(super) fn render_info_popup(frame: &mut Frame, area: Rect, message: &str, accent: Color) {
    let _ = render_info_popup_scrolled(frame, area, message, accent, 0);
}

pub(super) fn render_info_popup_scrolled(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    accent: Color,
    requested_scroll: u16,
) -> (u16, u16) {
    let lines: Vec<Line> = message
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    let layout = info_popup_layout(area, message, requested_scroll);
    let popup_area = layout.area;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(Color::Black))
        .title(" Help ");

    let text = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((layout.scroll, 0));

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
            info_popup_dismiss_hint(hint_area.width),
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(hint, hint_area);
    }
    (layout.scroll, layout.max_scroll)
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use unicode_width::UnicodeWidthStr;

    use super::{info_popup_dismiss_hint, info_popup_layout, INFO_POPUP_DISMISS_HINT};

    #[test]
    fn info_popup_names_the_modal_safe_dismiss_key() {
        assert!(INFO_POPUP_DISMISS_HINT.contains("Esc"));
        assert!(!INFO_POPUP_DISMISS_HINT.contains("any key"));
    }

    #[test]
    fn info_popup_layout_stays_inside_a_narrow_viewport() {
        let viewport = Rect::new(7, 11, 18, 8);
        let layout = info_popup_layout(viewport, "a very long line that must wrap", u16::MAX);

        assert!(layout.area.x >= viewport.x);
        assert!(layout.area.y >= viewport.y);
        assert!(layout.area.right() <= viewport.right());
        assert!(layout.area.bottom() <= viewport.bottom());
        assert!(layout.max_scroll > 0);
        assert_eq!(layout.scroll, layout.max_scroll);
        let hint = info_popup_dismiss_hint(layout.area.width);
        assert!(hint.contains("Esc"));
        assert!(hint.width() <= usize::from(layout.area.width));
    }
}
