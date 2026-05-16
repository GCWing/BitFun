/// Diff rendering for TUI - Unified and Split views
///
/// Uses the `similar` crate to compute line-level diffs and renders them
/// with line numbers, hunk headers, and full-line background colors.

use ratatui::text::{Line, Span};
use similar::{ChangeTag, TextDiff};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::theme::{Theme, StyleKind};

/// Diff view mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffViewMode {
    /// Traditional unified diff with +/- prefixes
    Unified,
    /// Side-by-side split view (old left, new right)
    Split,
    /// Automatically choose based on available width (>120 = Split)
    Auto,
}

/// Compute diff statistics: (additions, deletions)
pub fn diff_stats(old_content: &str, new_content: &str) -> (usize, usize) {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }
    (additions, deletions)
}

/// Render a diff between old and new content.
///
/// Supports Unified and Split view modes with line numbers,
/// hunk headers, and full-line background colors.
pub fn render_diff<'a>(
    old_content: &str,
    new_content: &str,
    theme: &Theme,
    max_lines: usize,
    view_mode: DiffViewMode,
    available_width: u16,
) -> Vec<Line<'a>> {
    let effective_mode = match view_mode {
        DiffViewMode::Auto => {
            if available_width > 120 {
                DiffViewMode::Split
            } else {
                DiffViewMode::Unified
            }
        }
        other => other,
    };

    match effective_mode {
        DiffViewMode::Unified | DiffViewMode::Auto => {
            render_unified(old_content, new_content, theme, max_lines, available_width)
        }
        DiffViewMode::Split => {
            render_split(old_content, new_content, theme, max_lines, available_width)
        }
    }
}

/// Backward-compatible wrapper (used by existing code paths)
#[allow(dead_code)]
pub fn render_unified_diff<'a>(
    old_content: &str,
    new_content: &str,
    theme: &Theme,
    max_lines: usize,
) -> Vec<Line<'a>> {
    render_unified(old_content, new_content, theme, max_lines, 120)
}

// ============ Unified Diff ============

