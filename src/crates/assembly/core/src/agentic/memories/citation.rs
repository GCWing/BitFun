use serde::{Deserialize, Serialize};

use crate::agentic::core::message::{MemoryCitation, MemoryCitationEntry};
use bitfun_agent_stream::{HiddenTextStreamParser, HiddenTextTag};

const BITFUN_MEMORY_CITATION_OPEN: &str = "<bitfun-mem-citation>";
const BITFUN_MEMORY_CITATION_CLOSE: &str = "</bitfun-mem-citation>";
const CITATION_ENTRIES_OPEN: &str = "<citation_entries>";
const CITATION_ENTRIES_CLOSE: &str = "</citation_entries>";
const ROLLOUT_IDS_OPEN: &str = "<rollout_ids>";
const ROLLOUT_IDS_CLOSE: &str = "</rollout_ids>";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitFunMemoryCitation {
    pub entries: Vec<BitFunMemoryCitationEntry>,
    pub rollout_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitFunMemoryCitationEntry {
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub note: Option<String>,
}

pub fn parse_bitfun_memory_citation(text: &str) -> Option<BitFunMemoryCitation> {
    let (_, payloads) = strip_bitfun_memory_citations(text);
    parse_bitfun_memory_citation_payloads(payloads)
}

pub fn parse_bitfun_memory_citation_payloads<I, S>(payloads: I) -> Option<BitFunMemoryCitation>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut entries = Vec::new();
    let mut rollout_ids = Vec::new();

    for payload in payloads {
        let block = payload.as_ref();
        entries.extend(
            extract_block(block, CITATION_ENTRIES_OPEN, CITATION_ENTRIES_CLOSE)
                .map(parse_citation_entries)
                .unwrap_or_default(),
        );
        if let Some(ids_block) = extract_block(block, ROLLOUT_IDS_OPEN, ROLLOUT_IDS_CLOSE) {
            push_unique_rollout_ids(&mut rollout_ids, ids_block);
        }
    }

    if entries.is_empty() && rollout_ids.is_empty() {
        None
    } else {
        Some(BitFunMemoryCitation {
            entries,
            rollout_ids,
        })
    }
}

pub fn strip_bitfun_memory_citations(text: &str) -> (String, Vec<String>) {
    let mut parser = HiddenTextStreamParser::new(vec![HiddenTextTag::new(
        "memory_citation",
        BITFUN_MEMORY_CITATION_OPEN,
        BITFUN_MEMORY_CITATION_CLOSE,
    )]);
    let mut parsed = parser.push_str(text);
    let tail = parser.finish();
    parsed.visible_text.push_str(&tail.visible_text);
    parsed.hidden_blocks.extend(tail.hidden_blocks);
    let payloads = parsed
        .hidden_blocks
        .into_iter()
        .map(|block| block.payload)
        .collect();
    (parsed.visible_text, payloads)
}

impl From<BitFunMemoryCitation> for MemoryCitation {
    fn from(value: BitFunMemoryCitation) -> Self {
        Self {
            entries: value
                .entries
                .into_iter()
                .map(|entry| MemoryCitationEntry {
                    path: entry.path,
                    line_start: entry.line_start,
                    line_end: entry.line_end,
                    note: entry.note,
                })
                .collect(),
            rollout_ids: value.rollout_ids,
        }
    }
}

fn parse_citation_entries(body: &str) -> Vec<BitFunMemoryCitationEntry> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (path_part, note_part) = line.split_once("|note=[")?;
            let note = note_part.strip_suffix(']')?.trim();
            let (path_with_range, line_range) = path_part.rsplit_once(':')?;
            let (line_start, line_end) = line_range.split_once('-')?;
            Some(BitFunMemoryCitationEntry {
                path: path_with_range.trim().to_string(),
                line_start: line_start.trim().parse().ok()?,
                line_end: line_end.trim().parse().ok()?,
                note: if note.is_empty() {
                    None
                } else {
                    Some(note.to_string())
                },
            })
        })
        .collect()
}

fn push_unique_rollout_ids(ids: &mut Vec<String>, body: &str) {
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !ids.iter().any(|existing| existing == line) {
            ids.push(line.to_string());
        }
    }
}

fn extract_block<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bitfun_memory_citation_extracts_entries_and_rollout_ids() {
        let parsed = parse_bitfun_memory_citation(
            "hello<bitfun-mem-citation><citation_entries>\nMEMORY.md:1-2|note=[x]\nrollout_summaries/foo.md:3-4|note=[y]\n</citation_entries>\n<rollout_ids>\nrollout-1\nrollout-2\nrollout-1\n</rollout_ids></bitfun-mem-citation>world",
        )
        .expect("citation should parse");

        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].path, "MEMORY.md");
        assert_eq!(parsed.entries[0].line_start, 1);
        assert_eq!(parsed.entries[0].line_end, 2);
        assert_eq!(parsed.entries[0].note.as_deref(), Some("x"));
        assert_eq!(parsed.rollout_ids, vec!["rollout-1", "rollout-2"]);
    }

    #[test]
    fn strip_bitfun_memory_citations_removes_citation_blocks() {
        let (visible, payloads) =
            strip_bitfun_memory_citations("a<bitfun-mem-citation>one</bitfun-mem-citation>b");

        assert_eq!(visible, "ab");
        assert_eq!(payloads, vec!["one".to_string()]);
    }

    #[test]
    fn parse_bitfun_memory_citation_payloads_merges_blocks() {
        let parsed = parse_bitfun_memory_citation_payloads(vec![
            "<citation_entries>\nMEMORY.md:1-2|note=[x]\n</citation_entries>\n<rollout_ids>\na\n</rollout_ids>",
            "<citation_entries>\nrollout_summaries/foo.md:3-4|note=[y]\n</citation_entries>\n<rollout_ids>\na\nb\n</rollout_ids>",
        ])
        .expect("citation should parse");

        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[1].path, "rollout_summaries/foo.md");
        assert_eq!(parsed.rollout_ids, vec!["a", "b"]);
    }

    #[test]
    fn strip_bitfun_memory_citations_auto_closes_unterminated_block() {
        let (visible, payloads) = strip_bitfun_memory_citations("a<bitfun-mem-citation>one");

        assert_eq!(visible, "a");
        assert_eq!(payloads, vec!["one".to_string()]);
    }
}
