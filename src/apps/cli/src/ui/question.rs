/// AskUserQuestion interactive prompt
///
/// Inspired by opencode TUI's QuestionPrompt component.
/// Supports:
/// - Single-select: pick one option, Enter submits immediately (single question)
/// - Multi-select: toggle options with Enter, then advance to next question
/// - Multiple questions: Tab/Shift+Tab to switch, Confirm page at the end
/// - Custom "Other" input: type your own answer
/// - Number shortcuts: 1-9 to quick-pick

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme::{Theme, StyleKind};

// ============ Data Types ============

/// A single question option
#[derive(Debug, Clone)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// A single question with its options
#[derive(Debug, Clone)]
pub struct QuestionData {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
}

/// Interactive question prompt state
#[derive(Debug, Clone)]
pub struct QuestionPrompt {
    pub tool_id: String,
    pub questions: Vec<QuestionData>,
    /// Current active question tab (0-based); equals questions.len() when on confirm page
    pub current_tab: usize,
    /// Per-question answers: question_index -> selected option labels
    pub answers: Vec<Vec<String>>,
    /// Per-question custom input text
    pub custom_inputs: Vec<String>,
    /// Selected option index within current question (includes "Other" as last)
    pub selected_option: usize,
    /// Whether in custom text editing mode
    pub editing_custom: bool,
}

/// Result of handling a key event in the question prompt
#[derive(Debug, Clone)]
pub enum QuestionAction {
    /// No action, continue showing the prompt
    None,
    /// User confirmed all answers — submit to core
    Submit(serde_json::Value),
    /// User dismissed the prompt
    Reject,
}

impl QuestionPrompt {
    /// Create from parsed AskUserQuestion params
    pub fn from_params(tool_id: String, params: &serde_json::Value) -> Option<Self> {
        let questions_val = params.get("questions")?.as_array()?;
        let mut questions = Vec::new();

        for q in questions_val {
            let question = q.get("question").and_then(|v| v.as_str())
                .unwrap_or("").to_string();
            let header = q.get("header").and_then(|v| v.as_str())
                .unwrap_or("").to_string();
            let multi_select = q.get("multiSelect").and_then(|v| v.as_bool())
                .or_else(|| q.get("multi_select").and_then(|v| v.as_bool()))
                .unwrap_or(false);

            let mut options = Vec::new();
            if let Some(opts) = q.get("options").and_then(|v| v.as_array()) {
                for opt in opts {
                    let label = opt.get("label").and_then(|v| v.as_str())
                        .unwrap_or("").to_string();
                    let description = opt.get("description").and_then(|v| v.as_str())
                        .unwrap_or("").to_string();
                    options.push(QuestionOption { label, description });
                }
            }

            questions.push(QuestionData {
                question,
                header,
                options,
                multi_select,
            });
        }

        if questions.is_empty() {
            return None;
        }

        let q_count = questions.len();
        Some(Self {
            tool_id,
            questions,
            current_tab: 0,
            answers: vec![Vec::new(); q_count],
            custom_inputs: vec![String::new(); q_count],
            selected_option: 0,
            editing_custom: false,
        })
    }

    /// Whether this is a single question with single-select (auto-submit on pick)
    fn is_single_auto_submit(&self) -> bool {
        self.questions.len() == 1 && !self.questions[0].multi_select
    }

    /// Whether we are on the confirm/review page (multi-question only)
    fn on_confirm_page(&self) -> bool {
        !self.is_single_auto_submit() && self.current_tab == self.questions.len()
    }

    /// Total number of tabs (questions + confirm page for multi-question)
    fn tab_count(&self) -> usize {
        if self.is_single_auto_submit() {
            1
        } else {
            self.questions.len() + 1
        }
    }

    /// Current question (None if on confirm page)
    fn current_question(&self) -> Option<&QuestionData> {
        self.questions.get(self.current_tab)
    }

    /// Total selectable items for current question (options + "Other")
    fn total_options(&self) -> usize {
        if let Some(q) = self.current_question() {
            q.options.len() + 1 // +1 for "Other"
        } else {
            0
        }
    }

    /// Whether the selected option is "Other"
    fn is_other_selected(&self) -> bool {
        if let Some(q) = self.current_question() {
            self.selected_option == q.options.len()
        } else {
            false
        }
    }