/// Render a unified diff with line numbers and hunk headers.
///
/// Layout per line:
/// ```text
///  old_ln new_ln | +/- content
/// ```
fn render_unified<'a>(
    old_content: &str,
    new_content: &str,
    theme: &Theme,
    max_lines: usize,
    available_width: u16,
) -> Vec<Line<'a>> {
    let diff = TextDiff::from_lines(old_content, new_content);
    let context_size: usize = 2;

    // Collect changes with line numbers
    let mut entries: Vec<DiffEntry> = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;

    for change in diff.iter_all_changes() {
        let tag = change.tag();
        let text = change.value().to_string();
        match tag {
            ChangeTag::Delete => {
                old_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: Some(old_line),
                    new_ln: None,
                });
            }
            ChangeTag::Insert => {
                new_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: None,
                    new_ln: Some(new_line),
                });
            }
            ChangeTag::Equal => {
                old_line += 1;
                new_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: Some(old_line),
                    new_ln: Some(new_line),
                });
            }
        }
    }

    if entries.is_empty() {
        return vec![Line::from(Span::styled(
            "No changes".to_string(),
            theme.style(StyleKind::Muted),
        ))];
    }

    // Find changed indices
    let changed_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.tag != ChangeTag::Equal)
        .map(|(i, _)| i)
        .collect();

    if changed_indices.is_empty() {
        return vec![Line::from(Span::styled(
            "No changes".to_string(),
            theme.style(StyleKind::Muted),
        ))];
    }

    // Build visibility mask (changed lines + context)
    let mut visible = vec![false; entries.len()];
    for &idx in &changed_indices {
        let start = idx.saturating_sub(context_size);
        let end = (idx + context_size + 1).min(entries.len());
        for i in start..end {
            visible[i] = true;
        }
    }

    // Build hunks (groups of visible lines separated by gaps)
    let max_old = old_line;
    let max_new = new_line;
    let num_width = max_old.max(max_new).to_string().len().max(3);

    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut shown = 0;
    let mut last_visible = false;
    let mut hunk_old_start: Option<usize> = None;
    let mut hunk_new_start: Option<usize> = None;

    for (i, entry) in entries.iter().enumerate() {
        if shown >= max_lines {
            lines.push(Line::from(Span::styled(
                format!("\u{2026} ({} more changes)", entries.len() - i),
                theme.style(StyleKind::Muted),
            )));
            break;
        }

        if !visible[i] {
            if last_visible {
                // Reset hunk tracking for next group
                hunk_old_start = None;
                hunk_new_start = None;
            }
            last_visible = false;
            continue;
        }

        // Emit hunk header at the start of each visible group
        if !last_visible {
            // Determine hunk start lines
            let h_old = entry.old_ln.unwrap_or(
                entries[..i]
                    .iter()
                    .rev()
                    .find_map(|e| e.old_ln)
                    .unwrap_or(1),
            );
            let h_new = entry.new_ln.unwrap_or(
                entries[..i]
                    .iter()
                    .rev()
                    .find_map(|e| e.new_ln)
                    .unwrap_or(1),
            );

            if hunk_old_start != Some(h_old) || hunk_new_start != Some(h_new) {
                hunk_old_start = Some(h_old);
                hunk_new_start = Some(h_new);

                // Count lines in this hunk
                let mut hunk_old_count = 0;
                let mut hunk_new_count = 0;
                for j in i..entries.len() {
                    if !visible[j] {
                        break;
                    }
                    match entries[j].tag {
                        ChangeTag::Delete => hunk_old_count += 1,
                        ChangeTag::Insert => hunk_new_count += 1,
                        ChangeTag::Equal => {
                            hunk_old_count += 1;
                            hunk_new_count += 1;
                        }
                    }
                }

                lines.push(Line::from(Span::styled(
                    format!(
                        "@@ -{},{} +{},{} @@",
                        h_old, hunk_old_count, h_new, hunk_new_count
                    ),
                    theme.style(StyleKind::DiffHunkHeader),
                )));
                shown += 1;
            }
        }

        last_visible = true;
        let content = entry.text.trim_end_matches('\n');

        let old_num = entry
            .old_ln
            .map(|n| format!("{:>width$}", n, width = num_width))
            .unwrap_or_else(|| " ".repeat(num_width));
        let new_num = entry
            .new_ln
            .map(|n| format!("{:>width$}", n, width = num_width))
            .unwrap_or_else(|| " ".repeat(num_width));

        let (prefix, line_style) = match entry.tag {
            ChangeTag::Delete => ("-", theme.style(StyleKind::DiffRemoved)),
            ChangeTag::Insert => ("+", theme.style(StyleKind::DiffAdded)),
            ChangeTag::Equal => (" ", theme.style(StyleKind::Muted)),
        };

        let num_style = theme.style(StyleKind::DiffLineNumber);
        let sep_style = theme.style(StyleKind::Muted);
        let content_overhead = 2usize.saturating_mul(num_width).saturating_add(6);
        let content_width = (available_width as usize)
            .saturating_sub(content_overhead)
            .max(1);
        let wrapped = wrap_to_width(content, content_width);

        for (visual_idx, segment) in wrapped.iter().enumerate() {
            if shown >= max_lines {
                lines.push(Line::from(Span::styled(
                    format!("\u{2026} ({} more changes)", entries.len().saturating_sub(i)),
                    theme.style(StyleKind::Muted),
                )));
                return lines;
            }

            let old_num_display = if visual_idx == 0 {
                old_num.clone()
            } else {
                " ".repeat(num_width)
            };
            let new_num_display = if visual_idx == 0 {
                new_num.clone()
            } else {
                " ".repeat(num_width)
            };
            let prefix_display = if visual_idx == 0 {
                format!("{} ", prefix)
            } else {
                "  ".to_string()
            };

            lines.push(Line::from(vec![
                Span::styled(old_num_display, num_style),
                Span::styled(" ", sep_style),
                Span::styled(new_num_display, num_style),
                Span::styled(" \u{2502} ", sep_style),
                Span::styled(prefix_display, line_style),
                Span::styled(segment.clone(), line_style),
            ]));
            shown += 1;
        }
    }

    lines
}
// ============ Split Diff ============

