use ratatui::{
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

pub(super) enum ResponsivePopup {
    Content(Rect),
    TooSmall(Rect),
}

pub(super) fn responsive_popup(
    area: Rect,
    max_width: u16,
    ideal_height: u16,
    min_content_width: u16,
    min_content_height: u16,
) -> ResponsivePopup {
    if area.width == 0 || area.height == 0 {
        return ResponsivePopup::TooSmall(area);
    }

    let inset_width = area.width.saturating_sub(4).min(max_width);
    let inset_height = ideal_height.min(area.height.saturating_sub(2));
    let (width, height) = if inset_width >= min_content_width && inset_height >= min_content_height
    {
        (inset_width, inset_height)
    } else if area.width >= min_content_width && area.height >= min_content_height {
        (area.width.min(max_width), ideal_height.min(area.height))
    } else {
        return ResponsivePopup::TooSmall(area);
    };

    ResponsivePopup::Content(Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    })
}

pub(super) fn render_too_small(frame: &mut Frame, area: Rect, theme: &Theme, title: &str) {
    frame.render_widget(Clear, area);
    if area.width < 4 || area.height < 3 {
        frame.render_widget(
            Paragraph::new(Line::from("Esc")).style(theme.style(StyleKind::Muted)),
            area,
        );
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.style(StyleKind::Primary))
        .style(Style::default().bg(theme.background))
        .title(format!(" {title} "));
    frame.render_widget(
        Paragraph::new("Terminal too small - resize or Esc")
            .block(block)
            .style(Style::default().bg(theme.background)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_full_area_before_falling_back_to_a_message() {
        let compact = responsive_popup(Rect::new(0, 0, 20, 6), 72, 10, 18, 6);
        assert!(
            matches!(compact, ResponsivePopup::Content(area) if area == Rect::new(0, 0, 20, 6))
        );

        let tiny = responsive_popup(Rect::new(0, 0, 10, 3), 72, 10, 18, 6);
        assert!(matches!(tiny, ResponsivePopup::TooSmall(_)));

        let zero = responsive_popup(Rect::new(0, 0, 0, 0), 72, 10, 18, 6);
        assert!(matches!(zero, ResponsivePopup::TooSmall(_)));
    }
}
