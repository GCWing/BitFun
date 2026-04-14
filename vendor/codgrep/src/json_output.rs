use std::{io::Write, time::Duration};

use crate::error::Result;
use crate::search::{SearchHit, SearchLine, SearchResults};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonMessage {
    Begin(JsonBegin),
    End(JsonEnd),
    Match(JsonMatch),
    Context(JsonContext),
    Summary(JsonSummary),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonBegin {
    pub path: Option<JsonData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonEnd {
    pub path: Option<JsonData>,
    pub binary_offset: Option<u64>,
    pub stats: JsonStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSummary {
    pub elapsed_total: JsonDuration,
    pub stats: JsonStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonMatch {
    pub path: Option<JsonData>,
    pub lines: JsonData,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<JsonSubmatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonContext {
    pub path: Option<JsonData>,
    pub lines: JsonData,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<JsonSubmatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSubmatch {
    pub m: JsonData,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonData {
    Text { text: String },
    Bytes { bytes: String },
}

impl JsonData {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::Text {
                text: text.to_string(),
            },
            Err(_) => Self::Bytes {
                bytes: base64_standard(bytes),
            },
        }
    }

    pub fn from_string(text: String) -> Self {
        Self::Text { text }
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Self::Text { text } => {
                out.push_str("{\"text\":");
                push_json_string(out, text);
                out.push('}');
            }
            Self::Bytes { bytes } => {
                out.push_str("{\"bytes\":");
                push_json_string(out, bytes);
                out.push('}');
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDuration {
    pub secs: u64,
    pub nanos: u32,
    pub human: String,
}

impl JsonDuration {
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            secs: duration.as_secs(),
            nanos: duration.subsec_nanos(),
            human: format!("{:.6}s", duration.as_secs_f64()),
        }
    }

    pub fn zero() -> Self {
        Self::from_duration(Duration::ZERO)
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"secs\":");
        out.push_str(&self.secs.to_string());
        out.push_str(",\"nanos\":");
        out.push_str(&self.nanos.to_string());
        out.push_str(",\"human\":");
        push_json_string(out, &self.human);
        out.push('}');
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonStats {
    pub elapsed: JsonDuration,
    pub searches: u64,
    pub searches_with_match: u64,
    pub bytes_searched: u64,
    pub bytes_printed: u64,
    pub matched_lines: u64,
    pub matches: u64,
}

impl JsonStats {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"elapsed\":");
        self.elapsed.write_json(out);
        out.push_str(",\"searches\":");
        out.push_str(&self.searches.to_string());
        out.push_str(",\"searches_with_match\":");
        out.push_str(&self.searches_with_match.to_string());
        out.push_str(",\"bytes_searched\":");
        out.push_str(&self.bytes_searched.to_string());
        out.push_str(",\"bytes_printed\":");
        out.push_str(&self.bytes_printed.to_string());
        out.push_str(",\"matched_lines\":");
        out.push_str(&self.matched_lines.to_string());
        out.push_str(",\"matches\":");
        out.push_str(&self.matches.to_string());
        out.push('}');
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSearchFile {
    pub path: JsonData,
    pub messages: Vec<JsonMessage>,
    pub binary_offset: Option<u64>,
    pub stats: JsonStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSearchReport {
    pub files: Vec<JsonSearchFile>,
    pub summary: JsonStats,
    pub elapsed_total: JsonDuration,
}

impl JsonSearchReport {
    /// Converts structured search results into a JSON report shape.
    ///
    /// This projection preserves aggregate statistics and line-level snippets,
    /// but does not retain byte-accurate absolute offsets.
    pub fn from_search_results(results: &SearchResults) -> Self {
        let files = results
            .hits
            .iter()
            .map(json_file_from_search_hit)
            .collect::<Vec<_>>();
        Self {
            files,
            summary: JsonStats {
                elapsed: JsonDuration::zero(),
                searches: results.candidate_docs as u64,
                searches_with_match: results.searches_with_match as u64,
                bytes_searched: results.bytes_searched,
                bytes_printed: 0,
                matched_lines: results.matched_lines as u64,
                matches: results.matched_occurrences as u64,
            },
            elapsed_total: JsonDuration::zero(),
        }
    }

    pub fn has_match(&self) -> bool {
        self.summary.searches_with_match > 0
    }

    pub fn messages(&self) -> Vec<JsonMessage> {
        let mut total_bytes_printed = 0u64;
        let mut messages = Vec::new();

        for file in &self.files {
            let mut file_bytes_printed = 0u64;
            let begin = JsonMessage::Begin(JsonBegin {
                path: Some(file.path.clone()),
            });
            let begin_len = message_len(&begin);
            file_bytes_printed += begin_len;
            total_bytes_printed += begin_len;
            messages.push(begin);

            for message in &file.messages {
                let message = message.clone();
                let message_len = message_len(&message);
                file_bytes_printed += message_len;
                total_bytes_printed += message_len;
                messages.push(message);
            }

            let mut stats = file.stats.clone();
            stats.bytes_printed = file_bytes_printed;
            let end = JsonMessage::End(JsonEnd {
                path: Some(file.path.clone()),
                binary_offset: file.binary_offset,
                stats,
            });
            total_bytes_printed += message_len(&end);
            messages.push(end);
        }

        let mut summary_stats = self.summary.clone();
        summary_stats.bytes_printed = total_bytes_printed;
        messages.push(JsonMessage::Summary(JsonSummary {
            elapsed_total: self.elapsed_total.clone(),
            stats: summary_stats,
        }));
        messages
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        for message in self.messages() {
            writer.write_all(message.to_line().as_bytes())?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    }
}

impl JsonMessage {
    pub fn to_line(&self) -> String {
        let mut out = String::new();
        out.push('{');
        match self {
            Self::Begin(begin) => {
                out.push_str("\"type\":\"begin\",\"data\":");
                begin.write_json(&mut out);
            }
            Self::End(end) => {
                out.push_str("\"type\":\"end\",\"data\":");
                end.write_json(&mut out);
            }
            Self::Match(matched) => {
                out.push_str("\"type\":\"match\",\"data\":");
                matched.write_json(&mut out);
            }
            Self::Context(context) => {
                out.push_str("\"type\":\"context\",\"data\":");
                context.write_json(&mut out);
            }
            Self::Summary(summary) => {
                out.push_str("\"type\":\"summary\",\"data\":");
                summary.write_json(&mut out);
            }
        }
        out.push('}');
        out
    }
}

impl JsonBegin {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"path\":");
        write_optional_data(out, self.path.as_ref());
        out.push('}');
    }
}

impl JsonEnd {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"path\":");
        write_optional_data(out, self.path.as_ref());
        out.push_str(",\"binary_offset\":");
        write_optional_u64(out, self.binary_offset);
        out.push_str(",\"stats\":");
        self.stats.write_json(out);
        out.push('}');
    }
}

impl JsonSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"elapsed_total\":");
        self.elapsed_total.write_json(out);
        out.push_str(",\"stats\":");
        self.stats.write_json(out);
        out.push('}');
    }
}

impl JsonMatch {
    fn write_json(&self, out: &mut String) {
        write_line_message_json(
            out,
            self.path.as_ref(),
            &self.lines,
            self.line_number,
            self.absolute_offset,
            &self.submatches,
        );
    }
}

impl JsonContext {
    fn write_json(&self, out: &mut String) {
        write_line_message_json(
            out,
            self.path.as_ref(),
            &self.lines,
            self.line_number,
            self.absolute_offset,
            &self.submatches,
        );
    }
}

fn write_line_message_json(
    out: &mut String,
    path: Option<&JsonData>,
    lines: &JsonData,
    line_number: Option<u64>,
    absolute_offset: u64,
    submatches: &[JsonSubmatch],
) {
    out.push('{');
    out.push_str("\"path\":");
    write_optional_data(out, path);
    out.push_str(",\"lines\":");
    lines.write_json(out);
    out.push_str(",\"line_number\":");
    write_optional_u64(out, line_number);
    out.push_str(",\"absolute_offset\":");
    out.push_str(&absolute_offset.to_string());
    out.push_str(",\"submatches\":[");
    for (idx, submatch) in submatches.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        submatch.write_json(out);
    }
    out.push(']');
    out.push('}');
}

impl JsonSubmatch {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"match\":");
        self.m.write_json(out);
        out.push_str(",\"start\":");
        out.push_str(&self.start.to_string());
        out.push_str(",\"end\":");
        out.push_str(&self.end.to_string());
        out.push('}');
    }
}

