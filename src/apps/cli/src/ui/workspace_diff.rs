use crossterm::event::{KeyCode, KeyEvent};
use openbitfun_agent_runtime::sdk::{
    WorkspaceDiffContent, WorkspaceDiffFile, WorkspaceDiffFileStatus, WorkspaceDiffSnapshot,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::ui::theme::{StyleKind, Theme};

const WIDE_LAYOUT_MIN_WIDTH: u16 = 84;
const FILE_LIST_WIDTH: u16 = 32;
const PAGE_SCROLL_LINES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceDiffAction {
    None,
    Close,
}

pub(crate) struct WorkspaceDiffViewState {
    snapshot: WorkspaceDiffSnapshot,
    selected_file: usize,
    scroll: usize,
    viewport_height: usize,
    visible: bool,
}

impl WorkspaceDiffViewState {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: WorkspaceDiffSnapshot::default(),
            selected_file: 0,
            scroll: 0,
            viewport_height: 1,
            visible: false,
        }
    }

    pub(crate) fn show(&mut self, snapshot: WorkspaceDiffSnapshot) {
        self.snapshot = snapshot;
        self.selected_file = 0;
        self.scroll = 0;
        self.visible = true;
    }

    pub(crate) fn hide(&mut self) {
        self.visible = false;
    }

    pub(crate) fn reshow(&mut self) {
        self.visible = true;
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    #[cfg(test)]
    fn selected_path(&self) -> Option<&str> {
        self.selected_file().map(|file| file.path.as_str())
    }

    #[cfg(test)]
    fn scroll(&self) -> usize {
        self.scroll
    }

    pub(crate) fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height.max(1);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> WorkspaceDiffAction {
        if !self.visible {
            return WorkspaceDiffAction::None;
        }
        match key.code {
            KeyCode::Esc => {
                self.hide();
                WorkspaceDiffAction::Close
            }
            KeyCode::Char('n') => {
                self.move_file(1);
                WorkspaceDiffAction::None
            }
            KeyCode::Char('p') => {
                self.move_file(-1);
                WorkspaceDiffAction::None
            }
            KeyCode::Char(']') => {
                self.next_hunk();
                WorkspaceDiffAction::None
            }
            KeyCode::Char('[') => {
                self.previous_hunk();
                WorkspaceDiffAction::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                WorkspaceDiffAction::None
            }
            KeyCode::Down => {
                self.scroll_by(1);
                WorkspaceDiffAction::None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(PAGE_SCROLL_LINES);
                WorkspaceDiffAction::None
            }
            KeyCode::PageDown => {
                self.scroll_by(PAGE_SCROLL_LINES);
                WorkspaceDiffAction::None
            }
            KeyCode::Home => {
                self.scroll = 0;
                WorkspaceDiffAction::None
            }
            KeyCode::End => {
                self.scroll = self.max_scroll();
                WorkspaceDiffAction::None
            }
            _ => WorkspaceDiffAction::None,
        }
    }

    pub(crate) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }

        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.background)),
            area,
        );
        let regions = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_header(frame, regions[0], theme);
        if area.width >= WIDE_LAYOUT_MIN_WIDTH {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(FILE_LIST_WIDTH.min(area.width / 3)),
                    Constraint::Min(1),
                ])
                .split(regions[1]);
            self.render_files(frame, body[0], theme);
            self.render_patch(frame, body[1], theme);
        } else {
            self.render_patch(frame, regions[1], theme);
        }
        self.render_footer(frame, regions[2], theme);
    }

    fn selected_file(&self) -> Option<&WorkspaceDiffFile> {
        self.snapshot.files.get(self.selected_file)
    }

    fn move_file(&mut self, delta: isize) {
        if self.snapshot.files.is_empty() {
            return;
        }
        self.selected_file = (self.selected_file as isize + delta)
            .rem_euclid(self.snapshot.files.len() as isize) as usize;
        self.scroll = 0;
    }

    fn next_hunk(&mut self) {
        let hunks = self.hunk_positions();
        if let Some(position) = hunks.iter().find(|position| **position > self.scroll) {
            self.scroll = (*position).min(self.max_scroll());
        }
    }

    fn previous_hunk(&mut self) {
        let hunks = self.hunk_positions();
        let Some(current) = hunks.iter().rposition(|position| *position < self.scroll) else {
            return;
        };
        self.scroll = hunks[current].min(self.max_scroll());
    }

    fn hunk_positions(&self) -> Vec<usize> {
        self.patch_text()
            .map(|patch| {
                patch
                    .lines()
                    .enumerate()
                    .filter_map(|(index, line)| line.starts_with("@@").then_some(index))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn patch_text(&self) -> Option<&str> {
        match &self.selected_file()?.content {
            WorkspaceDiffContent::Text { patch } => Some(patch),
            WorkspaceDiffContent::Binary | WorkspaceDiffContent::TooLarge => None,
        }
    }

    fn content_line_count(&self) -> usize {
        self.patch_text()
            .map(|patch| patch.lines().count())
            .unwrap_or(1)
    }

    fn max_scroll(&self) -> usize {
        self.content_line_count()
            .saturating_sub(self.viewport_height)
    }

    fn scroll_by(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_add(amount).min(self.max_scroll());
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let additions = self
            .snapshot
            .files
            .iter()
            .map(|file| file.additions)
            .sum::<usize>();
        let deletions = self
            .snapshot
            .files
            .iter()
            .map(|file| file.deletions)
            .sum::<usize>();
        let mut title = vec![Span::styled(
            " Workspace Diff ",
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
        )];
        if self.snapshot.truncated {
            title.push(Span::styled(
                "[truncated] ",
                theme.style(StyleKind::Warning),
            ));
        }
        title.push(Span::styled(
            format!(
                "{} files  +{} -{}",
                self.snapshot.files.len(),
                additions,
                deletions
            ),
            theme.style(StyleKind::Muted),
        ));
        frame.render_widget(
            Paragraph::new(Line::from(title)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style(StyleKind::Primary))
                    .style(Style::default().bg(theme.background)),
            ),
            area,
        );
    }

    fn render_files(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let items = self.snapshot.files.iter().map(|file| {
            let status = match file.status {
                WorkspaceDiffFileStatus::Added => "A",
                WorkspaceDiffFileStatus::Modified => "M",
                WorkspaceDiffFileStatus::Deleted => "D",
                WorkspaceDiffFileStatus::Renamed => "R",
                WorkspaceDiffFileStatus::Conflicted => "U",
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{status} "), theme.style(StyleKind::Muted)),
                Span::raw(file.path.clone()),
            ]))
        });
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Files ")
                    .style(Style::default().bg(theme.background)),
            )
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .bg(theme.background_element)
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            );
        let mut state = ListState::default();
        if !self.snapshot.files.is_empty() {
            state.select(Some(self.selected_file));
        }
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_patch(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.set_viewport_height(area.height.saturating_sub(2) as usize);
        let title = self
            .selected_file()
            .map(file_title)
            .unwrap_or_else(|| " No workspace changes ".to_string());
        let lines = match self.selected_file() {
            Some(file) if file.status == WorkspaceDiffFileStatus::Conflicted => {
                vec![Line::styled(
                    "Conflict content is not shown as a regular two-way diff.",
                    theme.style(StyleKind::Warning),
                )]
            }
            Some(WorkspaceDiffFile {
                content: WorkspaceDiffContent::Text { patch },
                ..
            }) if patch.is_empty() => vec![Line::styled(
                "Staged and worktree changes have no net patch.",
                theme.style(StyleKind::Muted),
            )],
            Some(WorkspaceDiffFile {
                content: WorkspaceDiffContent::Text { patch },
                ..
            }) => patch
                .lines()
                .skip(self.scroll)
                .take(self.viewport_height)
                .map(|line| styled_patch_line(line, theme))
                .collect::<Vec<_>>(),
            Some(WorkspaceDiffFile {
                content: WorkspaceDiffContent::Binary,
                ..
            }) => vec![Line::styled(
                "Binary file changed; textual diff is unavailable.",
                theme.style(StyleKind::Muted),
            )],
            Some(WorkspaceDiffFile {
                content: WorkspaceDiffContent::TooLarge,
                ..
            }) => vec![Line::styled(
                "Diff omitted because it exceeds the workspace diff size limit.",
                theme.style(StyleKind::Warning),
            )],
            None => vec![Line::styled(
                "Working tree is clean.",
                theme.style(StyleKind::Muted),
            )],
        };
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .style(Style::default().bg(theme.background)),
            ),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let hint = " n/p: files  [ / ]: hunks  Up/Down/Page: scroll  Esc: close ";
        frame.render_widget(
            Paragraph::new(hint).style(theme.style(StyleKind::Muted)),
            area,
        );
    }
}

