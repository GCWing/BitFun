/// Provider selector popup for "Add Model"
///
/// First step of the add-model wizard. Shows two groups:
/// - **Providers**: Preset AI providers with auto-filled configuration
/// - **Custom**: Add a fully custom model configuration
///
/// After selection, triggers the model config form with pre-filled values.
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

/// A preset provider template
#[derive(Debug, Clone)]
pub struct ProviderTemplate {
    pub name: String,
    pub base_url: String,
    /// "openai" or "anthropic"
    pub format: String,
    pub models: Vec<String>,
    pub description: String,
}

/// The result of selecting from the provider list
#[derive(Debug, Clone)]
pub enum ProviderSelection {
    /// User selected a preset provider
    Provider(ProviderTemplate),
    /// User selected "Add Custom model"
    Custom,
}

/// Build built-in provider templates used by CLI model configuration.
pub fn builtin_provider_templates() -> Vec<ProviderTemplate> {
    vec![
        ProviderTemplate {
            name: "ZhiPu AI".into(),
            base_url: "https://open.bigmodel.cn/api/paas/v4/chat/completions".into(),
            format: "openai".into(),
            models: vec!["glm-4.7".into(), "glm-4.7-flash".into(), "glm-4.6".into()],
            description: "ZhiPu GLM series".into(),
        },
        ProviderTemplate {
            name: "Qwen (Alibaba)".into(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".into(),
            format: "openai".into(),
            models: vec![
                "qwen3-max".into(),
                "qwen3-coder-plus".into(),
                "qwen3-coder-flash".into(),
            ],
            description: "Alibaba Qwen series".into(),
        },
        ProviderTemplate {
            name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com/chat/completions".into(),
            format: "openai".into(),
            models: vec!["deepseek-chat".into(), "deepseek-reasoner".into()],
            description: "DeepSeek AI models".into(),
        },
        ProviderTemplate {
            name: "Volcengine".into(),
            base_url: "https://ark.cn-beijing.volces.com/api/v3/chat/completions".into(),
            format: "openai".into(),
            models: vec!["doubao-seed-1-8-251228".into(), "glm-4-7-251222".into()],
            description: "ByteDance Volcengine".into(),
        },
        ProviderTemplate {
            name: "MiniMax".into(),
            base_url: "https://api.minimaxi.com/anthropic/v1/messages".into(),
            format: "anthropic".into(),
            models: vec![
                "MiniMax-M2.1".into(),
                "MiniMax-M2.1-lightning".into(),
                "MiniMax-M2".into(),
            ],
            description: "MiniMax AI models".into(),
        },
        ProviderTemplate {
            name: "Moonshot (Kimi)".into(),
            base_url: "https://api.moonshot.cn/v1/chat/completions".into(),
            format: "openai".into(),
            models: vec![
                "kimi-k2.5".into(),
                "kimi-k2".into(),
                "kimi-k2-thinking".into(),
            ],
            description: "Moonshot Kimi series".into(),
        },
        ProviderTemplate {
            name: "Anthropic".into(),
            base_url: "https://api.anthropic.com/v1/messages".into(),
            format: "anthropic".into(),
            models: vec![
                "claude-opus-4-6".into(),
                "claude-sonnet-4-5-20250929".into(),
                "claude-haiku-4-5-20251001".into(),
            ],
            description: "Anthropic Claude series".into(),
        },
    ]
}

// ── Flattened display row ──

#[derive(Debug, Clone)]
enum DisplayRow {
    /// Group header ("PROVIDERS" or "CUSTOM")
    GroupHeader(String),
    /// A selectable provider item (index into templates vec)
    Provider(usize),
    /// The "Add Custom model" item
    Custom,
}

/// Provider selector popup state
pub struct ProviderSelectorState {
    visible: bool,
    templates: Vec<ProviderTemplate>,
    /// Flattened rows for rendering
    rows: Vec<DisplayRow>,
    /// Indices of selectable rows (into `rows`)
    selectable_row_indices: Vec<usize>,
    /// Currently highlighted selectable index
    selected: usize,
    /// Viewport scroll offset
    scroll_offset: usize,
    /// Number of visible content rows (updated each frame)
    visible_rows: usize,
    /// Last rendered popup area (for mouse hit-testing)
    last_area: Option<Rect>,
}

impl ProviderSelectorState {
    pub fn new() -> Self {
        Self {
            visible: false,
            templates: Vec::new(),
            rows: Vec::new(),
            selectable_row_indices: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_rows: 0,
            last_area: None,
        }
    }

    pub fn show(&mut self) {
        self.templates = builtin_provider_templates();
        self.selected = 0;
        self.scroll_offset = 0;
        self.visible = true;
        self.build_rows();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.templates.clear();
        self.rows.clear();
        self.selectable_row_indices.clear();
        self.last_area = None;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Reshow the provider selector (for back navigation)
    pub fn reshow(&mut self) {
        if !self.templates.is_empty() || !builtin_provider_templates().is_empty() {
            self.templates = builtin_provider_templates();
            self.build_rows();
            self.visible = true;
        }
    }

    fn build_rows(&mut self) {
        self.rows.clear();
        self.selectable_row_indices.clear();

        // Providers group
        self.rows.push(DisplayRow::GroupHeader("PROVIDERS".into()));
        for i in 0..self.templates.len() {
            let row_idx = self.rows.len();
            self.selectable_row_indices.push(row_idx);
            self.rows.push(DisplayRow::Provider(i));
        }

        // Custom group
        self.rows.push(DisplayRow::GroupHeader("CUSTOM".into()));
        let row_idx = self.rows.len();
        self.selectable_row_indices.push(row_idx);
        self.rows.push(DisplayRow::Custom);
    }

    fn move_up(&mut self) {
        if self.selectable_row_indices.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.ensure_selected_visible();
    }

    fn move_down(&mut self) {
        if self.selectable_row_indices.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.selectable_row_indices.len() - 1);
        self.ensure_selected_visible();
    }

    fn ensure_selected_visible(&mut self) {
        if self.selectable_row_indices.is_empty() || self.visible_rows == 0 {
            return;
        }
        let row_idx = self.selectable_row_indices[self.selected];
        if row_idx < self.scroll_offset {
            self.scroll_offset = row_idx.saturating_sub(1); // show group header too
        } else if row_idx >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = row_idx.saturating_sub(self.visible_rows - 1);
        }
    }

    fn confirm_selection(&self) -> Option<ProviderSelection> {
        if self.selectable_row_indices.is_empty() {
            return None;
        }
        let row_idx = self.selectable_row_indices[self.selected];
        match &self.rows[row_idx] {
            DisplayRow::Provider(tmpl_idx) => Some(ProviderSelection::Provider(
                self.templates[*tmpl_idx].clone(),
            )),
            DisplayRow::Custom => Some(ProviderSelection::Custom),
            _ => None,
        }
    }

    // ── Key handling ──

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<ProviderSelection> {
        if !self.visible {
            return None;
        }

        match key.code {
            KeyCode::Esc => {
                self.hide();
                None
            }
            KeyCode::Enter => {
                let result = self.confirm_selection();
                if result.is_some() {
                    self.hide();
                }
                result
            }
            KeyCode::Up => {
                self.move_up();
                None
            }
            KeyCode::Down => {
                self.move_down();
                None
            }
            _ => None,
        }
    }

    // ── Mouse handling ──

    /// Convert mouse row to selectable index
    fn selectable_index_at_row(&self, mouse_row: u16, popup_area: &Rect) -> Option<usize> {
        let content_start_y = popup_area.y + 2; // border + title
        if mouse_row < content_start_y {
            return None;
        }
        let visual_offset = (mouse_row - content_start_y) as usize;
        let row_index = self.scroll_offset + visual_offset;
        if row_index >= self.rows.len() {
            return None;
        }
        // Find which selectable index this row corresponds to
        self.selectable_row_indices
            .iter()
            .position(|&ri| ri == row_index)
    }

    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<ProviderSelection> {
        if !self.visible {
            return None;
        }

        let area = match self.last_area {
            Some(a) => a,
            None => return None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
                None
            }
            MouseEventKind::ScrollDown if in_popup => {
                let max_offset = self.rows.len().saturating_sub(self.visible_rows);
                self.scroll_offset = (self.scroll_offset + 3).min(max_offset);
                None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(sel_idx) = self.selectable_index_at_row(mouse.row, &area) {
                    self.selected = sel_idx;
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(sel_idx) = self.selectable_index_at_row(mouse.row, &area) {
                    self.selected = sel_idx;
                    let result = self.confirm_selection();
                    if result.is_some() {
                        self.hide();
                    }
                    return result;
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                None
            }
            _ => None,
        }
    }

    pub fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
        self.visible
    }

    // ── Rendering ──

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        let popup_width = area.width.saturating_sub(4).min(64);
        let max_popup_height = (area.height as f32 * 0.75) as u16;
        let ideal_height = (self.rows.len() as u16 + 4).min(max_popup_height); // +4: border*2, title, hint
        let popup_height = ideal_height.max(8).min(area.height.saturating_sub(2));
        if popup_height < 6 || popup_width < 20 {
            self.last_area = None;
            return;
        }

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        self.last_area = Some(popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Add Model \u{2015} Select Provider ");

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);

        let inner = Rect {
            x: popup_area.x + 1,
            y: popup_area.y + 1,
            width: popup_area.width.saturating_sub(2),
            height: popup_area.height.saturating_sub(2),
        };

        if inner.height < 3 || inner.width < 4 {
            return;
        }

        // Content area (reserve 1 row for hint at bottom)
        let content_height = inner.height.saturating_sub(1) as usize;
        self.visible_rows = content_height;

        // Clamp scroll
        if self.rows.len() <= content_height {
            self.scroll_offset = 0;
        } else {
            let max_offset = self.rows.len() - content_height;
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }

        let visible_end = (self.scroll_offset + content_height).min(self.rows.len());
        for (vi, row_idx) in (self.scroll_offset..visible_end).enumerate() {
            let row = &self.rows[row_idx];
            let row_y = inner.y + vi as u16;
            if row_y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }

            let row_area = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };

            match row {
                DisplayRow::GroupHeader(name) => {
                    let header_line = Line::from(vec![Span::styled(
                        format!("  {}", name),
                        theme.style(StyleKind::Muted).add_modifier(Modifier::BOLD),
                    )]);
                    frame.render_widget(Paragraph::new(header_line), row_area);
                }
                DisplayRow::Provider(tmpl_idx) => {
                    let tmpl = &self.templates[*tmpl_idx];
                    let is_selected = self
                        .selectable_row_indices
                        .get(self.selected)
                        .map_or(false, |&ri| ri == row_idx);

                    self.render_item_row(
                        frame,
                        row_area,
                        &tmpl.name,
                        &tmpl.description,
                        is_selected,
                        theme,
                    );
                }
                DisplayRow::Custom => {
                    let is_selected = self
                        .selectable_row_indices
                        .get(self.selected)
                        .map_or(false, |&ri| ri == row_idx);

                    self.render_item_row(
                        frame,
                        row_area,
                        "Add Custom model",
                        "Configure a custom API endpoint",
                        is_selected,
                        theme,
                    );
                }
            }
        }

        // Hint line
        let hint_y = inner.y + inner.height.saturating_sub(1);
        if hint_y > inner.y {
            let hint_area = Rect {
                x: inner.x,
                y: hint_y,
                width: inner.width,
                height: 1,
            };
            let hint = Paragraph::new(Line::from(Span::styled(
                " \u{2191}\u{2193} Navigate  Enter Select  Esc Cancel",
                theme.style(StyleKind::Muted),
            )));
            frame.render_widget(hint, hint_area);
        }
    }

    fn render_item_row(
        &self,
        frame: &mut Frame,
        row_area: Rect,
        label: &str,
        description: &str,
        is_selected: bool,
        theme: &Theme,
    ) {
        let label_style = if is_selected {
            Style::default()
                .bg(theme.primary)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            theme.style(StyleKind::Primary)
        };

        let desc_style = if is_selected {
            Style::default().bg(theme.primary).fg(Color::White)
        } else {
            theme.style(StyleKind::Muted)
        };

        let bg_style = if is_selected {
            Style::default().bg(theme.primary)
        } else {
            Style::default()
        };

        if is_selected {
            let bg_fill = " ".repeat(row_area.width as usize);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(bg_fill, bg_style))),
                row_area,
            );
        }

        let line = Line::from(vec![
            Span::styled("    ", bg_style),
            Span::styled(label, label_style),
            Span::styled("  ", bg_style),
            Span::styled(description, desc_style),
        ]);
        frame.render_widget(Paragraph::new(line), row_area);
    }
}