fn write_optional_data(out: &mut String, data: Option<&JsonData>) {
    match data {
        Some(data) => data.write_json(out),
        None => out.push_str("null"),
    }
}

fn write_optional_u64(out: &mut String, value: Option<u64>) {
    match value {
        Some(value) => out.push_str(&value.to_string()),
        None => out.push_str("null"),
    }
}

fn json_file_from_search_hit(hit: &SearchHit) -> JsonSearchFile {
    let path = JsonData::from_string(hit.path.clone());
    let mut messages = Vec::new();
    let mut matched_lines = 0u64;
    let mut matches = 0u64;

    for line in &hit.lines {
        match line {
            SearchLine::Match(file_match) => {
                matched_lines += 1;
                matches += 1;
                let start = file_match.location.column.saturating_sub(1);
                let end = start.saturating_add(file_match.matched_text.len());
                messages.push(JsonMessage::Match(JsonMatch {
                    path: Some(path.clone()),
                    lines: JsonData::from_string(file_match.snippet.clone()),
                    line_number: Some(file_match.location.line as u64),
                    absolute_offset: 0,
                    submatches: vec![JsonSubmatch {
                        m: JsonData::from_string(file_match.matched_text.clone()),
                        start,
                        end,
                    }],
                }));
            }
            SearchLine::Context(context) => {
                messages.push(JsonMessage::Context(JsonContext {
                    path: Some(path.clone()),
                    lines: JsonData::from_string(context.snippet.clone()),
                    line_number: Some(context.line_number as u64),
                    absolute_offset: 0,
                    submatches: Vec::new(),
                }));
            }
            SearchLine::ContextBreak => {}
        }
    }

    JsonSearchFile {
        path,
        messages,
        binary_offset: None,
        stats: JsonStats {
            elapsed: JsonDuration::zero(),
            searches: 1,
            searches_with_match: u64::from(matches > 0),
            bytes_searched: 0,
            bytes_printed: 0,
            matched_lines,
            matches,
        },
    }
}

