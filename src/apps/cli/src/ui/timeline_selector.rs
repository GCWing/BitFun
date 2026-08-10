use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::{
    chat_state::SessionTimelinePoint,
    ui::{
        conversation_selector::{
            ConversationPoint, ConversationSelectorAction, ConversationSelectorState,
        },
        message_time::format_message_timestamp,
        theme::Theme,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TimelineAction {
    None,
    Move(String),
    Select(String),
    Close,
}

pub(super) struct TimelineSelectorState {
    selector: ConversationSelectorState,
}

impl TimelineSelectorState {
    pub(super) fn new() -> Self {
        Self {
            selector: ConversationSelectorState::new("Timeline", "Jump"),
        }
    }

    pub(super) fn show(&mut self, points: Vec<SessionTimelinePoint>) {
        self.selector.show(
            points
                .into_iter()
                .map(|point| {
                    ConversationPoint::new(
                        point.message_id,
                        point.prompt,
                        format_message_timestamp(point.timestamp).unwrap_or_default(),
                    )
                })
                .collect(),
        );
    }

    pub(super) fn hide(&mut self) {
        self.selector.hide();
    }

    pub(super) fn reshow(&mut self) {
        self.selector.reshow();
    }

    pub(super) fn is_visible(&self) -> bool {
        self.selector.is_visible()
    }

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> TimelineAction {
        match self.selector.handle_key_event(key) {
            ConversationSelectorAction::Move(id) => TimelineAction::Move(id),
            ConversationSelectorAction::Select(id) => TimelineAction::Select(id),
            ConversationSelectorAction::Close => TimelineAction::Close,
            ConversationSelectorAction::None => TimelineAction::None,
        }
    }

    pub(super) fn selected_message_id(&self) -> Option<String> {
        self.selector
            .is_visible()
            .then(|| self.selector.selected_id())
            .flatten()
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.selector.render(frame, area, theme);
    }
}

#[cfg(test)]
mod tests {
    use super::TimelineSelectorState;
    use crate::chat_state::SessionTimelinePoint;
    use std::time::SystemTime;

    #[test]
    fn hidden_timeline_does_not_keep_a_transcript_scroll_anchor() {
        let mut selector = TimelineSelectorState::new();
        selector.show(vec![SessionTimelinePoint {
            message_id: "message-1".to_string(),
            prompt: "Prompt".to_string(),
            timestamp: SystemTime::now(),
        }]);
        assert_eq!(selector.selected_message_id().as_deref(), Some("message-1"));

        selector.hide();

        assert_eq!(selector.selected_message_id(), None);
    }
}
