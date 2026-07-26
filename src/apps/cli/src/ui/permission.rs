/// Permission request modal panel.
///
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::string_utils::truncate_str;
use super::theme::Theme;
use bitfun_agent_runtime::sdk::{PermissionReply, PermissionRequest};

#[derive(Debug, Clone)]
pub(crate) struct PermissionPrompt {
    pub(crate) request: PermissionRequest,
    pub(crate) selected_option: usize,
    reject_feedback: String,
    editing_reject_feedback: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PermissionAction {
    None,
    Reply(PermissionReply),
}

impl PermissionPrompt {
    pub(crate) fn new(request: PermissionRequest) -> Self {
        Self {
            request,
            selected_option: 0,
            reject_feedback: String::new(),
            editing_reject_feedback: false,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> PermissionAction {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return PermissionAction::None;
        }
        if self.editing_reject_feedback {
            return match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => PermissionAction::Reply(PermissionReply::Reject {
                    feedback: match self.reject_feedback.trim() {
                        "" => None,
                        feedback => Some(feedback.to_string()),
                    },
                }),
                (KeyCode::Esc, _) => {
                    self.editing_reject_feedback = false;
                    PermissionAction::None
                }
                (KeyCode::Backspace, _) => {
                    self.reject_feedback.pop();
                    PermissionAction::None
                }
                (KeyCode::Char(character), KeyModifiers::NONE | KeyModifiers::SHIFT)
                    if !character.is_control() =>
                {
                    self.reject_feedback.push(character);
                    PermissionAction::None
                }
                _ => PermissionAction::None,
            };
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_option = self.selected_option.saturating_sub(1);
                PermissionAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected_option = (self.selected_option + 1).min(2);
                PermissionAction::None
            }
            KeyCode::Esc => PermissionAction::Reply(PermissionReply::Reject { feedback: None }),
            KeyCode::Enter => match self.selected_option {
                0 => PermissionAction::Reply(PermissionReply::Once),
                1 => PermissionAction::Reply(PermissionReply::Always),
                _ => {
                    self.editing_reject_feedback = true;
                    PermissionAction::None
                }
            },
            _ => PermissionAction::None,
        }
    }
}

// ============ Rendering ============

fn permission_delegation_lines(request: &PermissionRequest) -> Option<[String; 2]> {
    let delegation = request.delegation.as_ref()?;
    Some([
        format!(
            "Subagent: {}  Child session: {}",
            delegation.subagent_type, request.session_id
        ),
        format!(
            "Parent session: {}  Task: {}",
            delegation.parent_session_id, delegation.parent_tool_call_id
        ),
    ])
}

fn permission_project_display_label(request: &PermissionRequest) -> &str {
    request
        .project_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(&request.project_id)
}

fn permission_footer_secondary_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.primary)
        .bg(theme.background_element)
}