fn file_title(file: &WorkspaceDiffFile) -> String {
    let path = file
        .old_path
        .as_ref()
        .map(|old_path| format!("{old_path} -> {}", file.path))
        .unwrap_or_else(|| file.path.clone());
    let change_source = [
        file.staged.then_some("staged"),
        file.unstaged.then_some("unstaged"),
        file.untracked.then_some("untracked"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let change_source = if change_source.is_empty() {
        "workspace".to_string()
    } else {
        change_source.join("+")
    };
    format!(
        " {path}  {change_source}  +{} -{} ",
        file.additions, file.deletions
    )
}

fn styled_patch_line(line: &str, theme: &Theme) -> Line<'static> {
    let line = expand_tabs(line);
    let style = if line.starts_with("@@") {
        Style::default().fg(theme.diff_hunk_header)
    } else if line.starts_with("+++") || line.starts_with("---") {
        theme.style(StyleKind::Muted)
    } else if line.starts_with('+') {
        Style::default()
            .fg(theme.diff_added_fg)
            .bg(theme.diff_added_bg)
    } else if line.starts_with('-') {
        Style::default()
            .fg(theme.diff_removed_fg)
            .bg(theme.diff_removed_bg)
    } else {
        Style::default()
    };
    Line::styled(line, style)
}

fn expand_tabs(line: &str) -> String {
    const TAB_STOP: usize = 4;

    let mut expanded = String::with_capacity(line.len());
    let mut column = 0usize;
    for character in line.chars() {
        if character == '\t' {
            let spaces = TAB_STOP - column % TAB_STOP;
            expanded.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            expanded.push(character);
            column += character.width().unwrap_or(0);
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use openbitfun_agent_runtime::sdk::{
        WorkspaceDiffContent, WorkspaceDiffFile, WorkspaceDiffFileStatus, WorkspaceDiffSnapshot,
    };
    use ratatui::{backend::TestBackend, Terminal};

    use super::{WorkspaceDiffAction, WorkspaceDiffViewState};
    use crate::ui::theme::Theme;

    fn file(path: &str, patch: &str) -> WorkspaceDiffFile {
        WorkspaceDiffFile {
            path: path.to_string(),
            old_path: None,
            status: WorkspaceDiffFileStatus::Modified,
            staged: false,
            unstaged: true,
            untracked: false,
            additions: 2,
            deletions: 1,
            content: WorkspaceDiffContent::Text {
                patch: patch.to_string(),
            },
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn opencode_navigation_moves_between_files_and_hunks() {
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![
                file("a.rs", "@@ -1 +1 @@\n-a\n+b\n"),
                file(
                    "b.rs",
                    "header\n@@ -1 +1 @@\n-old\n+new\n@@ -10 +10 @@\n-old2\n+new2\n",
                ),
            ],
            truncated: false,
        });

        assert_eq!(view.selected_path(), Some("a.rs"));
        assert_eq!(
            view.handle_key_event(key(KeyCode::Char('n'))),
            WorkspaceDiffAction::None
        );
        assert_eq!(view.selected_path(), Some("b.rs"));
        assert_eq!(view.scroll(), 0);
        view.handle_key_event(key(KeyCode::Char(']')));
        assert_eq!(view.scroll(), 1);
        view.handle_key_event(key(KeyCode::Char(']')));
        assert_eq!(view.scroll(), 4);
        view.handle_key_event(key(KeyCode::Char('[')));
        assert_eq!(view.scroll(), 1);
        view.handle_key_event(key(KeyCode::Char('p')));
        assert_eq!(view.selected_path(), Some("a.rs"));
    }

    #[test]
    fn scrolling_is_bounded_and_escape_closes_the_viewer() {
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![file("long.rs", &"line\n".repeat(30))],
            truncated: false,
        });
        view.set_viewport_height(10);

        view.handle_key_event(key(KeyCode::End));
        assert_eq!(view.scroll(), 20);
        view.handle_key_event(key(KeyCode::Down));
        assert_eq!(view.scroll(), 20);
        view.handle_key_event(key(KeyCode::Home));
        assert_eq!(view.scroll(), 0);
        assert_eq!(
            view.handle_key_event(key(KeyCode::Esc)),
            WorkspaceDiffAction::Close
        );
        assert!(!view.is_visible());
    }

    #[test]
    fn responsive_render_keeps_the_patch_and_collapses_only_the_file_list() {
        let snapshot = WorkspaceDiffSnapshot {
            files: vec![
                file("src/a.rs", "@@ -1 +1 @@\n-old\n+new\n"),
                file("src/b.rs", "@@ -1 +1 @@\n-before\n+after\n"),
            ],
            truncated: false,
        };
        let mut wide = WorkspaceDiffViewState::new();
        wide.show(snapshot.clone());
        let mut wide_terminal = Terminal::new(TestBackend::new(100, 18)).expect("wide terminal");
        wide_terminal
            .draw(|frame| wide.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render wide diff");
        let wide_text = rendered_text(&wide_terminal);
        assert!(wide_text.contains("src/b.rs"));
        assert!(wide_text.contains("+new"));

        let mut narrow = WorkspaceDiffViewState::new();
        narrow.show(snapshot);
        let mut narrow_terminal = Terminal::new(TestBackend::new(60, 18)).expect("narrow terminal");
        narrow_terminal
            .draw(|frame| narrow.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render narrow diff");
        let narrow_text = rendered_text(&narrow_terminal);
        assert!(!narrow_text.contains("src/b.rs"));
        assert!(narrow_text.contains("+new"));
        assert!(narrow_text.contains("unstaged"));
    }

    #[test]
    fn narrow_render_keeps_the_truncated_badge_visible() {
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![file("src/a.rs", "@@ -1 +1 @@\n-old\n+new\n")],
            truncated: true,
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 10)).expect("narrow terminal");

        terminal
            .draw(|frame| view.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render truncated diff");

        assert!(rendered_text(&terminal).contains("truncated"));
    }

    #[test]
    fn render_can_reach_patch_lines_beyond_u16_scroll_range() {
        let patch = (0..70_000)
            .map(|index| format!("line {index}\n"))
            .collect::<String>();
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![file("many-lines.rs", &patch)],
            truncated: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).expect("terminal");

        terminal
            .draw(|frame| view.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("prime viewport size");
        view.handle_key_event(key(KeyCode::End));

        terminal
            .draw(|frame| view.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render long diff");

        assert!(rendered_text(&terminal).contains("line 69999"));
    }

    #[test]
    fn render_preserves_tab_indentation_with_terminal_safe_spaces() {
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![file("tabs.rs", "+\tindented\n")],
            truncated: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).expect("terminal");

        terminal
            .draw(|frame| view.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render tabbed diff");

        assert!(rendered_text(&terminal).contains("+   indented"));
    }

    #[test]
    fn render_explains_conflicts_and_combines_change_sources() {
        let mut conflict = file("conflict.rs", "");
        conflict.status = WorkspaceDiffFileStatus::Conflicted;
        conflict.staged = true;
        conflict.unstaged = true;
        conflict.untracked = true;
        let mut view = WorkspaceDiffViewState::new();
        view.show(WorkspaceDiffSnapshot {
            files: vec![conflict],
            truncated: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(72, 8)).expect("terminal");

        terminal
            .draw(|frame| view.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render conflict");
        let text = rendered_text(&terminal);

        assert!(text.contains("staged+unstaged+untracked"));
        assert!(text.contains("Conflict content is not shown as a regular two-way diff."));
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
