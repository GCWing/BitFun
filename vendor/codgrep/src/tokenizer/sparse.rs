use crate::tokenizer::Tokenizer;
use crate::tokenizer::TokenizerOptions;

pub struct SparseNgramTokenizer {
    options: TokenizerOptions,
}

impl SparseNgramTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self { options }
    }
}

impl Tokenizer for SparseNgramTokenizer {
    fn collect_document_token_hashes(&self, text: &str, out: &mut Vec<u64>) {
        collect_document_sparse_token_hashes(text, &self.options, out);
    }

    fn collect_query_token_hashes(&self, literal: &str, out: &mut Vec<u64>) {
        collect_query_sparse_token_hashes(literal, &self.options, out);
    }
}

fn collect_document_sparse_token_hashes(
    text: &str,
    options: &TokenizerOptions,
    out: &mut Vec<u64>,
) {
    let char_offsets = build_char_offsets(text);
    let char_len = char_offsets.len().saturating_sub(1);
    if char_len < options.min_sparse_len {
        return;
    }

    let bytes = text.as_bytes();
    let weights = build_bigram_weights(bytes, &char_offsets);
    let intervals = build_all_intervals(&weights, options);
    hash_intervals(bytes, &char_offsets, intervals, out);
    collect_ascii_span_fallback_trigrams(bytes, options, out);
}

fn collect_query_sparse_token_hashes(text: &str, options: &TokenizerOptions, out: &mut Vec<u64>) {
    let char_offsets = build_char_offsets(text);
    let char_len = char_offsets.len().saturating_sub(1);
    if char_len < options.min_sparse_len {
        return;
    }

    let bytes = text.as_bytes();
    let weights = build_bigram_weights(bytes, &char_offsets);
    let all_intervals = build_all_intervals(&weights, options);
    if all_intervals.is_empty() {
        return;
    }

    let covering = build_covering_intervals(&all_intervals, char_len);
    if covering.is_empty() {
        hash_intervals(bytes, &char_offsets, all_intervals, out);
    } else {
        hash_intervals(bytes, &char_offsets, covering, out);
    }
}

fn build_char_offsets(text: &str) -> Vec<usize> {
    let mut offsets = text.char_indices().map(|(idx, _)| idx).collect::<Vec<_>>();
    offsets.push(text.len());
    offsets
}

fn build_bigram_weights(bytes: &[u8], char_offsets: &[usize]) -> Vec<u32> {
    let char_len = char_offsets.len().saturating_sub(1);
    let mut weights = Vec::with_capacity(char_len.saturating_sub(1));
    for start in 0..char_len.saturating_sub(1) {
        let start_byte = char_offsets[start];
        let end_byte = char_offsets[start + 2];
        weights.push(crc32fast::hash(&bytes[start_byte..end_byte]));
    }
    weights
}

fn build_all_intervals(weights: &[u32], options: &TokenizerOptions) -> Vec<Interval> {
    if weights.len() < 2 {
        return Vec::new();
    }

    let min_bigram_span = options.min_sparse_len.saturating_sub(2).max(1);
    let max_bigram_span = options.max_sparse_len.saturating_sub(2).max(1);
    let mut intervals = Vec::new();

    for left_bigram in 0..weights.len() {
        let start_right = left_bigram + min_bigram_span;
        if start_right >= weights.len() {
            break;
        }
        let end_right = (left_bigram + max_bigram_span).min(weights.len() - 1);
        if end_right < start_right {
            continue;
        }

        let left_weight = weights[left_bigram];
        let mut interior_max = u32::MIN;
        for right_bigram in left_bigram + 1..=end_right {
            if right_bigram > left_bigram + 1 {
                interior_max = interior_max.max(weights[right_bigram - 1]);
            }

            if right_bigram < start_right {
                continue;
            }

            let right_weight = weights[right_bigram];
            if left_weight > interior_max && right_weight > interior_max {
                intervals.push(Interval {
                    start_char: left_bigram,
                    end_char: right_bigram + 2,
                });
            }
        }
    }

    intervals
}

