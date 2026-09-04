use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use openbitfun_product_domains::external_sources::PromptCommandShellReviewPlan;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::permission::render_button_bar;
use super::theme::Theme;
use super::widgets::wrapped_line_count;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptCommandShellReviewAction {
    None,
    RunOnce,
    Remember,
    Cancel,
}

#[derive(Debug, Clone)]
pub(crate) struct PromptCommandShellReviewPrompt {
    pub(crate) plan: PromptCommandShellReviewPlan,
    selected_option: usize,
    scroll: u16,
    max_scroll: u16,
}

impl PromptCommandShellReviewPrompt {
    pub(crate) fn new(plan: PromptCommandShellReviewPlan) -> Self {
        Self {
            plan,
            selected_option: 0,
            scroll: 0,
            max_scroll: 0,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> PromptCommandShellReviewAction {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return PromptCommandShellReviewAction::None;
        }
        let last_option = if self.plan.can_remember { 2 } else { 1 };
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_option = self.selected_option.saturating_sub(1);
                PromptCommandShellReviewAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected_option = (self.selected_option + 1).min(last_option);
                PromptCommandShellReviewAction::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                PromptCommandShellReviewAction::None
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1).min(self.max_scroll);
                PromptCommandShellReviewAction::None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                PromptCommandShellReviewAction::None
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10).min(self.max_scroll);
                PromptCommandShellReviewAction::None
            }
            KeyCode::Esc => PromptCommandShellReviewAction::Cancel,
            KeyCode::Enter => match (self.plan.can_remember, self.selected_option) {
                (_, 0) => PromptCommandShellReviewAction::RunOnce,
                (true, 1) => PromptCommandShellReviewAction::Remember,
                _ => PromptCommandShellReviewAction::Cancel,
            },
            _ => PromptCommandShellReviewAction::None,
        }
    }

    pub(crate) fn render(&mut self, frame: &mut Frame, theme: &Theme, area: Rect) {
        let overlay_height = 20u16.min(area.height.saturating_sub(2));
        let overlay_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(overlay_height),
            width: area.width,
            height: overlay_height,
        };
        frame.render_widget(Clear, overlay_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(2)])
            .split(overlay_area);
        let block = Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .border_style(Style::default().fg(theme.warning))
            .style(Style::default().bg(theme.background_panel));
        let inner = block.inner(chunks[0]);
        frame.render_widget(block, chunks[0]);

        let mut lines = vec![
            Line::from(Span::styled(
                "External prompt command wants to run local commands",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(format!(
                "Source: {}  Shell: {}",
                self.plan.source_display_name, self.plan.shell_display_name
            )),
            Line::from(format!("Executable: {}", self.plan.shell_executable)),
            Line::from(format!("Directory: {}", self.plan.working_directory)),
            Line::from("Only standard output is added to the prompt."),
            Line::from(""),
        ];
        for (index, command) in self.plan.commands.iter().enumerate() {
            lines.push(Line::from(format!("{}. $ {command}", index + 1)));
            if index + 1 != self.plan.commands.len() {
                lines.push(Line::from(""));
            }
        }
        let rendered_text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        self.max_scroll =
            wrapped_line_count(&rendered_text, inner.width).saturating_sub(inner.height);
        self.scroll = self.scroll.min(self.max_scroll);
        frame.render_widget(paragraph.scroll((self.scroll, 0)), inner);

        if self.plan.can_remember {
            render_button_bar(
                frame,
                chunks[1],
                theme,
                &["Run once", "Remember exact plan", "Cancel"],
                self.selected_option,
                "⇆ select  ↑↓ scroll  Enter confirm  Esc cancel",
            );
        } else {
            render_button_bar(
                frame,
                chunks[1],
                theme,
                &["Run once", "Cancel"],
                self.selected_option,
                "⇆ select  ↑↓ scroll  Enter confirm  Esc cancel",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PromptCommandShellReviewAction, PromptCommandShellReviewPrompt};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use openbitfun_product_domains::external_sources::PromptCommandShellReviewPlan;

    fn plan(can_remember: bool) -> PromptCommandShellReviewPlan {
        PromptCommandShellReviewPlan {
            schema_version: 1,
            plan_fingerprint: "sha256:plan".to_string(),
            source_display_name: "OpenCode".to_string(),
            working_directory: "D:/workspace".to_string(),
            shell_display_name: "PowerShell 7".to_string(),
            shell_executable: "C:/Program Files/PowerShell/7/pwsh.exe".to_string(),
            commands: vec!["git status".to_string()],
            can_remember,
            preference_revision: 3,
        }
    }

    #[test]
    fn static_plan_offers_run_once_remember_and_cancel() {
        let mut prompt = PromptCommandShellReviewPrompt::new(plan(true));
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptCommandShellReviewAction::Remember
        );
        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PromptCommandShellReviewAction::Cancel
        );
    }

    #[test]
    fn dynamic_plan_never_exposes_remember() {
        let mut prompt = PromptCommandShellReviewPrompt::new(plan(false));
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptCommandShellReviewAction::Cancel
        );
    }
}
