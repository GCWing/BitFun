use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};
use std::{collections::HashMap, time::SystemTime};

use crate::chat_state::SessionForkPoint;
use crate::ui::{
    conversation_selector::{
        ConversationPoint, ConversationSelectorAction, ConversationSelectorState,
    },
    theme::Theme,
};

const FULL_SESSION_ID: &str = "__full_session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForkTarget {
    FullSession,
    BeforeTurn {
        turn_id: String,
        message_id: String,
        prompt: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForkAction {
    None,
    Select(ForkTarget),
    Close,
}

pub(super) struct ForkSelectorState {
    selector: ConversationSelectorState,
    targets: HashMap<String, ForkTarget>,
}

impl ForkSelectorState {
    pub(super) fn new() -> Self {
        Self {
            selector: ConversationSelectorState::new("Fork Session", "Fork"),
            targets: HashMap::new(),
        }
    }

    pub(super) fn show(&mut self, points: Vec<SessionForkPoint>) {
        self.targets.clear();
        self.targets
            .insert(FULL_SESSION_ID.to_string(), ForkTarget::FullSession);
        let mut selector_points = vec![ConversationPoint::new(
            FULL_SESSION_ID,
            "Full session",
            "Fork from the latest turn",
        )];
        selector_points.extend(points.into_iter().map(|point| {
            let id = format!("message:{}", point.message_id);
            self.targets.insert(
                id.clone(),
                ForkTarget::BeforeTurn {
                    turn_id: point.turn_id,
                    message_id: point.message_id,
                    prompt: point.prompt.clone(),
                },
            );
            ConversationPoint::new(id, point.prompt, relative_time(point.timestamp))
        }));
        self.selector.show(selector_points);
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

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> ForkAction {
        match self.selector.handle_key_event(key) {
            ConversationSelectorAction::Select(id) => self
                .targets
                .get(&id)
                .cloned()
                .map(ForkAction::Select)
                .unwrap_or(ForkAction::None),
            ConversationSelectorAction::Close => ForkAction::Close,
            ConversationSelectorAction::Move(_) | ConversationSelectorAction::None => {
                ForkAction::None
            }
        }
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.selector.render(frame, area, theme);
    }
}

fn relative_time(timestamp: SystemTime) -> String {
    let seconds = timestamp.elapsed().unwrap_or_default().as_secs();
    match seconds {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m ago", seconds / 60),
        3_600..=86_399 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::{ForkAction, ForkSelectorState, ForkTarget};
    use crate::chat_state::SessionForkPoint;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::SystemTime;

    #[test]
    fn full_session_is_first_then_prompts_keep_their_supplied_order() {
        let mut selector = ForkSelectorState::new();
        selector.show(vec![SessionForkPoint {
            message_id: "message-newest".to_string(),
            turn_id: "turn-newest".to_string(),
            prompt: "Newest prompt".to_string(),
            timestamp: SystemTime::now(),
        }]);

        assert_eq!(
            selector.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ForkAction::Select(ForkTarget::FullSession)
        );
        selector.show(vec![SessionForkPoint {
            message_id: "message-newest".to_string(),
            turn_id: "turn-newest".to_string(),
            prompt: "Newest prompt".to_string(),
            timestamp: SystemTime::now(),
        }]);
        selector.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            selector.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ForkAction::Select(ForkTarget::BeforeTurn {
                turn_id: "turn-newest".to_string(),
                message_id: "message-newest".to_string(),
                prompt: "Newest prompt".to_string(),
            })
        );
    }
}