fn build_covering_intervals(intervals: &[Interval], char_len: usize) -> Vec<Interval> {
    if intervals.is_empty() {
        return Vec::new();
    }

    let mut sorted = intervals.to_vec();
    sorted.sort_unstable_by(|left, right| {
        left.start_char
            .cmp(&right.start_char)
            .then(right.end_char.cmp(&left.end_char))
    });

    let mut selected = Vec::new();
    let mut cursor = 0usize;
    let mut scan = 0usize;

    while cursor < char_len {
        let mut best: Option<Interval> = None;
        while scan < sorted.len() && sorted[scan].start_char <= cursor {
            let candidate = sorted[scan];
            if best.is_none_or(|current| candidate.end_char > current.end_char) {
                best = Some(candidate);
            }
            scan += 1;
        }

        let Some(best) = best else {
            return Vec::new();
        };
        if best.end_char <= cursor {
            return Vec::new();
        }

        selected.push(best);
        cursor = best.end_char;
    }

    selected
}

fn hash_intervals(
    bytes: &[u8],
    char_offsets: &[usize],
    intervals: Vec<Interval>,
    out: &mut Vec<u64>,
) {
    let mut hashed_tokens = Vec::with_capacity(intervals.len());
    for interval in intervals {
        let start_byte = char_offsets[interval.start_char];
        let end_byte = char_offsets[interval.end_char];
        hashed_tokens.push(u64::from(crc32fast::hash(&bytes[start_byte..end_byte])));
    }
    hashed_tokens.sort_unstable();
    hashed_tokens.dedup();
    out.extend(hashed_tokens);
}

fn collect_ascii_span_fallback_trigrams(
    bytes: &[u8],
    options: &TokenizerOptions,
    out: &mut Vec<u64>,
) {
    let mut start = 0usize;
    while start < bytes.len() {
        while start < bytes.len() && !is_ascii_token_byte(bytes[start]) {
            start += 1;
        }
        if start >= bytes.len() {
            break;
        }

        let mut end = start;
        while end < bytes.len() && is_ascii_token_byte(bytes[end]) {
            end += 1;
        }

        let span = &bytes[start..end];
        let char_len = span.len();
        if char_len >= options.min_sparse_len {
            let char_offsets = (0..=span.len()).collect::<Vec<_>>();
            let weights = build_bigram_weights(span, &char_offsets);
            if build_all_intervals(&weights, options).is_empty() {
                collect_trigram_hashes(span, out);
            }
        }

        start = end;
    }
}

fn collect_trigram_hashes(bytes: &[u8], out: &mut Vec<u64>) {
    if bytes.len() < 3 {
        return;
    }
    out.reserve(bytes.len().saturating_sub(2));
    for window in bytes.windows(3) {
        out.push(u64::from(crc32fast::hash(window)));
    }
}

