use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::{
    prompt_stash::PromptStashEntry,
    ui::{
        conversation_selector::{
            ConversationPoint, ConversationSelectorAction, ConversationSelectorState,
        },
        theme::Theme,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptStashAction {
    None,
    Select(String),
    Close,
}

pub(super) struct PromptStashSelectorState {
    selector: ConversationSelectorState,
}

impl PromptStashSelectorState {
    pub(super) fn new() -> Self {
        Self {
            selector: ConversationSelectorState::new("Stash", "Restore"),
        }
    }

    pub(super) fn show(&mut self, entries: Vec<PromptStashEntry>) {
        self.selector.show(
            entries
                .into_iter()
                .map(|entry| {
                    ConversationPoint::new(
                        entry.id,
                        entry.draft.text,
                        relative_time(entry.timestamp_ms),
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

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> PromptStashAction {
        match self.selector.handle_key_event(key) {
            ConversationSelectorAction::Select(id) => PromptStashAction::Select(id),
            ConversationSelectorAction::Close => PromptStashAction::Close,
            ConversationSelectorAction::Move(_) | ConversationSelectorAction::None => {
                PromptStashAction::None
            }
        }
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.selector.render(frame, area, theme);
    }
}

fn relative_time(timestamp_ms: u64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let elapsed = now_ms.saturating_sub(timestamp_ms) / 1_000;
    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", elapsed / 60),
        3_600..=86_399 => format!("{}h ago", elapsed / 3_600),
        _ => format!("{}d ago", elapsed / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::relative_time;

    #[test]
    fn future_stash_timestamps_fail_closed_to_just_now() {
        assert_eq!(relative_time(u64::MAX), "just now");
    }
}
