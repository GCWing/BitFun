use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::time::SystemTime;

use crate::chat_state::SessionForkPoint;
use crate::ui::theme::{StyleKind, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForkTarget {
    FullSession,
    BeforeTurn { turn_id: String, prompt: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForkAction {
    None,
    Select(ForkTarget),
    Close,
}

pub(super) struct ForkSelectorState {
    points: Vec<SessionForkPoint>,
    list_state: ListState,
    visible: bool,
}

impl ForkSelectorState {
    pub(super) fn new() -> Self {
        Self {
            points: Vec::new(),
            list_state: ListState::default(),
            visible: false,
        }
    }

    pub(super) fn show(&mut self, points: Vec<SessionForkPoint>) {
        self.points = points;
        self.list_state.select(Some(0));
        self.visible = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
    }

    pub(super) fn reshow(&mut self) {
        self.visible = true;
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> ForkAction {
        if !self.visible {
            return ForkAction::None;
        }
        match key.code {
            KeyCode::Up => {
                self.move_selection(-1);
                ForkAction::None
            }
            KeyCode::Down => {
                self.move_selection(1);
                ForkAction::None
            }
            KeyCode::Enter => {
                let target = self.selected_target();
                if target.is_some() {
                    self.hide();
                }
                target.map(ForkAction::Select).unwrap_or(ForkAction::None)
            }
            KeyCode::Esc => {
                self.hide();
                ForkAction::Close
            }
            _ => ForkAction::None,
        }
    }

    fn selected_target(&self) -> Option<ForkTarget> {
        match self.list_state.selected()? {
            0 => Some(ForkTarget::FullSession),
            index => self
                .points
                .get(index - 1)
                .map(|point| ForkTarget::BeforeTurn {
                    turn_id: point.turn_id.clone(),
                    prompt: point.prompt.clone(),
                }),
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.points.len() + 1;
        let selected = self.list_state.selected().unwrap_or(0) as isize;
        let next = (selected + delta).rem_euclid(len as isize) as usize;
        self.list_state.select(Some(next));
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }
        let width = area.width.saturating_sub(4).min(78);
        let height = (self.points.len() as u16 + 5).min(area.height.saturating_sub(2));
        if width < 24 || height < 5 {
            return;
        }
        let popup = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        };
        let preview_width = width.saturating_sub(16) as usize;
        let mut items = Vec::with_capacity(self.points.len() + 1);
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                "Full session",
                theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Fork from the latest turn", theme.style(StyleKind::Muted)),
        ])));
        items.extend(self.points.iter().map(|point| {
            let preview = one_line_preview(&point.prompt, preview_width);
            ListItem::new(Line::from(vec![
                Span::styled(preview, theme.style(StyleKind::Primary)),
                Span::styled(
                    format!("  {}", relative_time(point.timestamp)),
                    theme.style(StyleKind::Muted),
                ),
            ]))
        }));

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style(StyleKind::Primary))
                    .style(Style::default().bg(theme.background))
                    .title(" Fork Session "),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.primary)
                    .fg(theme.selection_foreground())
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(Clear, popup);
        frame.render_stateful_widget(list, popup, &mut self.list_state);

        let hint_y = popup.y + popup.height;
        if hint_y < area.y + area.height {
            frame.render_widget(
                Paragraph::new(" Up/Down: Navigate  Enter: Fork  Esc: Close ")
                    .style(theme.style(StyleKind::Muted)),
                Rect {
                    x: popup.x,
                    y: hint_y,
                    width: popup.width,
                    height: 1,
                },
            );
        }
    }
}

fn one_line_preview(prompt: &str, max_chars: usize) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut preview = normalized
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    preview.push('…');
    preview
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
            turn_id: "turn-newest".to_string(),
            prompt: "Newest prompt".to_string(),
            timestamp: SystemTime::now(),
        }]);

        assert_eq!(
            selector.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ForkAction::Select(ForkTarget::FullSession)
        );
        selector.show(vec![SessionForkPoint {
            turn_id: "turn-newest".to_string(),
            prompt: "Newest prompt".to_string(),
            timestamp: SystemTime::now(),
        }]);
        selector.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            selector.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ForkAction::Select(ForkTarget::BeforeTurn {
                turn_id: "turn-newest".to_string(),
                prompt: "Newest prompt".to_string(),
            })
        );
    }
}