/// Render a side-by-side split diff.
///
/// Layout:
/// ```text
///  old_ln │ - old_content        │ new_ln │ + new_content
/// ```
fn render_split<'a>(
    old_content: &str,
    new_content: &str,
    theme: &Theme,
    max_lines: usize,
    available_width: u16,
) -> Vec<Line<'a>> {
    let diff = TextDiff::from_lines(old_content, new_content);
    let context_size: usize = 2;

    // Collect changes
    let mut entries: Vec<DiffEntry> = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;

    for change in diff.iter_all_changes() {
        let tag = change.tag();
        let text = change.value().to_string();
        match tag {
            ChangeTag::Delete => {
                old_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: Some(old_line),
                    new_ln: None,
                });
            }
            ChangeTag::Insert => {
                new_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: None,
                    new_ln: Some(new_line),
                });
            }
            ChangeTag::Equal => {
                old_line += 1;
                new_line += 1;
                entries.push(DiffEntry {
                    tag,
                    text,
                    old_ln: Some(old_line),
                    new_ln: Some(new_line),
                });
            }
        }
    }

    if entries.is_empty() {
        return vec![Line::from(Span::styled(
            "No changes".to_string(),
            theme.style(StyleKind::Muted),
        ))];
    }

    // Build visibility mask
    let changed_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.tag != ChangeTag::Equal)
        .map(|(i, _)| i)
        .collect();

    if changed_indices.is_empty() {
        return vec![Line::from(Span::styled(
            "No changes".to_string(),
            theme.style(StyleKind::Muted),
        ))];
    }

    let mut visible = vec![false; entries.len()];
    for &idx in &changed_indices {
        let start = idx.saturating_sub(context_size);
        let end = (idx + context_size + 1).min(entries.len());
        for i in start..end {
            visible[i] = true;
        }
    }

    let max_num = old_line.max(new_line);
    let num_width = max_num.to_string().len().max(3);

    // Calculate column widths:
    //   num " │ " prefix " " content " │ " num " │ " prefix " " content
    // prefix is 1 char (+/-/space), followed by 1 space before content
    // Overhead: 2*num_width + 13
    let overhead = (2 * num_width + 13) as u16;
    let content_total = available_width.saturating_sub(overhead);
    let half_width = ((content_total / 2) as usize).max(1);

    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut shown = 0;
    let mut last_visible = false;

    let blank_num = " ".repeat(num_width);
    let empty_col = " ".repeat(half_width);

    // Process entries - pair deletions with insertions for side-by-side
    let mut i = 0;
    while i < entries.len() {
        if shown >= max_lines {
            lines.push(Line::from(Span::styled(
                format!("\u{2026} ({} more changes)", entries.len() - i),
                theme.style(StyleKind::Muted),
            )));
            break;
        }

        if !visible[i] {
            if last_visible {
                // Separator between hunks
                let sep_line = format!(
                    "{} \u{2502} {} \u{2502} {} \u{2502} {}",
                    "\u{2026}".to_string() + &" ".repeat(num_width.saturating_sub(1)),
                    "  ".to_string() + &" ".repeat(half_width),
                    "\u{2026}".to_string() + &" ".repeat(num_width.saturating_sub(1)),
                    "  ".to_string() + &" ".repeat(half_width),
                );
                lines.push(Line::from(Span::styled(
                    sep_line,
                    theme.style(StyleKind::Muted),
                )));
                shown += 1;
            }
            last_visible = false;
            i += 1;
            continue;
        }

        last_visible = true;
        let entry = &entries[i];

        match entry.tag {
            ChangeTag::Equal => {
                let wrapped = wrap_to_width(entry.text.trim_end_matches('\n'), half_width);
                for (row_idx, segment) in wrapped.iter().enumerate() {
                    if shown >= max_lines {
                        lines.push(Line::from(Span::styled(
                            format!("\u{2026} ({} more changes)", entries.len().saturating_sub(i)),
                            theme.style(StyleKind::Muted),
                        )));
                        return lines;
                    }

                    let old_num = if row_idx == 0 {
                        format!("{:>width$}", entry.old_ln.unwrap_or(0), width = num_width)
                    } else {
                        blank_num.clone()
                    };
                    let new_num = if row_idx == 0 {
                        format!("{:>width$}", entry.new_ln.unwrap_or(0), width = num_width)
                    } else {
                        blank_num.clone()
                    };
                    let padded = pad_to_width(segment, half_width);
                    let ctx_style = theme.style(StyleKind::Muted);
                    let num_style = theme.style(StyleKind::DiffLineNumber);
                    let sep_style = theme.style(StyleKind::Muted);

                    lines.push(Line::from(vec![
                        Span::styled(old_num, num_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled("  ", ctx_style),
                        Span::styled(padded.clone(), ctx_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled(new_num, num_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled("  ", ctx_style),
                        Span::styled(padded, ctx_style),
                    ]));
                    shown += 1;
                }
                i += 1;
            }
            ChangeTag::Delete => {
                let has_insert_pair = i + 1 < entries.len()
                    && visible[i + 1]
                    && entries[i + 1].tag == ChangeTag::Insert;

                if has_insert_pair {
                    let next = &entries[i + 1];
                    let old_wrapped = wrap_to_width(entry.text.trim_end_matches('\n'), half_width);
                    let new_wrapped = wrap_to_width(next.text.trim_end_matches('\n'), half_width);
                    let row_count = old_wrapped.len().max(new_wrapped.len());

                    for row_idx in 0..row_count {
                        if shown >= max_lines {
                            lines.push(Line::from(Span::styled(
                                format!("\u{2026} ({} more changes)", entries.len().saturating_sub(i)),
                                theme.style(StyleKind::Muted),
                            )));
                            return lines;
                        }

                        let old_num = if row_idx == 0 {
                            format!("{:>width$}", entry.old_ln.unwrap_or(0), width = num_width)
                        } else {
                            blank_num.clone()
                        };
                        let new_num = if row_idx == 0 {
                            format!("{:>width$}", next.new_ln.unwrap_or(0), width = num_width)
                        } else {
                            blank_num.clone()
                        };

                        let old_piece = old_wrapped.get(row_idx).map(|s| s.as_str()).unwrap_or("");
                        let new_piece = new_wrapped.get(row_idx).map(|s| s.as_str()).unwrap_or("");
                        let left_prefix = if row_idx == 0 { "- " } else { "  " };
                        let right_prefix = if row_idx == 0 { "+ " } else { "  " };

                        let padded_old = pad_to_width(old_piece, half_width);
                        let padded_new = pad_to_width(new_piece, half_width);

                        let num_style = theme.style(StyleKind::DiffLineNumber);
                        let sep_style = theme.style(StyleKind::Muted);
                        let removed_style = theme.style(StyleKind::DiffRemoved);
                        let added_style = theme.style(StyleKind::DiffAdded);

                        lines.push(Line::from(vec![
                            Span::styled(old_num, num_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled(left_prefix, removed_style),
                            Span::styled(padded_old, removed_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled(new_num, num_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled(right_prefix, added_style),
                            Span::styled(padded_new, added_style),
                        ]));
                        shown += 1;
                    }

                    i += 2;
                } else {
                    let old_wrapped = wrap_to_width(entry.text.trim_end_matches('\n'), half_width);
                    for (row_idx, old_piece) in old_wrapped.iter().enumerate() {
                        if shown >= max_lines {
                            lines.push(Line::from(Span::styled(
                                format!("\u{2026} ({} more changes)", entries.len().saturating_sub(i)),
                                theme.style(StyleKind::Muted),
                            )));
                            return lines;
                        }

                        let old_num = if row_idx == 0 {
                            format!("{:>width$}", entry.old_ln.unwrap_or(0), width = num_width)
                        } else {
                            blank_num.clone()
                        };
                        let left_prefix = if row_idx == 0 { "- " } else { "  " };

                        let padded_old = pad_to_width(old_piece, half_width);

                        let num_style = theme.style(StyleKind::DiffLineNumber);
                        let sep_style = theme.style(StyleKind::Muted);
                        let removed_style = theme.style(StyleKind::DiffRemoved);
                        let muted_style = theme.style(StyleKind::Muted);

                        lines.push(Line::from(vec![
                            Span::styled(old_num, num_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled(left_prefix, removed_style),
                            Span::styled(padded_old, removed_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled(blank_num.clone(), num_style),
                            Span::styled(" \u{2502} ", sep_style),
                            Span::styled("  ", sep_style),
                            Span::styled(empty_col.clone(), muted_style),
                        ]));
                        shown += 1;
                    }
                    i += 1;
                }
            }
            ChangeTag::Insert => {
                let new_wrapped = wrap_to_width(entry.text.trim_end_matches('\n'), half_width);
                for (row_idx, new_piece) in new_wrapped.iter().enumerate() {
                    if shown >= max_lines {
                        lines.push(Line::from(Span::styled(
                            format!("\u{2026} ({} more changes)", entries.len().saturating_sub(i)),
                            theme.style(StyleKind::Muted),
                        )));
                        return lines;
                    }

                    let new_num = if row_idx == 0 {
                        format!("{:>width$}", entry.new_ln.unwrap_or(0), width = num_width)
                    } else {
                        blank_num.clone()
                    };
                    let right_prefix = if row_idx == 0 { "+ " } else { "  " };
                    let padded_new = pad_to_width(new_piece, half_width);

                    let num_style = theme.style(StyleKind::DiffLineNumber);
                    let sep_style = theme.style(StyleKind::Muted);
                    let muted_style = theme.style(StyleKind::Muted);
                    let added_style = theme.style(StyleKind::DiffAdded);

                    lines.push(Line::from(vec![
                        Span::styled(blank_num.clone(), num_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled("  ", sep_style),
                        Span::styled(empty_col.clone(), muted_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled(new_num, num_style),
                        Span::styled(" \u{2502} ", sep_style),
                        Span::styled(right_prefix, added_style),
                        Span::styled(padded_new, added_style),
                    ]));
                    shown += 1;
                }
                i += 1;
            }
        }
    }

    lines
}
// ============ Internal Types ============

struct DiffEntry {
    tag: ChangeTag,
    text: String,
    old_ln: Option<usize>,
    new_ln: Option<usize>,
}

// ============ Helpers ============

/// Wrap a string to fit within a given display width (columns).
/// Uses `unicode_width` for correct CJK / multi-byte character handling.
fn wrap_to_width(s: &str, max_width: usize) -> Vec<String> {
    let width_limit = max_width.max(1);
    if s.is_empty() {
        return vec![String::new()];
    }

    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if !current.is_empty() && current_width + ch_width > width_limit {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }

        current.push(ch);
        current_width += ch_width;

        if current_width >= width_limit {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }

    out
}

/// Pad a string with spaces to reach a target display width (columns).
/// Uses `unicode_width` for correct CJK / multi-byte character handling.
fn pad_to_width(s: &str, target_width: usize) -> String {
    let display_width = UnicodeWidthStr::width(s);
    if display_width >= target_width {
        return s.to_string();
    }
    format!("{}{}", s, " ".repeat(target_width - display_width))
}
