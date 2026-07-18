use std::time::{Duration, Instant};

#[derive(Debug)]
pub(super) struct ResizeRedrawState {
    debounce: Duration,
    pending_since: Option<Instant>,
}

impl ResizeRedrawState {
    pub(super) const fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            pending_since: None,
        }
    }

    pub(super) fn observe(&mut self, observed_at: Instant) {
        self.pending_since = Some(observed_at);
    }

    pub(super) fn take_ready(&mut self, now: Instant) -> bool {
        let Some(observed_at) = self.pending_since else {
            return false;
        };
        if now.saturating_duration_since(observed_at) < self.debounce {
            return false;
        }
        self.pending_since = None;
        true
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.pending_since.is_some()
    }

    pub(super) const fn can_render(&self) -> bool {
        !self.is_pending()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::ResizeRedrawState;
    use crate::chat_state::ChatState;
    use crate::ui::chat::ChatView;
    use crate::ui::theme::Theme;

    const DEBOUNCE: Duration = Duration::from_millis(75);

    #[test]
    fn resize_redraw_waits_for_the_quiet_period() {
        let started_at = Instant::now();
        let mut state = ResizeRedrawState::new(DEBOUNCE);

        state.observe(started_at);

        assert!(!state.can_render());
        assert!(!state.take_ready(started_at + Duration::from_millis(74)));
        assert!(state.take_ready(started_at + DEBOUNCE));
        assert!(state.can_render());
    }

    #[test]
    fn resize_redraw_restarts_the_quiet_period_for_a_burst() {
        let started_at = Instant::now();
        let mut state = ResizeRedrawState::new(DEBOUNCE);

        state.observe(started_at);
        state.observe(started_at + Duration::from_millis(40));

        assert!(!state.take_ready(started_at + DEBOUNCE));
        assert!(state.take_ready(started_at + Duration::from_millis(115)));
    }

    #[test]
    fn resize_redraw_is_consumed_once() {
        let started_at = Instant::now();
        let mut state = ResizeRedrawState::new(DEBOUNCE);

        state.observe(started_at);

        assert!(state.take_ready(started_at + DEBOUNCE));
        assert!(!state.take_ready(started_at + DEBOUNCE));
        assert!(!state.is_pending());
    }

    #[test]
    fn active_streaming_content_reflows_at_the_resized_width() {
        let mut chat_state = ChatState::new(
            "session-resize".to_string(),
            "Resize test".to_string(),
            "general".to_string(),
            None,
        );
        chat_state.handle_turn_started("turn-resize", "show the current status");
        chat_state.handle_text_chunk(
            "STARTMARK alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma ENDMARK",
        );

        let mut chat_view = ChatView::new(Theme::dark(), Vec::new());
        let mut terminal = Terminal::new(TestBackend::new(48, 30)).expect("test terminal");
        terminal
            .draw(|frame| chat_view.render(frame, &chat_state))
            .expect("initial render");
        let wide_buffer = terminal.backend().buffer();
        let wide_start = marker_row(wide_buffer, "STARTMARK");
        let wide_end = marker_row(wide_buffer, "ENDMARK");

        terminal.backend_mut().resize(24, 30);
        chat_view.invalidate_lines_cache();
        terminal
            .draw(|frame| chat_view.render(frame, &chat_state))
            .expect("resized render");

        let buffer = terminal.backend().buffer();
        let narrow_start = marker_row(buffer, "STARTMARK");
        let narrow_end = marker_row(buffer, "ENDMARK");

        assert_eq!(buffer.area, Rect::new(0, 0, 24, 30));
        assert!(
            narrow_end - narrow_start > wide_end - wide_start,
            "narrow rendering should wrap the same streaming content across more rows"
        );
    }

    fn marker_row(buffer: &ratatui::buffer::Buffer, marker: &str) -> usize {
        (0..buffer.area.height)
            .find(|&y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .contains(marker)
            })
            .map(usize::from)
            .unwrap_or_else(|| panic!("missing marker {marker:?} in rendered buffer"))
    }
}