    /// Build the answers JSON payload for submission
    fn build_answers_payload(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (i, answer_list) in self.answers.iter().enumerate() {
            let q = &self.questions[i];
            // Replace "Other" with actual custom input
            let custom = &self.custom_inputs[i];
            let processed: Vec<String> = answer_list.iter().map(|a| {
                if a == "Other" && !custom.is_empty() {
                    custom.clone()
                } else {
                    a.clone()
                }
            }).collect();

            if q.multi_select {
                map.insert(
                    i.to_string(),
                    serde_json::Value::Array(
                        processed.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            } else {
                let val = processed.first().cloned().unwrap_or_default();
                map.insert(i.to_string(), serde_json::Value::String(val));
            }
        }
        serde_json::Value::Object(map)
    }

    /// Handle a key event. Returns a QuestionAction.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> QuestionAction {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return QuestionAction::None;
        }

        // Custom text editing mode
        if self.editing_custom && !self.on_confirm_page() {
            return self.handle_editing_key(key);
        }

        // Confirm page
        if self.on_confirm_page() {
            return self.handle_confirm_key(key);
        }

        // Normal question selection
        self.handle_question_key(key)
    }

    /// Handle keys when editing custom "Other" text
    fn handle_editing_key(&mut self, key: KeyEvent) -> QuestionAction {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.editing_custom = false;
                QuestionAction::None
            }
            (KeyCode::Enter, _) => {
                let text = self.custom_inputs[self.current_tab].trim().to_string();
                self.editing_custom = false;

                if text.is_empty() {
                    // Clear custom answer
                    let answers = &mut self.answers[self.current_tab];
                    answers.retain(|a| a != "Other");
                    return QuestionAction::None;
                }

                let q = &self.questions[self.current_tab];
                if q.multi_select {
                    // For multi-select: store custom text, toggle "Other" marker
                    self.custom_inputs[self.current_tab] = text.clone();
                    let answers = &mut self.answers[self.current_tab];
                    // Remove old "Other" and re-add with new text
                    answers.retain(|a| a != "Other");
                    answers.push("Other".to_string());
                    QuestionAction::None
                } else {
                    // For single-select: pick and advance
                    self.custom_inputs[self.current_tab] = text.clone();
                    self.answers[self.current_tab] = vec!["Other".to_string()];
                    if self.is_single_auto_submit() {
                        QuestionAction::Submit(self.build_answers_payload())
                    } else {
                        self.advance_tab();
                        QuestionAction::None
                    }
                }
            }
            (KeyCode::Backspace, _) => {
                self.custom_inputs[self.current_tab].pop();
                QuestionAction::None
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                let text = &self.custom_inputs[self.current_tab];
                if text.is_empty() {
                    self.editing_custom = false;
                } else {
                    self.custom_inputs[self.current_tab].clear();
                }
                QuestionAction::None
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                if !c.is_control() {
                    self.custom_inputs[self.current_tab].push(c);
                }
                QuestionAction::None
            }
            _ => QuestionAction::None,
        }
    }

    /// Handle keys on the confirm/review page
    fn handle_confirm_key(&mut self, key: KeyEvent) -> QuestionAction {
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => {
                QuestionAction::Submit(self.build_answers_payload())
            }
            (KeyCode::Esc, _) => {
                QuestionAction::Reject
            }
            // Navigate back to questions
            (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                self.current_tab = self.questions.len().saturating_sub(1);
                self.selected_option = 0;
                QuestionAction::None
            }
            (KeyCode::BackTab, _) => {
                self.current_tab = self.questions.len().saturating_sub(1);
                self.selected_option = 0;
                QuestionAction::None
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                // Wrap around to first question
                self.current_tab = 0;
                self.selected_option = 0;
                QuestionAction::None
            }
            _ => QuestionAction::None,
        }
    }

    /// Handle keys during normal question selection
    fn handle_question_key(&mut self, key: KeyEvent) -> QuestionAction {
        let total = self.total_options();

        match (key.code, key.modifiers) {
            // Navigate options vertically
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                if total > 0 {
                    self.selected_option = (self.selected_option + total - 1) % total;
                }
                QuestionAction::None
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                if total > 0 {
                    self.selected_option = (self.selected_option + 1) % total;
                }
                QuestionAction::None
            }

            // Navigate tabs (multi-question)
            (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                if self.tab_count() > 1 {
                    let tabs = self.tab_count();
                    self.current_tab = (self.current_tab + tabs - 1) % tabs;
                    self.selected_option = 0;
                }
                QuestionAction::None
            }
            (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if self.tab_count() > 1 {
                    let tabs = self.tab_count();
                    self.current_tab = (self.current_tab + 1) % tabs;
                    self.selected_option = 0;
                }
                QuestionAction::None
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                if self.tab_count() > 1 {
                    let tabs = self.tab_count();
                    self.current_tab = (self.current_tab + 1) % tabs;
                    self.selected_option = 0;
                }
                QuestionAction::None
            }
            (KeyCode::BackTab, _) => {
                if self.tab_count() > 1 {
                    let tabs = self.tab_count();
                    self.current_tab = (self.current_tab + tabs - 1) % tabs;
                    self.selected_option = 0;
                }
                QuestionAction::None
            }

            // Select / toggle
            (KeyCode::Enter, _) => {
                self.select_current_option()
            }

            // Number shortcuts (1-9)
            (KeyCode::Char(c), KeyModifiers::NONE) if c.is_ascii_digit() && c != '0' => {
                let digit = (c as u8 - b'0') as usize;
                if digit >= 1 && digit <= total.min(9) {
                    self.selected_option = digit - 1;
                    return self.select_current_option();
                }
                QuestionAction::None
            }

            // Escape = reject
            (KeyCode::Esc, _) => QuestionAction::Reject,

            _ => QuestionAction::None,
        }
    }

    /// Select or toggle the currently highlighted option
    fn select_current_option(&mut self) -> QuestionAction {
        if self.is_other_selected() {
            // Enter editing mode for custom input
            self.editing_custom = true;
            return QuestionAction::None;
        }

        let q = &self.questions[self.current_tab];
        let opt_label = q.options[self.selected_option].label.clone();

        if q.multi_select {
            // Toggle
            let answers = &mut self.answers[self.current_tab];
            if let Some(pos) = answers.iter().position(|a| a == &opt_label) {
                answers.remove(pos);
            } else {
                answers.push(opt_label);
            }
            QuestionAction::None
        } else {
            // Single-select: pick and advance
            self.answers[self.current_tab] = vec![opt_label];
            if self.is_single_auto_submit() {
                QuestionAction::Submit(self.build_answers_payload())
            } else {
                self.advance_tab();
                QuestionAction::None
            }
        }
    }

    /// Advance to the next tab
    fn advance_tab(&mut self) {
        if self.current_tab < self.tab_count() - 1 {
            self.current_tab += 1;
            self.selected_option = 0;
        }
    }
}

