/// Command palette popup (Ctrl+P)
///
/// A centered overlay with a search box at the top and grouped command items below.
/// Unlike the slash command menu which appears inline, this is a full-screen centered popup.
/// Supports viewport scrolling when content exceeds visible area.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};

// ── Data types ──

/// A single item in the command palette
#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub group: String,
}

/// Action returned after handling a key event
pub enum PaletteAction {
    /// User confirmed selection — carries the item id
    Execute(String),
    /// User dismissed the palette (Esc)
    Dismiss,
    /// Key was consumed but no actionable result
    None,
}

// ── Default palette items ──

/// Build the default set of palette items (all groups)
pub fn default_palette_items() -> Vec<PaletteItem> {
    vec![
        // Session group
        PaletteItem {
            id: "new_session".into(),
            label: "New session".into(),
            description: "Start a new conversation".into(),
            group: "Session".into(),
        },
        PaletteItem {
            id: "sessions".into(),
            label: "Sessions".into(),
            description: "Browse and switch sessions".into(),
            group: "Session".into(),
        },
        // Prompt group
        PaletteItem {
            id: "skills".into(),
            label: "Skills".into(),
            description: "Browse and select available skills".into(),
            group: "Prompt".into(),
        },
        PaletteItem {
            id: "subagents".into(),
            label: "Subagents".into(),
            description: "Browse and launch subagents".into(),
            group: "Prompt".into(),
        },
        // Models group
        PaletteItem {
            id: "select_model".into(),
            label: "Select model".into(),
            description: "Select AI model for all modes".into(),
            group: "Models".into(),
        },
        PaletteItem {
            id: "add_model".into(),
            label: "Add model".into(),
            description: "Add a new AI model configuration".into(),
            group: "Models".into(),
        },
        // Agent group
        PaletteItem {
            id: "switch_agent".into(),
            label: "Switch agent".into(),
            description: "Switch agent mode".into(),
            group: "Agent".into(),
        },
        // MCP group
        PaletteItem {
            id: "mcp_servers".into(),
            label: "MCP servers".into(),
            description: "Manage MCP servers".into(),
            group: "MCP".into(),
        },
        // System group
        PaletteItem {
            id: "help".into(),
            label: "Help".into(),
            description: "Show help information".into(),
            group: "System".into(),
        },
        PaletteItem {
            id: "exit".into(),
            label: "Exit the app".into(),
            description: "Quit the application".into(),
            group: "System".into(),
        },
    ]
}

/// Build suggested items
fn build_suggested_items() -> Vec<PaletteItem> {
    vec![
        PaletteItem {
            id: "select_model".into(),
            label: "Select model".into(),
            description: "Select AI model for all modes".into(),
            group: "Suggested".into(),
        },
        PaletteItem {
            id: "switch_agent".into(),
            label: "Switch agent".into(),
            description: "Switch agent mode".into(),
            group: "Suggested".into(),
        },
        PaletteItem {
            id: "new_session".into(),
            label: "New session".into(),
            description: "Start a new conversation".into(),
            group: "Suggested".into(),
        },
    ]
}

// ── Flattened row for rendering ──

#[derive(Debug, Clone)]
enum PaletteRow {
    /// Group header row
    GroupHeader(String),
    /// Selectable item row — index into `selectable_items`
    Item(usize),
}

// ── State ──

pub struct CommandPaletteState {
    visible: bool,
    search_input: String,
    search_cursor: usize,

    /// All items (suggested + regular groups)
    all_items: Vec<PaletteItem>,

    /// Rows after filtering (headers + items)
    rows: Vec<PaletteRow>,
    /// Flat list of selectable item indices (into `all_items`) in display order
    selectable_items: Vec<usize>,
    /// Currently highlighted selectable index (index into `selectable_items`)
    selected_index: usize,

    /// Viewport scroll offset (first visible row index in `rows`)
    scroll_offset: usize,
    /// Number of visible content rows (set each render frame)
    visible_rows: usize,

