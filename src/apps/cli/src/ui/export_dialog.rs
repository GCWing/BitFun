use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::responsive_popup::{render_too_small, responsive_popup, ResponsivePopup};
use crate::ui::theme::{StyleKind, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportDialogRequest {
    pub(crate) filename: String,
    pub(crate) include_reasoning: bool,
    pub(crate) include_tool_details: bool,
    pub(crate) open_in_editor: bool,
    pub(crate) save_to_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExportDialogAction {
    None,
    Confirm(ExportDialogRequest),
    ConfirmOverwrite(ExportDialogRequest),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportField {
    Filename,
    IncludeReasoning,
    IncludeToolDetails,
    OpenInEditor,
    SaveToFile,
}

impl ExportField {
    fn next(self, backwards: bool) -> Self {
        let fields = [
            Self::Filename,
            Self::IncludeReasoning,
            Self::IncludeToolDetails,
            Self::OpenInEditor,
            Self::SaveToFile,
        ];
        let current = fields.iter().position(|field| *field == self).unwrap_or(0);
        let next = if backwards {
            current.checked_sub(1).unwrap_or(fields.len() - 1)
        } else {
            (current + 1) % fields.len()
        };
        fields[next]
    }
}

pub(super) struct ExportDialogState {
    visible: bool,
    filename: String,
    filename_cursor: usize,
    include_reasoning: bool,
    include_tool_details: bool,
    open_in_editor: bool,
    save_to_file: bool,
    field: ExportField,
    overwrite_target: Option<String>,
    error: Option<String>,
    interaction_enabled: bool,
}

impl ExportDialogState {
    pub(super) fn new() -> Self {
        Self {
            visible: false,
            filename: String::new(),
            filename_cursor: 0,
            include_reasoning: false,
            include_tool_details: false,
            open_in_editor: false,
            save_to_file: true,
            field: ExportField::Filename,
            overwrite_target: None,
            error: None,
            interaction_enabled: true,
        }
    }

    pub(super) fn show(&mut self, filename: String) {
        self.visible = true;
        self.filename = filename;
        self.filename_cursor = self.filename.chars().count();
        self.include_reasoning = false;
        self.include_tool_details = false;
        self.open_in_editor = false;
        self.save_to_file = true;
        self.field = ExportField::Filename;
        self.overwrite_target = None;
        self.error = None;
        self.interaction_enabled = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.overwrite_target = None;
        self.error = None;
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    #[cfg(test)]
    pub(super) fn filename(&self) -> &str {
        &self.filename
    }

    #[cfg(test)]
    pub(super) fn include_reasoning(&self) -> bool {
        self.include_reasoning
    }

    #[cfg(test)]
    pub(super) fn include_tool_details(&self) -> bool {
        self.include_tool_details
    }

    #[cfg(test)]
    pub(super) fn open_in_editor(&self) -> bool {
        self.open_in_editor
    }

    #[cfg(test)]
    pub(super) fn save_to_file(&self) -> bool {
        self.save_to_file
    }

    #[cfg(test)]
    pub(super) fn is_confirming_overwrite(&self) -> bool {
        self.overwrite_target.is_some()
    }

    pub(super) fn request_overwrite_confirmation(&mut self, target: String) {
        self.overwrite_target = Some(target);
        self.error = None;
    }

    pub(super) fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.overwrite_target = None;
    }

    pub(super) fn insert_text(&mut self, text: &str) {
        if !self.visible
            || !self.interaction_enabled
            || self.overwrite_target.is_some()
            || self.field != ExportField::Filename
        {
            return;
        }
        for character in text
            .chars()
            .filter(|character| !matches!(character, '\r' | '\n' | '\t'))
        {
            insert_char(&mut self.filename, &mut self.filename_cursor, character);
        }
    }

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> ExportDialogAction {
        if !self.visible {
            return ExportDialogAction::None;
        }
        if !self.interaction_enabled {
            if key.code == KeyCode::Esc {
                self.hide();
                return ExportDialogAction::Cancel;
            }
            return ExportDialogAction::None;
        }
        if self.overwrite_target.is_some() {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let request = self.request();
                    ExportDialogAction::ConfirmOverwrite(request)
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.overwrite_target = None;
                    ExportDialogAction::None
                }
                _ => ExportDialogAction::None,
            };
        }

        self.error = None;
        match key.code {
            KeyCode::Esc => {
                self.hide();
                ExportDialogAction::Cancel
            }
            KeyCode::Tab => {
                self.field = self.field.next(key.modifiers.contains(KeyModifiers::SHIFT));
                ExportDialogAction::None
            }
            KeyCode::BackTab => {
                self.field = self.field.next(true);
                ExportDialogAction::None
            }
            KeyCode::Up => {
                self.field = self.field.next(true);
                ExportDialogAction::None
            }
            KeyCode::Down => {
                self.field = self.field.next(false);
                ExportDialogAction::None
            }
            KeyCode::Char(' ')
                if self.field != ExportField::Filename && key.modifiers.is_empty() =>
            {
                self.toggle_selected();
                ExportDialogAction::None
            }
            KeyCode::Enter => {
                if self.save_to_file && self.filename.trim().is_empty() {
                    self.error = Some("Enter a relative Markdown file path".to_string());
                    return ExportDialogAction::None;
                }
                if !self.save_to_file && !self.open_in_editor {
                    self.error =
                        Some("Enable Save to file or Open in editor before exporting".to_string());
                    return ExportDialogAction::None;
                }
                ExportDialogAction::Confirm(self.request())
            }
            KeyCode::Char('u')
                if self.field == ExportField::Filename
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.filename.clear();
                self.filename_cursor = 0;
                ExportDialogAction::None
            }
            KeyCode::Char(character)
                if self.field == ExportField::Filename
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) =>
            {
                insert_char(&mut self.filename, &mut self.filename_cursor, character);
                ExportDialogAction::None
            }
            KeyCode::Backspace if self.field == ExportField::Filename => {
                backspace(&mut self.filename, &mut self.filename_cursor);
                ExportDialogAction::None
            }
            KeyCode::Delete if self.field == ExportField::Filename => {
                delete_forward(&mut self.filename, self.filename_cursor);
                ExportDialogAction::None
            }
            KeyCode::Left if self.field == ExportField::Filename => {
                self.filename_cursor = self.filename_cursor.saturating_sub(1);
                ExportDialogAction::None
            }
            KeyCode::Right if self.field == ExportField::Filename => {
                self.filename_cursor =
                    (self.filename_cursor + 1).min(self.filename.chars().count());
                ExportDialogAction::None
            }
            KeyCode::Home if self.field == ExportField::Filename => {
                self.filename_cursor = 0;
                ExportDialogAction::None
            }
            KeyCode::End if self.field == ExportField::Filename => {
                self.filename_cursor = self.filename.chars().count();
                ExportDialogAction::None
            }
            _ => ExportDialogAction::None,
        }
    }

    fn toggle_selected(&mut self) {
        match self.field {
            ExportField::Filename => {}
            ExportField::IncludeReasoning => self.include_reasoning = !self.include_reasoning,
            ExportField::IncludeToolDetails => {
                self.include_tool_details = !self.include_tool_details
            }
            ExportField::OpenInEditor => self.open_in_editor = !self.open_in_editor,
            ExportField::SaveToFile => self.save_to_file = !self.save_to_file,
        }
    }

    fn request(&self) -> ExportDialogRequest {
        ExportDialogRequest {
            filename: self.filename.trim().to_string(),
            include_reasoning: self.include_reasoning,
            include_tool_details: self.include_tool_details,
            open_in_editor: self.open_in_editor,
            save_to_file: self.save_to_file,
        }
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }
        let dialog_area = match responsive_popup(area, 70, 11, 28, 9) {
            ResponsivePopup::Content(area) => area,
            ResponsivePopup::TooSmall(area) => {
                self.interaction_enabled = false;
                render_too_small(frame, area, theme, "Export session");
                return;
            }
        };
        self.interaction_enabled = true;
        frame.render_widget(Clear, dialog_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Export session ");
        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        if let Some(target) = self.overwrite_target.as_deref() {
            let text = format!(
                "{target} already exists.\n\nOverwrite it?\n\nEnter/Y confirm   N/Esc back"
            );
            frame.render_widget(
                Paragraph::new(text).style(theme.style(StyleKind::Warning)),
                inner,
            );
            return;
        }

        let selected = |field| self.field == field;
        let line = |field, label: String| {
            let marker = if selected(field) { "> " } else { "  " };
            let style = if selected(field) {
                theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(marker, style),
                Span::styled(label, style),
            ])
        };
        let check = |value| if value { "[x]" } else { "[ ]" };
        let mut lines = vec![
            line(ExportField::Filename, format!("File: {}", self.filename)),
            line(
                ExportField::IncludeReasoning,
                format!("{} Include reasoning", check(self.include_reasoning)),
            ),
            line(
                ExportField::IncludeToolDetails,
                format!("{} Include tool details", check(self.include_tool_details)),
            ),
            line(
                ExportField::OpenInEditor,
                format!("{} Open in editor", check(self.open_in_editor)),
            ),
            line(
                ExportField::SaveToFile,
                format!("{} Save to file", check(self.save_to_file)),
            ),
            Line::from(Span::styled(
                "Tab/Up/Down move   Space toggle   Enter export   Esc cancel",
                theme.style(StyleKind::Muted),
            )),
        ];
        if let Some(error) = self.error.as_deref() {
            lines.push(Line::from(Span::styled(
                error,
                theme.style(StyleKind::Error),
            )));
        }
        frame.render_widget(Paragraph::new(lines), inner);

        if self.field == ExportField::Filename {
            let prefix_width = 8u16;
            let before_cursor = self
                .filename
                .chars()
                .take(self.filename_cursor)
                .collect::<String>();
            let cursor = unicode_width::UnicodeWidthStr::width(before_cursor.as_str())
                .min(u16::MAX as usize) as u16;
            frame.set_cursor_position((
                inner
                    .x
                    .saturating_add(prefix_width)
                    .saturating_add(cursor)
                    .min(inner.right().saturating_sub(1)),
                inner.y,
            ));
        }
    }
}