// ============ Rendering ============

/// Render the question overlay on top of the message area.
pub fn render_question_overlay(
    frame: &mut Frame,
    prompt: &QuestionPrompt,
    theme: &Theme,
    area: Rect,
) {
    if prompt.on_confirm_page() {
        render_confirm_page(frame, prompt, theme, area);
    } else {
        render_question_page(frame, prompt, theme, area);
    }
}

/// Render a single question page with options
fn render_question_page(
    frame: &mut Frame,
    prompt: &QuestionPrompt,
    theme: &Theme,
    area: Rect,
) {
    let q = match prompt.current_question() {
        Some(q) => q,
        None => return,
    };

    // Calculate overlay height: header(1) + question(2) + options + other + hint(2) + padding
    let options_count = q.options.len() + 1; // +1 for "Other"
    let description_lines: usize = q.options.iter()
        .map(|o| if o.description.is_empty() { 0 } else { 1 })
        .sum();
    let tab_line = if prompt.tab_count() > 1 { 2 } else { 0 };
    let editing_line = if prompt.editing_custom { 1 } else { 0 };
    let content_height = 2 + tab_line + options_count + description_lines + editing_line + 1; // question + options + descriptions + padding
    let overlay_height = (content_height as u16 + 3).min(area.height.saturating_sub(2)); // +3 for borders/hint

    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };

    frame.render_widget(Clear, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // content
            Constraint::Length(2), // hint bar
        ])
        .split(overlay_area);

    // Content block with accent left border
    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(theme.background_panel));

    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();

    // Tab bar (multi-question only)
    if prompt.tab_count() > 1 {
        let mut tab_spans = Vec::new();
        for (i, qd) in prompt.questions.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::raw("  "));
            }
            let is_active = i == prompt.current_tab;
            let is_answered = !prompt.answers[i].is_empty();
            if is_active {
                tab_spans.push(Span::styled(
                    format!(" {} ", qd.header),
                    Style::default()
                        .fg(theme.background)
                        .bg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                tab_spans.push(Span::styled(
                    format!(" {} ", qd.header),
                    Style::default().fg(
                        if is_answered { theme.success } else { theme.muted }
                    ),
                ));
            }
        }
        // Confirm tab
        tab_spans.push(Span::raw("  "));
        let confirm_active = prompt.on_confirm_page();
        if confirm_active {
            tab_spans.push(Span::styled(
                " Confirm ",
                Style::default()
                    .fg(theme.background)
                    .bg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(" Confirm ", theme.style(StyleKind::Muted)));
        }
        lines.push(Line::from(tab_spans));
        lines.push(Line::from(""));
    }

    // Question text
    let multi_hint = if q.multi_select { " (select all that apply)" } else { "" };
    lines.push(Line::from(Span::styled(
        format!("{}{}", q.question, multi_hint),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Options
    for (i, opt) in q.options.iter().enumerate() {
        let is_active = i == prompt.selected_option;
        let is_picked = prompt.answers[prompt.current_tab].contains(&opt.label);

        let number_style = if is_active {
            theme.style(StyleKind::Primary)
        } else {
            theme.style(StyleKind::Muted)
        };

        let label_style = if is_active {
            Style::default().fg(theme.primary)
        } else if is_picked {
            Style::default().fg(theme.success)
        } else {
            Style::default()
        };

        let marker = if q.multi_select {
            if is_picked { "[\u{2713}]" } else { "[ ]" }
        } else {
            if is_picked { "(\u{2022})" } else { "( )" }
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{}. ", i + 1), number_style),
            Span::styled(format!("{} ", marker), label_style),
            Span::styled(opt.label.clone(), label_style),
            if is_picked && !q.multi_select {
                Span::styled(" \u{2713}", theme.style(StyleKind::Success))
            } else {
                Span::raw("")
            },
        ]));

        if !opt.description.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(opt.description.clone(), theme.style(StyleKind::Muted)),
            ]));
        }
    }

    // "Other" option
    let other_idx = q.options.len();
    let is_other_active = prompt.selected_option == other_idx;
    let custom_text = &prompt.custom_inputs[prompt.current_tab];
    let is_other_picked = prompt.answers[prompt.current_tab].contains(&"Other".to_string());

    let other_style = if is_other_active {
        Style::default().fg(theme.primary)
    } else if is_other_picked {
        Style::default().fg(theme.success)
    } else {
        Style::default()
    };

    let other_marker = if q.multi_select {
        if is_other_picked { "[\u{2713}]" } else { "[ ]" }
    } else {
        if is_other_picked { "(\u{2022})" } else { "( )" }
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}. ", other_idx + 1),
            if is_other_active { theme.style(StyleKind::Primary) } else { theme.style(StyleKind::Muted) },
        ),
        Span::styled(format!("{} ", other_marker), other_style),
        Span::styled("Type your own answer", other_style),
    ]));

    // Show custom input field when editing
    if prompt.editing_custom {
        let display = if custom_text.is_empty() {
            "(type your answer)".to_string()
        } else {
            format!("{}\u{2588}", custom_text) // cursor block
        };
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                display,
                if custom_text.is_empty() {
                    theme.style(StyleKind::Muted)
                } else {
                    Style::default()
                },
            ),
        ]));
    } else if !custom_text.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(custom_text.clone(), theme.style(StyleKind::Muted)),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    // Hint bar
    render_question_hint_bar(frame, chunks[1], theme, prompt);
}