fn is_ascii_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
fn is_sparse_boundary(weights: &[u32], left_bigram: usize, right_bigram: usize) -> bool {
    if right_bigram <= left_bigram || right_bigram >= weights.len() {
        return false;
    }

    let boundary_floor = weights[left_bigram].min(weights[right_bigram]);
    weights[left_bigram + 1..right_bigram]
        .iter()
        .all(|&interior| interior < boundary_floor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Interval {
    start_char: usize,
    end_char: usize,
}

#[cfg(test)]
mod tests {
    use super::{
        build_all_intervals, build_bigram_weights, build_char_offsets, build_covering_intervals,
        is_sparse_boundary, SparseNgramTokenizer,
    };
    use crate::tokenizer::{Tokenizer, TokenizerOptions};

    #[test]
    fn sparse_tokenizer_emits_some_tokens_for_ascii() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 3,
            max_sparse_len: 8,
        });
        let mut tokens = Vec::new();
        tokenizer.collect_document_token_hashes("getUserById", &mut tokens);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn empty_interior_uses_boundary_weights() {
        let weights = vec![10, 20];
        assert!(is_sparse_boundary(&weights, 0, 1));
    }

    #[test]
    fn bigram_weights_work_for_unicode_slices() {
        let text = "a你b";
        let offsets = build_char_offsets(text);
        let weights = build_bigram_weights(text.as_bytes(), &offsets);
        assert_eq!(weights.len(), 2);
        assert_ne!(weights[0], weights[1]);
    }

    #[test]
    fn query_tokens_prefer_boundary_sparse_ngrams() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 3,
            max_sparse_len: 8,
        });
        let mut doc_tokens = Vec::new();
        tokenizer.collect_document_token_hashes("getUserById", &mut doc_tokens);
        doc_tokens.sort_unstable();
        doc_tokens.dedup();

        let mut query_tokens = Vec::new();
        tokenizer.collect_query_token_hashes("getUserById", &mut query_tokens);
        query_tokens.sort_unstable();
        query_tokens.dedup();
        assert!(!query_tokens.is_empty());
        assert!(query_tokens.len() <= doc_tokens.len());
        assert!(query_tokens.iter().all(|token| doc_tokens.contains(token)));
    }

    #[test]
    fn sparse_tokenizer_handles_unicode_without_string_tokens() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 3,
            max_sparse_len: 8,
        });
        let mut tokens = Vec::new();
        tokenizer.collect_document_token_hashes("你好世界abc", &mut tokens);
        assert!(!tokens.is_empty());
        let mut deduped = tokens.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(tokens, deduped);
    }

    #[test]
    fn document_tokens_fall_back_to_trigrams_for_ascii_identifier_without_sparse_intervals() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 5,
            max_sparse_len: 16,
        });
        let mut tokens = Vec::new();
        tokenizer.collect_document_token_hashes("const err_sys = 1;", &mut tokens);
        tokens.sort_unstable();
        tokens.dedup();

        let expected = ["err", "rr_", "r_s", "_sy", "sys"]
            .into_iter()
            .map(|token| u64::from(crc32fast::hash(token.as_bytes())))
            .collect::<Vec<_>>();
        for token in expected {
            assert!(tokens.contains(&token));
        }
    }

    #[test]
    fn query_covering_can_be_empty_for_identifier_substring() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 5,
            max_sparse_len: 16,
        });

        let mut query_tokens = Vec::new();
        tokenizer.collect_query_token_hashes("err_sys", &mut query_tokens);
        query_tokens.sort_unstable();
        query_tokens.dedup();

        assert!(query_tokens.is_empty());
    }

    #[test]
    fn build_all_intervals_match_boundary_rule() {
        let text = "abcd";
        let offsets = build_char_offsets(text);
        let weights = build_bigram_weights(text.as_bytes(), &offsets);
        let intervals = build_all_intervals(
            &weights,
            &TokenizerOptions {
                min_sparse_len: 3,
                max_sparse_len: 8,
            },
        );
        assert!(!intervals.is_empty());
        for interval in intervals {
            let left = interval.start_char;
            let right = interval.end_char - 2;
            assert!(is_sparse_boundary(&weights, left, right));
        }
    }

    #[test]
    fn build_all_intervals_emits_complete_sparse_set_within_bounds() {
        let text = "getUserById";
        let offsets = build_char_offsets(text);
        let weights = build_bigram_weights(text.as_bytes(), &offsets);
        let options = TokenizerOptions {
            min_sparse_len: 5,
            max_sparse_len: 16,
        };

        let actual = build_all_intervals(&weights, &options);
        let mut expected = Vec::new();
        for left_bigram in 0..weights.len() {
            for right_bigram in left_bigram + 1..weights.len() {
                let char_len = right_bigram - left_bigram + 2;
                if char_len < options.min_sparse_len || char_len > options.max_sparse_len {
                    continue;
                }
                if is_sparse_boundary(&weights, left_bigram, right_bigram) {
                    expected.push(super::Interval {
                        start_char: left_bigram,
                        end_char: right_bigram + 2,
                    });
                }
            }
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn covering_intervals_reduce_interval_count() {
        let intervals = vec![
            super::Interval {
                start_char: 0,
                end_char: 4,
            },
            super::Interval {
                start_char: 0,
                end_char: 6,
            },
            super::Interval {
                start_char: 4,
                end_char: 8,
            },
        ];
        let covering = build_covering_intervals(&intervals, 8);
        assert_eq!(covering.len(), 2);
        assert_eq!(covering[0].start_char, 0);
        assert_eq!(covering[0].end_char, 6);
    }

    #[test]
    fn superstring_document_tokens_do_not_help_substring_query_covering() {
        let tokenizer = SparseNgramTokenizer::new(TokenizerOptions {
            min_sparse_len: 5,
            max_sparse_len: 16,
        });

        let query = "err_sys";
        let doc = "ac_err_system";

        let mut doc_tokens = Vec::new();
        tokenizer.collect_document_token_hashes(doc, &mut doc_tokens);
        doc_tokens.sort_unstable();
        doc_tokens.dedup();

        let mut query_tokens = Vec::new();
        tokenizer.collect_query_token_hashes(query, &mut query_tokens);
        query_tokens.sort_unstable();
        query_tokens.dedup();

        let shared = query_tokens
            .iter()
            .filter(|token| doc_tokens.contains(token))
            .count();

        assert!(doc_tokens.len() >= 1);
        assert!(query_tokens.is_empty());
        assert_eq!(shared, 0);
    }
}