pub(super) fn render_permission_overlay(
    frame: &mut Frame,
    prompt: &PermissionPrompt,
    theme: &Theme,
    area: Rect,
) {
    let overlay_height = 11u16.min(area.height.saturating_sub(2));
    let overlay_height = if prompt.request.delegation.is_some() {
        overlay_height.saturating_add(2)
    } else {
        overlay_height
    }
    .min(area.height.saturating_sub(2));
    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };
    frame.render_widget(Clear, overlay_area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(2)])
        .split(overlay_area);
    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.background_panel));
    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    let request = &prompt.request;
    let resources = request
        .resources
        .iter()
        .map(|resource| truncate_str(resource, 80))
        .collect::<Vec<_>>()
        .join(", ");
    let save_scope = if request.save_resources.is_empty() {
        "No remembered scope".to_string()
    } else {
        format!(
            "Always saves {} project resource(s)",
            request.save_resources.len()
        )
    };
    let risk = request
        .display_metadata
        .get("riskDescription")
        .or_else(|| request.display_metadata.get("risk"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("No additional risk information");
    let mut lines = vec![
        Line::from(Span::styled(
            "Permission required",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "Action: {}  Source: {:?}:{}",
            request.action, request.source.kind, request.source.identity
        )),
    ];
    if let Some(delegation_lines) = permission_delegation_lines(request) {
        lines.extend(delegation_lines.map(Line::from));
    }
    lines.extend([
        Line::from(format!("Resources: {resources}")),
        Line::from(format!(
            "Project: {}  {save_scope}",
            permission_project_display_label(request)
        )),
        Line::from(format!("Risk: {}", truncate_str(risk, 100))),
        if prompt.editing_reject_feedback {
            Line::from(format!(
                "Rejection feedback (optional): {}_",
                prompt.reject_feedback
            ))
        } else {
            Line::from("")
        },
    ]);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    render_button_bar(
        frame,
        chunks[1],
        theme,
        if prompt.editing_reject_feedback {
            &["Submit reject"]
        } else {
            &["Allow once", "Always allow", "Reject"]
        },
        if prompt.editing_reject_feedback {
            0
        } else {
            prompt.selected_option
        },
        if prompt.editing_reject_feedback {
            "Enter submit  Esc back"
        } else {
            "\u{21c6} select  Enter confirm  Esc reject"
        },
    );
}

/// Render a horizontal button bar with selectable options
fn render_button_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    options: &[&str],
    selected: usize,
    hint_text: &str,
) {
    let bar_block = Block::default().style(Style::default().bg(theme.background_element));
    frame.render_widget(bar_block, area);

    // Build button spans
    let mut spans = vec![Span::raw(" ")];
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if i == selected {
            spans.push(Span::styled(
                format!(" {} ", option),
                Style::default()
                    .fg(theme.background)
                    .bg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", option),
                permission_footer_secondary_style(theme),
            ));
        }
    }

    // Add hint text on the right side if there's room
    let buttons_width: usize = spans.iter().map(|s| s.width()).sum();
    let hint_width = hint_text.len() + 2;
    if buttons_width + hint_width < area.width as usize {
        let padding = area.width as usize - buttons_width - hint_width;
        spans.push(Span::raw(" ".repeat(padding)));
        spans.push(Span::styled(
            hint_text,
            permission_footer_secondary_style(theme),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(theme.background_element));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::{
        permission_delegation_lines, permission_footer_secondary_style,
        permission_project_display_label, PermissionAction, PermissionPrompt,
    };
    use crate::ui::theme::{builtin_theme_json, Appearance, EffectiveColorScheme, Theme};
    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionReply, PermissionRequest, PermissionRequestSource,
        PermissionRequestSourceKind,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serde_json::Map;

    fn request() -> PermissionRequest {
        PermissionRequest {
            request_id: "request-1".to_string(),
            round_id: "synthetic:request-1".to_string(),
            order: 0,
            tool_call_id: None,
            project_path: None,
            project_id: "project-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agentic".to_string(),
            action: "edit".to_string(),
            resources: vec!["src/main.rs".to_string()],
            save_resources: vec!["src/main.rs".to_string()],
            source: PermissionRequestSource {
                kind: PermissionRequestSourceKind::ToolCall,
                identity: "write_file".to_string(),
            },
            delegation: None,
            display_metadata: Map::new(),
        }
    }

    #[test]
    fn v2_prompt_returns_project_always_reply_without_using_legacy_runtime_scope() {
        let mut prompt = PermissionPrompt::new(request());
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PermissionAction::Reply(PermissionReply::Always)
        );
    }

    #[test]
    fn v2_prompt_collects_optional_rejection_feedback() {
        let mut prompt = PermissionPrompt::new(request());
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        prompt.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PermissionAction::None
        );
        for character in "read only".chars() {
            prompt.handle_key_event(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }

        assert_eq!(
            prompt.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PermissionAction::Reply(PermissionReply::Reject {
                feedback: Some("read only".to_string()),
            })
        );
    }

    #[test]
    fn delegated_prompt_names_the_child_and_parent_task_context() {
        let mut request = request();
        request.session_id = "child-session".to_string();
        request.agent_id = "Explore".to_string();
        request.delegation = Some(PermissionDelegationContext {
            parent_session_id: "parent-session".to_string(),
            parent_dialog_turn_id: Some("parent-turn".to_string()),
            parent_tool_call_id: "parent-task".to_string(),
            subagent_type: "Explore".to_string(),
        });

        assert_eq!(
            permission_delegation_lines(&request),
            Some([
                "Subagent: Explore  Child session: child-session".to_string(),
                "Parent session: parent-session  Task: parent-task".to_string(),
            ])
        );
    }

    #[test]
    fn permission_prompt_prefers_a_nonempty_project_path_for_display() {
        let mut with_path = request();
        with_path.project_path = Some("  E:/Projects/BitFun  ".to_string());
        assert_eq!(
            permission_project_display_label(&with_path),
            "E:/Projects/BitFun"
        );

        let mut empty_path = request();
        empty_path.project_path = Some("   ".to_string());
        assert_eq!(permission_project_display_label(&empty_path), "project-1");
    }

    #[test]
    fn permission_footer_secondary_content_remains_visible_in_ansi16_themes() {
        for theme_id in ["bitfun-dark", "bitfun-midnight", "bitfun-tokyo-night"] {
            let theme = Theme::dark()
                .apply_opencode_theme_json(
                    builtin_theme_json(theme_id).expect("built-in theme must exist"),
                    Appearance::Dark,
                )
                .expect("built-in theme must resolve")
                .with_effective_scheme(EffectiveColorScheme::Ansi16);
            let style = permission_footer_secondary_style(&theme);

            assert_eq!(style.fg, Some(theme.primary), "{theme_id}");
            assert_eq!(style.bg, Some(theme.background_element), "{theme_id}");
            assert_ne!(style.fg, style.bg, "{theme_id}");
        }
    }
}
