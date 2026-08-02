use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversationPoint {
    pub(super) id: String,
    title: String,
    footer: String,
}

impl ConversationPoint {
    pub(super) fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        footer: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            footer: footer.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ConversationSelectorAction {
    None,
    Move(String),
    Select(String),
    Close,
}

pub(super) struct ConversationSelectorState {
    points: Vec<ConversationPoint>,
    list_state: ListState,
    visible: bool,
    title: &'static str,
    confirm_label: &'static str,
}

impl ConversationSelectorState {
    pub(super) fn new(title: &'static str, confirm_label: &'static str) -> Self {
        Self {
            points: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            title,
            confirm_label,
        }
    }

    pub(super) fn show(&mut self, points: Vec<ConversationPoint>) {
        self.points = points;
        self.list_state
            .select((!self.points.is_empty()).then_some(0));
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

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> ConversationSelectorAction {
        if !self.visible {
            return ConversationSelectorAction::None;
        }
        match key.code {
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Enter => {
                let selected = self.selected_id();
                if selected.is_some() {
                    self.hide();
                }
                selected
                    .map(ConversationSelectorAction::Select)
                    .unwrap_or(ConversationSelectorAction::None)
            }
            KeyCode::Esc => {
                self.hide();
                ConversationSelectorAction::Close
            }
            _ => ConversationSelectorAction::None,
        }
    }

    fn move_selection(&mut self, delta: isize) -> ConversationSelectorAction {
        if self.points.is_empty() {
            return ConversationSelectorAction::None;
        }
        let selected = self.list_state.selected().unwrap_or(0) as isize;
        let next = (selected + delta).rem_euclid(self.points.len() as isize) as usize;
        self.list_state.select(Some(next));
        ConversationSelectorAction::Move(self.points[next].id.clone())
    }

    pub(super) fn selected_id(&self) -> Option<String> {
        self.points
            .get(self.list_state.selected()?)
            .map(|point| point.id.clone())
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }
        let width = area.width.saturating_sub(4).min(78);
        let height = (self.points.len() as u16 + 4).min(area.height.saturating_sub(2));
        if width < 24 || height < 4 {
            return;
        }
        let popup = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        };
        let preview_width = width.saturating_sub(16) as usize;
        let items = self.points.iter().map(|point| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    one_line_preview(&point.title, preview_width),
                    theme.style(StyleKind::Primary),
                ),
                Span::styled(format!("  {}", point.footer), theme.style(StyleKind::Muted)),
            ]))
        });
        let list = List::new(items.collect::<Vec<_>>())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style(StyleKind::Primary))
                    .style(Style::default().bg(theme.background))
                    .title(format!(" {} ", self.title)),
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
                Paragraph::new(format!(
                    " Up/Down: Navigate  Enter: {}  Esc: Close ",
                    self.confirm_label
                ))
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

#[cfg(test)]
mod tests {
    use super::{ConversationPoint, ConversationSelectorAction, ConversationSelectorState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn navigation_wraps_and_reports_preview_moves_without_closing() {
        let mut selector = ConversationSelectorState::new("Timeline", "Jump");
        selector.show(vec![
            ConversationPoint::new("new", "Newest prompt", "12:00"),
            ConversationPoint::new("old", "Oldest prompt", "11:00"),
        ]);

        assert_eq!(
            selector.handle_key_event(key(KeyCode::Up)),
            ConversationSelectorAction::Move("old".to_string())
        );
        assert!(selector.is_visible());
        assert_eq!(
            selector.handle_key_event(key(KeyCode::Down)),
            ConversationSelectorAction::Move("new".to_string())
        );
        assert_eq!(
            selector.handle_key_event(key(KeyCode::Enter)),
            ConversationSelectorAction::Select("new".to_string())
        );
        assert!(!selector.is_visible());
    }
}