/// Render the confirm/review page (multi-question)
fn render_confirm_page(
    frame: &mut Frame,
    prompt: &QuestionPrompt,
    theme: &Theme,
    area: Rect,
) {
    let content_height = 3 + prompt.questions.len(); // title + blank + questions + padding
    let tab_line = 2;
    let overlay_height = ((content_height + tab_line) as u16 + 4).min(area.height.saturating_sub(2));

    let overlay_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(overlay_height),
        width: area.width,
        height: overlay_height,
    };

    frame.render_widget(Clear, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(overlay_area);

    let content_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::default().fg(theme.primary))
        .style(Style::default().bg(theme.background_panel));

    let inner = content_block.inner(chunks[0]);
    frame.render_widget(content_block, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();

    // Tab bar
    let mut tab_spans = Vec::new();
    for (i, qd) in prompt.questions.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::raw("  "));
        }
        let is_answered = !prompt.answers[i].is_empty();
        tab_spans.push(Span::styled(
            format!(" {} ", qd.header),
            Style::default().fg(if is_answered { theme.success } else { theme.muted }),
        ));
    }
    tab_spans.push(Span::raw("  "));
    tab_spans.push(Span::styled(
        " Confirm ",
        Style::default()
            .fg(theme.background)
            .bg(theme.primary)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::from(tab_spans));
    lines.push(Line::from(""));

    // Review title
    lines.push(Line::from(Span::styled(
        "Review your answers",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Answer summary per question
    for (i, q) in prompt.questions.iter().enumerate() {
        let answer_list = &prompt.answers[i];
        let custom = &prompt.custom_inputs[i];

        let display_answers: Vec<String> = answer_list.iter().map(|a| {
            if a == "Other" && !custom.is_empty() {
                custom.clone()
            } else {
                a.clone()
            }
        }).collect();

        let answered = !display_answers.is_empty();
        let value_text = if answered {
            display_answers.join(", ")
        } else {
            "(not answered)".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{}: ", q.header), theme.style(StyleKind::Muted)),
            Span::styled(
                value_text,
                if answered { Style::default() } else { theme.style(StyleKind::Error) },
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    // Hint bar
    let hint_block = Block::default()
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(hint_block, chunks[1]);

    let hint = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled("Enter", Style::default()),
        Span::styled(" submit  ", theme.style(StyleKind::Muted)),
        Span::styled("Tab", Style::default()),
        Span::styled(" back  ", theme.style(StyleKind::Muted)),
        Span::styled("Esc", Style::default()),
        Span::styled(" dismiss", theme.style(StyleKind::Muted)),
    ]))
    .style(Style::default().bg(theme.background_element));
    frame.render_widget(hint, chunks[1]);
}

/// Render the hint bar for question pages
fn render_question_hint_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    prompt: &QuestionPrompt,
) {
    let hint_block = Block::default()
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(hint_block, area);

    let mut spans = vec![Span::raw(" ")];

    if prompt.editing_custom {
        spans.push(Span::styled("Enter", Style::default()));
        spans.push(Span::styled(" confirm  ", theme.style(StyleKind::Muted)));
        spans.push(Span::styled("Esc", Style::default()));
        spans.push(Span::styled(" cancel", theme.style(StyleKind::Muted)));
    } else {
        if prompt.tab_count() > 1 {
            spans.push(Span::styled("Tab", Style::default()));
            spans.push(Span::styled(" switch  ", theme.style(StyleKind::Muted)));
        }
        spans.push(Span::styled("\u{2191}\u{2193}", Style::default()));
        spans.push(Span::styled(" select  ", theme.style(StyleKind::Muted)));
        spans.push(Span::styled("Enter", Style::default()));

        let q = prompt.current_question();
        let action_text = if q.map(|q| q.multi_select).unwrap_or(false) {
            " toggle  "
        } else if prompt.is_single_auto_submit() {
            " submit  "
        } else {
            " confirm  "
        };
        spans.push(Span::styled(action_text, theme.style(StyleKind::Muted)));

        spans.push(Span::styled("1-9", Style::default()));
        spans.push(Span::styled(" quick  ", theme.style(StyleKind::Muted)));
        spans.push(Span::styled("Esc", Style::default()));
        spans.push(Span::styled(" dismiss", theme.style(StyleKind::Muted)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(theme.background_element));
    frame.render_widget(paragraph, area);
}
