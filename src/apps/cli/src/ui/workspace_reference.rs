use openbitfun_agent_runtime::sdk::{
    AgentWorkspaceReference, AgentWorkspaceReferenceKind, AgentWorkspaceReferenceSearchEntry,
    AgentWorkspaceReferenceSourceRange,
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::theme::{StyleKind, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceReferenceQuery {
    pub(crate) token_start: usize,
    pub(crate) token_end: usize,
    pub(crate) path_query: String,
    pub(crate) start_line: Option<u32>,
    pub(crate) end_line: Option<u32>,
}

pub(crate) fn workspace_reference_query(
    text: &str,
    cursor: usize,
) -> Option<WorkspaceReferenceQuery> {
    let chars = text.chars().collect::<Vec<_>>();
    if cursor > chars.len() {
        return None;
    }
    let token_start = chars[..cursor].iter().rposition(|ch| *ch == '@')?;
    if token_start > 0 && !chars[token_start - 1].is_whitespace() {
        return None;
    }
    if chars[token_start + 1..cursor]
        .iter()
        .any(|ch| ch.is_whitespace())
    {
        return None;
    }
    let raw = chars[token_start + 1..cursor].iter().collect::<String>();
    let (path_query, start_line, end_line) = parse_line_range(&raw);
    Some(WorkspaceReferenceQuery {
        token_start,
        token_end: cursor,
        path_query,
        start_line,
        end_line,
    })
}

fn parse_line_range(raw: &str) -> (String, Option<u32>, Option<u32>) {
    let Some((path, suffix)) = raw.rsplit_once('#') else {
        return (raw.to_string(), None, None);
    };
    let parsed = match suffix.split_once('-') {
        Some((start, end)) => start
            .parse::<u32>()
            .ok()
            .zip(end.parse::<u32>().ok())
            .filter(|(start, end)| *start > 0 && *end >= *start)
            .map(|(start, end)| (Some(start), Some(end))),
        None => suffix
            .parse::<u32>()
            .ok()
            .filter(|start| *start > 0)
            .map(|start| (Some(start), None)),
    };
    match parsed {
        Some((start, end)) => (path.to_string(), start, end),
        None => (path.to_string(), None, None),
    }
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceReferencePopupState {
    pub(crate) query: Option<WorkspaceReferenceQuery>,
    entries: Vec<AgentWorkspaceReferenceSearchEntry>,
    list_state: ListState,
    loading: bool,
}

impl WorkspaceReferencePopupState {
    pub(crate) fn is_visible(&self) -> bool {
        self.query.is_some()
    }

    pub(crate) fn set_query(&mut self, query: Option<WorkspaceReferenceQuery>) {
        let changed = self.query.as_ref().map(|item| &item.path_query)
            != query.as_ref().map(|item| &item.path_query);
        if changed {
            self.entries.clear();
            self.list_state.select(Some(0));
        }
        self.query = query;
        if changed {
            self.loading = self.query.is_some();
        }
    }

    pub(crate) fn set_results(&mut self, entries: Vec<AgentWorkspaceReferenceSearchEntry>) {
        self.entries = entries;
        self.loading = false;
        self.list_state
            .select((!self.entries.is_empty()).then_some(0));
    }

    pub(crate) fn hide(&mut self) {
        self.query = None;
        self.entries.clear();
        self.loading = false;
        self.list_state.select(None);
    }

    pub(crate) fn up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(
            current.checked_sub(1).unwrap_or(self.entries.len() - 1),
        ));
    }

    pub(crate) fn down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((current + 1) % self.entries.len()));
    }

    pub(crate) fn selected(&self) -> Option<AgentWorkspaceReferenceSearchEntry> {
        self.list_state
            .selected()
            .and_then(|index| self.entries.get(index))
            .cloned()
    }

    pub(crate) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.is_visible() {
            return;
        }
        let height = (self.entries.len().max(1) as u16 + 2).min(area.height.min(12));
        let popup = Rect::new(
            area.x.saturating_add(1),
            area.y + area.height.saturating_sub(height + 1),
            area.width.saturating_sub(2),
            height,
        );
        let items = if self.entries.is_empty() {
            vec![ListItem::new(if self.loading {
                "Searching workspace..."
            } else {
                "No matching files"
            })]
        } else {
            self.entries
                .iter()
                .map(|entry| {
                    let icon = if entry.kind == AgentWorkspaceReferenceKind::Directory {
                        "▸"
                    } else {
                        " "
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{icon} "), theme.style(StyleKind::Muted)),
                        Span::raw(entry.path.clone()),
                    ]))
                })
                .collect()
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Workspace files ")
                    .border_style(theme.style(StyleKind::Border)),
            )
            .highlight_style(
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup, &mut self.list_state);
    }
}

pub(crate) fn reference_from_selection(
    query: &WorkspaceReferenceQuery,
    entry: &AgentWorkspaceReferenceSearchEntry,
) -> (String, AgentWorkspaceReference) {
    let (start_line, end_line) = if entry.kind == AgentWorkspaceReferenceKind::File {
        (query.start_line, query.end_line)
    } else {
        (None, None)
    };
    let mut value = format!("@{}", entry.path);
    if let Some(start) = start_line {
        value.push('#');
        value.push_str(&start.to_string());
        if let Some(end) = end_line {
            value.push('-');
            value.push_str(&end.to_string());
        }
    }
    let reference = AgentWorkspaceReference {
        path: entry.path.clone(),
        kind: entry.kind,
        start_line,
        end_line,
        source: AgentWorkspaceReferenceSourceRange {
            start: query.token_start,
            end: query.token_start + value.chars().count(),
            value: value.clone(),
        },
    };
    (value, reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mention_trigger_matches_opencode_start_or_whitespace_rule() {
        assert!(workspace_reference_query("@src", 4).is_some());
        assert!(workspace_reference_query("read @src", 9).is_some());
        assert!(workspace_reference_query("mail@example", 12).is_none());
        assert!(workspace_reference_query("@src file", 9).is_none());
    }

    #[test]
    fn parses_opencode_line_ranges_with_character_offsets() {
        let query = workspace_reference_query("看看 @src/你.rs#2-8", 16).unwrap();
        assert_eq!(query.token_start, 3);
        assert_eq!(query.path_query, "src/你.rs");
        assert_eq!((query.start_line, query.end_line), (Some(2), Some(8)));
    }
}