fn char_to_byte(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

fn insert_char(value: &mut String, cursor: &mut usize, character: char) {
    let byte = char_to_byte(value, *cursor);
    value.insert(byte, character);
    *cursor += 1;
}

fn backspace(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = char_to_byte(value, *cursor - 1);
    let end = char_to_byte(value, *cursor);
    value.replace_range(start..end, "");
    *cursor -= 1;
}

fn delete_forward(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = char_to_byte(value, cursor);
    let end = char_to_byte(value, cursor + 1);
    value.replace_range(start..end, "");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn export_dialog_starts_with_safe_defaults() {
        let mut dialog = ExportDialogState::new();
        dialog.show("session-12345678.md".to_string());

        assert!(dialog.is_visible());
        assert_eq!(dialog.filename(), "session-12345678.md");
        assert!(!dialog.include_reasoning());
        assert!(!dialog.include_tool_details());
        assert!(!dialog.open_in_editor());
        assert!(dialog.save_to_file());
    }

    #[test]
    fn tab_and_space_toggle_only_the_focused_option() {
        let mut dialog = ExportDialogState::new();
        dialog.show("session.md".to_string());

        assert_eq!(
            dialog.handle_key_event(key(KeyCode::Tab)),
            ExportDialogAction::None
        );
        assert_eq!(
            dialog.handle_key_event(key(KeyCode::Char(' '))),
            ExportDialogAction::None
        );
        assert!(dialog.include_reasoning());
        assert!(!dialog.include_tool_details());
    }

    #[test]
    fn overwrite_decline_returns_to_the_unchanged_form() {
        let mut dialog = ExportDialogState::new();
        dialog.show("existing.md".to_string());
        dialog.request_overwrite_confirmation("existing.md".to_string());

        assert_eq!(
            dialog.handle_key_event(key(KeyCode::Char('n'))),
            ExportDialogAction::None
        );
        assert_eq!(dialog.filename(), "existing.md");
        assert!(dialog.is_visible());
        assert!(!dialog.is_confirming_overwrite());
    }

    #[test]
    fn modified_character_keys_do_not_change_the_filename() {
        let mut dialog = ExportDialogState::new();
        dialog.show("session.md".to_string());

        assert_eq!(
            dialog.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL,)),
            ExportDialogAction::None
        );
        assert_eq!(dialog.filename(), "session.md");
    }
}