    last_area: Option<Rect>,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        Self {
            visible: false,
            search_input: String::new(),
            search_cursor: 0,
            all_items: Vec::new(),
            rows: Vec::new(),
            selectable_items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            visible_rows: 0,
            last_area: None,
        }
    }

    /// Show the command palette with the default items
    pub fn show(&mut self) {
        let mut items = build_suggested_items();
        items.extend(default_palette_items());
        self.all_items = items;
        self.search_input.clear();
        self.search_cursor = 0;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.visible = true;
        self.rebuild_filtered();
    }

    /// Hide the command palette
    pub fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear data here to support back navigation
        self.last_area = None;
    }

    /// Reshow the command palette (for back navigation)
    pub fn reshow(&mut self) {
        self.visible = true;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // ── Filtering ──

    fn rebuild_filtered(&mut self) {
        let query = self.search_input.to_lowercase();
        self.rows.clear();
        self.selectable_items.clear();

        let mut groups: Vec<String> = Vec::new();
        for item in &self.all_items {
            if !groups.contains(&item.group) {
                groups.push(item.group.clone());
            }
        }

        for group in &groups {
            let mut group_item_indices: Vec<usize> = Vec::new();
            for (idx, item) in self.all_items.iter().enumerate() {
                if &item.group != group {
                    continue;
                }
                if !query.is_empty() {
                    let matches = item.label.to_lowercase().contains(&query)
                        || item.description.to_lowercase().contains(&query);
                    if !matches {
                        continue;
                    }
                }
                group_item_indices.push(idx);
            }

            if group_item_indices.is_empty() {
                continue;
            }

            self.rows.push(PaletteRow::GroupHeader(group.clone()));

            for idx in group_item_indices {
                let selectable_idx = self.selectable_items.len();
                self.selectable_items.push(idx);
                self.rows.push(PaletteRow::Item(selectable_idx));
            }
        }

        // Clamp selected index
        if self.selectable_items.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.selectable_items.len() - 1);
        }

        // Reset scroll and ensure selected item is visible
        self.scroll_offset = 0;
        self.ensure_selected_visible();
    }

    // ── Scrolling ──

    /// Scroll viewport up by `n` rows
    fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll viewport down by `n` rows
    fn scroll_down(&mut self, n: usize) {
        let max_offset = self.rows.len().saturating_sub(self.visible_rows);
        self.scroll_offset = (self.scroll_offset + n).min(max_offset);
    }

    /// Ensure the currently selected item's row is within the visible viewport
    fn ensure_selected_visible(&mut self) {
        if self.selectable_items.is_empty() || self.visible_rows == 0 {
            return;
        }
        // Find the row index of the selected item
        if let Some(row_idx) = self.row_index_of_selected() {
            if row_idx < self.scroll_offset {
                // Selected is above viewport — scroll up to show it
                // Also try to show the group header above it if possible
                self.scroll_offset = row_idx.saturating_sub(1);
            } else if row_idx >= self.scroll_offset + self.visible_rows {
                // Selected is below viewport — scroll down
                self.scroll_offset = row_idx.saturating_sub(self.visible_rows - 1);
            }
        }
    }

    /// Find the row index (in `self.rows`) that corresponds to the currently selected item
    fn row_index_of_selected(&self) -> Option<usize> {
        for (i, row) in self.rows.iter().enumerate() {
            if let PaletteRow::Item(sel_idx) = row {
                if *sel_idx == self.selected_index {
                    return Some(i);
                }
            }
        }
        None
    }

    // ── Navigation ──

    fn move_up(&mut self) {
        if self.selectable_items.is_empty() {
            return;
        }
        self.selected_index = self.selected_index.saturating_sub(1);
        self.ensure_selected_visible();
    }

    fn move_down(&mut self) {
        if self.selectable_items.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.selectable_items.len() - 1);
        self.ensure_selected_visible();
    }

    fn confirm_selection(&self) -> Option<String> {
        if self.selectable_items.is_empty() {
            return None;
        }
        let item_idx = self.selectable_items[self.selected_index];
        Some(self.all_items[item_idx].id.clone())
    }

    // ── Key handling ──

    pub fn handle_key_event(&mut self, key: KeyEvent) -> PaletteAction {
        if !self.visible {
            return PaletteAction::None;
        }

        if key.kind != KeyEventKind::Press {
            return PaletteAction::None;
        }

        match key.code {
            KeyCode::Esc => {
                self.hide();
                PaletteAction::Dismiss
            }
            KeyCode::Enter => {
                if let Some(id) = self.confirm_selection() {
                    // Don't hide here to support back navigation
                    // The caller will handle hiding if needed
                    PaletteAction::Execute(id)
                } else {
                    PaletteAction::None
                }
            }
            KeyCode::Up => {
                self.move_up();
                PaletteAction::None
            }
            KeyCode::Down => {
                self.move_down();
                PaletteAction::None
            }
            KeyCode::PageUp => {
                self.scroll_up(self.visible_rows.max(1));
                PaletteAction::None
            }
            KeyCode::PageDown => {
                self.scroll_down(self.visible_rows.max(1));
                PaletteAction::None
            }
            KeyCode::Backspace => {
                if self.search_cursor > 0 {
                    let byte_start = self.char_to_byte(self.search_cursor - 1);
                    let byte_end = self.char_to_byte(self.search_cursor);
                    self.search_input.drain(byte_start..byte_end);
                    self.search_cursor -= 1;
                    self.rebuild_filtered();
                }
                PaletteAction::None
            }
            KeyCode::Char(c) => {
                let byte_pos = self.char_to_byte(self.search_cursor);
                self.search_input.insert(byte_pos, c);
                self.search_cursor += 1;
                self.rebuild_filtered();
                PaletteAction::None
            }
            KeyCode::Left => {
                if self.search_cursor > 0 {
                    self.search_cursor -= 1;
                }
                PaletteAction::None
            }
            KeyCode::Right => {
                let char_count = self.search_input.chars().count();
                if self.search_cursor < char_count {
                    self.search_cursor += 1;
                }
                PaletteAction::None
            }
            KeyCode::Home => {
                self.search_cursor = 0;
                PaletteAction::None
            }
            KeyCode::End => {
                self.search_cursor = self.search_input.chars().count();
                PaletteAction::None
            }
            _ => PaletteAction::None,
        }
    }

    // ── Mouse handling ──

    /// Content rows start at popup_area.y + 3 (border + search + separator)
    fn content_start_y(popup_area: &Rect) -> u16 {
        popup_area.y + 3
    }

    /// Convert a mouse row to the selectable index it corresponds to, accounting for scroll offset
    fn selectable_index_at_row(&self, row: u16, popup_area: &Rect) -> Option<usize> {
        let start = Self::content_start_y(popup_area);
        if row < start {
            return None;
        }
        let visual_offset = (row - start) as usize;
        let row_index = self.scroll_offset + visual_offset;
        if row_index >= self.rows.len() {
            return None;
        }
        if let PaletteRow::Item(sel_idx) = &self.rows[row_index] {
            Some(*sel_idx)
        } else {
            None
        }
    }

    pub fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> PaletteAction {
        if !self.visible {
            return PaletteAction::None;
        }

        let area = match self.last_area {
            Some(a) => a,
            None => return PaletteAction::None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            // Scroll wheel — scroll the viewport
            MouseEventKind::ScrollUp if in_popup => {
                self.scroll_up(3);
                PaletteAction::None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.scroll_down(3);
                PaletteAction::None
            }
            // Hover — highlight the item under the cursor
            MouseEventKind::Moved if in_popup => {
                if let Some(sel_idx) = self.selectable_index_at_row(mouse.row, &area) {
                    self.selected_index = sel_idx;
                }
                PaletteAction::None
            }
            // Click — select and execute
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(sel_idx) = self.selectable_index_at_row(mouse.row, &area) {
                    self.selected_index = sel_idx;
                    if let Some(id) = self.confirm_selection() {
                        self.hide();
                        return PaletteAction::Execute(id);
                    }
                }
                PaletteAction::None
            }
            // Click outside popup — dismiss
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                PaletteAction::Dismiss
            }
            _ => PaletteAction::None,
        }
    }

    /// Whether the palette captures mouse events (prevents passthrough)
    pub fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
        self.visible
    }

    // ── Rendering ──

    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            self.last_area = None;
            return;
        }

        // Calculate popup dimensions
        let popup_width = area.width.saturating_sub(4).min(60);
        // Fixed height: use up to 70% of terminal height, with reasonable bounds
        let max_popup_height = (area.height as f32 * 0.7) as u16;
        // Content: 1 search + 1 separator + rows + 1 hint + 2 borders
        let ideal_height = (self.rows.len() as u16 + 5).min(max_popup_height);
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

        // Draw background + border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(" Command Palette ");

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);

        // Inner area (inside border)
        let inner = Rect {
            x: popup_area.x + 1,
            y: popup_area.y + 1,
            width: popup_area.width.saturating_sub(2),
            height: popup_area.height.saturating_sub(2),
        };

        if inner.height < 4 || inner.width < 4 {
            return;
        }

        // Row 0: Search box
        let search_display = if self.search_input.is_empty() {
            Line::from(vec![
                Span::styled("> ", theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)),
                Span::styled("Search commands...", theme.style(StyleKind::Muted)),
            ])
        } else {
            Line::from(vec![
                Span::styled("> ", theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)),
                Span::raw(&self.search_input),
            ])
        };
        let search_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(search_display), search_area);

        // Set cursor position in search box
        let cursor_x = inner.x + 2 + self.search_input[..self.char_to_byte(self.search_cursor)]
            .chars()
            .count() as u16;
        frame.set_cursor_position((cursor_x, inner.y));

        // Separator line
        let sep_y = inner.y + 1;
        if sep_y < inner.y + inner.height {
            let sep = "\u{2500}".repeat(inner.width as usize);
            let sep_area = Rect {
                x: inner.x,
                y: sep_y,
                width: inner.width,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(sep, theme.style(StyleKind::Border)))),
                sep_area,
            );
        }

        // Content area: between separator and hint line
        // inner.height = border_inner_h, used: search(1) + sep(1) + hint(1) = 3
        let content_start_y = inner.y + 2;
        let max_content_rows = inner.height.saturating_sub(3) as usize;

        // Update visible_rows for scroll calculations
        self.visible_rows = max_content_rows;

        // Clamp scroll offset
        if self.rows.len() <= max_content_rows {
            self.scroll_offset = 0;
        } else {
            let max_offset = self.rows.len() - max_content_rows;
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }

        // Render visible rows from scroll_offset
        let visible_end = (self.scroll_offset + max_content_rows).min(self.rows.len());
        for (vi, row_idx) in (self.scroll_offset..visible_end).enumerate() {
            let row = &self.rows[row_idx];
            let row_y = content_start_y + vi as u16;
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
                PaletteRow::GroupHeader(name) => {
                    let header_line = Line::from(vec![
                        Span::styled(
                            format!("  {}", name.to_uppercase()),
                            theme.style(StyleKind::Muted).add_modifier(Modifier::BOLD),
                        ),
                    ]);
                    frame.render_widget(Paragraph::new(header_line), row_area);
                }
                PaletteRow::Item(sel_idx) => {
                    let item_idx = self.selectable_items[*sel_idx];
                    let item = &self.all_items[item_idx];
                    let is_selected = *sel_idx == self.selected_index;

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

                    // Fill background for selected row
                    if is_selected {
                        let bg_fill = " ".repeat(inner.width as usize);
                        frame.render_widget(
                            Paragraph::new(Line::from(Span::styled(bg_fill, bg_style))),
                            row_area,
                        );
                    }

                    let line = Line::from(vec![
                        Span::styled("    ", bg_style),
                        Span::styled(&item.label, label_style),
                        Span::styled("  ", bg_style),
                        Span::styled(&item.description, desc_style),
                    ]);
                    frame.render_widget(Paragraph::new(line), row_area);
                }
            }
        }

        // Scroll indicator (show when content overflows)
        let has_more_above = self.scroll_offset > 0;
        let has_more_below = visible_end < self.rows.len();

        // Bottom hint line
        let hint_y = inner.y + inner.height.saturating_sub(1);
        if hint_y > content_start_y {
            let hint_area = Rect {
                x: inner.x,
                y: hint_y,
                width: inner.width,
                height: 1,
            };

            let mut hints = vec![
                Span::styled(
                    " \u{2191}\u{2193} Navigate  Enter Select  Esc Cancel",
                    theme.style(StyleKind::Muted),
                ),
            ];

            if has_more_above || has_more_below {
                let indicator = if has_more_above && has_more_below {
                    " \u{2195}"
                } else if has_more_above {
                    " \u{2191}"
                } else {
                    " \u{2193}"
                };
                hints.push(Span::styled(
                    indicator,
                    theme.style(StyleKind::Warning),
                ));
            }

            let hint_line = Line::from(hints);
            frame.render_widget(Paragraph::new(hint_line), hint_area);
        }
    }

    // ── Helpers ──

    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.search_input
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.search_input.len())
    }
}