fn message_len(message: &JsonMessage) -> u64 {
    message.to_line().len() as u64 + 1
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            ch if ch <= '\u{1F}' => {
                out.push_str("\\u");
                let code = ch as u32;
                out.push(hex_digit((code >> 12) & 0xF));
                out.push(hex_digit((code >> 8) & 0xF));
                out.push(hex_digit((code >> 4) & 0xF));
                out.push(hex_digit(code & 0xF));
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
}

fn hex_digit(value: u32) -> char {
    match value {
        0..=9 => char::from_u32(b'0' as u32 + value).unwrap_or('0'),
        10..=15 => char::from_u32(b'a' as u32 + value - 10).unwrap_or('a'),
        _ => '0',
    }
}

fn base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::new();
    let mut it = bytes.chunks_exact(3);
    for chunk in &mut it {
        let group24 =
            (usize::from(chunk[0]) << 16) | (usize::from(chunk[1]) << 8) | usize::from(chunk[2]);
        out.push(char::from(ALPHABET[(group24 >> 18) & 0b111_111]));
        out.push(char::from(ALPHABET[(group24 >> 12) & 0b111_111]));
        out.push(char::from(ALPHABET[(group24 >> 6) & 0b111_111]));
        out.push(char::from(ALPHABET[group24 & 0b111_111]));
    }
    match it.remainder() {
        [] => {}
        [byte0] => {
            let group8 = usize::from(*byte0);
            out.push(char::from(ALPHABET[(group8 >> 2) & 0b111_111]));
            out.push(char::from(ALPHABET[(group8 << 4) & 0b111_111]));
            out.push('=');
            out.push('=');
        }
        [byte0, byte1] => {
            let group16 = (usize::from(*byte0) << 8) | usize::from(*byte1);
            out.push(char::from(ALPHABET[(group16 >> 10) & 0b111_111]));
            out.push(char::from(ALPHABET[(group16 >> 4) & 0b111_111]));
            out.push(char::from(ALPHABET[(group16 << 2) & 0b111_111]));
            out.push('=');
        }
        _ => unreachable!("chunks_exact remainder length is at most 2"),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{JsonMessage, JsonSearchReport};
    use crate::search::{
        FileContext, FileMatch, MatchLocation, SearchHit, SearchLine, SearchResults,
    };

    #[test]
    fn from_search_results_projects_summary_stats() {
        let results = SearchResults {
            candidate_docs: 7,
            searches_with_match: 2,
            bytes_searched: 1234,
            matched_lines: 3,
            matched_occurrences: 4,
            file_counts: Vec::new(),
            file_match_counts: Vec::new(),
            hits: Vec::new(),
        };

        let report = JsonSearchReport::from_search_results(&results);
        assert_eq!(report.summary.searches, 7);
        assert_eq!(report.summary.searches_with_match, 2);
        assert_eq!(report.summary.bytes_searched, 1234);
        assert_eq!(report.summary.matched_lines, 3);
        assert_eq!(report.summary.matches, 4);
    }

    #[test]
    fn from_search_results_projects_lines_into_file_messages() {
        let results = SearchResults {
            candidate_docs: 1,
            searches_with_match: 1,
            bytes_searched: 10,
            matched_lines: 1,
            matched_occurrences: 1,
            file_counts: Vec::new(),
            file_match_counts: Vec::new(),
            hits: vec![SearchHit {
                path: "src/main.rs".into(),
                matches: vec![FileMatch {
                    location: MatchLocation { line: 3, column: 5 },
                    snippet: "let answer = value;".into(),
                    matched_text: "answer".into(),
                }],
                lines: vec![
                    SearchLine::Match(FileMatch {
                        location: MatchLocation { line: 3, column: 5 },
                        snippet: "let answer = value;".into(),
                        matched_text: "answer".into(),
                    }),
                    SearchLine::Context(FileContext {
                        line_number: 2,
                        snippet: "fn main() {".into(),
                    }),
                    SearchLine::ContextBreak,
                ],
            }],
        };

        let report = JsonSearchReport::from_search_results(&results);
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].messages.len(), 2);
        assert!(matches!(report.files[0].messages[0], JsonMessage::Match(_)));
        assert!(matches!(
            report.files[0].messages[1],
            JsonMessage::Context(_)
        ));
    }
}
