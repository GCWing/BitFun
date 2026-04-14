use aho_corasick::AhoCorasick;
use memchr::memrchr;

use crate::{config::QueryConfig, planner::QueryPlan};

#[derive(Debug, Clone)]
pub(super) struct SearchPreparation {
    pub(super) candidate_ids: Vec<u32>,
    pub(super) line_prefilter: Option<LinePrefilter>,
}

#[derive(Debug, Clone)]
pub(crate) enum LinePrefilter {
    Literal(LiteralPrefilter),
}

impl LinePrefilter {
    pub(crate) fn compile(config: &QueryConfig, plan: &QueryPlan) -> Option<Self> {
        let _ = config;
        LiteralPrefilter::new(plan, config.case_insensitive).map(Self::Literal)
    }

    pub(crate) fn find_candidate_line(
        &self,
        haystack: &[u8],
        line_terminator: u8,
    ) -> Option<usize> {
        match self {
            Self::Literal(prefilter) => prefilter.find_candidate_line(haystack, line_terminator),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LiteralPrefilter {
    branches: Vec<Vec<String>>,
    case_insensitive: bool,
    fast_path: Option<LiteralFastPath>,
}

#[derive(Debug, Clone)]
enum LiteralFastPath {
    AnySingleLiteral(AhoCorasick),
}

impl LiteralPrefilter {
    pub(crate) fn new(plan: &QueryPlan, case_insensitive: bool) -> Option<Self> {
        if plan.fallback_to_scan {
            return None;
        }

        if !plan
            .branches
            .iter()
            .any(|branch| !branch.literals.is_empty())
        {
            return None;
        }

        let branches = plan
            .branches
            .iter()
            .map(|branch| {
                branch
                    .literals
                    .iter()
                    .filter(|literal| !literal.is_empty())
                    .map(|literal| {
                        if case_insensitive {
                            fold_query_literal(literal)
                        } else {
                            literal.clone()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|branch| !branch.is_empty())
            .collect::<Vec<_>>();

        if branches.is_empty() {
            None
        } else {
            let fast_path = build_literal_fast_path(&branches, case_insensitive);
            Some(Self {
                branches,
                case_insensitive,
                fast_path,
            })
        }
    }

    pub(super) fn text_may_match(&self, text: &str) -> bool {
        if self.case_insensitive {
            if text.is_ascii() {
                let folded = text.to_ascii_lowercase();
                self.any_branch_matches(&folded)
            } else {
                let folded = text.to_lowercase();
                self.any_branch_matches(&folded)
            }
        } else {
            self.any_branch_matches(text)
        }
    }

    pub(super) fn line_may_match_bytes(&self, bytes: &[u8]) -> bool {
        if self.case_insensitive {
            match std::str::from_utf8(bytes) {
                Ok(text) => self.text_may_match(text),
                Err(_) => false,
            }
        } else {
            self.any_branch_matches_bytes(bytes)
        }
    }

    pub(super) fn find_candidate_line(
        &self,
        haystack: &[u8],
        line_terminator: u8,
    ) -> Option<usize> {
        if let Some(fast_path) = &self.fast_path {
            if let Some(offset) = fast_path.find_candidate_line(haystack, line_terminator) {
                return Some(offset);
            }
        }
        let mut offset = 0usize;
        while offset < haystack.len() {
            let line_len = haystack[offset..]
                .iter()
                .position(|&byte| byte == line_terminator)
                .map_or(haystack.len() - offset, |idx| idx + 1);
            let line_end = offset + line_len;
            if self.line_may_match_bytes(&haystack[offset..line_end]) {
                return Some(offset);
            }
            offset = line_end;
        }
        None
    }

    fn any_branch_matches(&self, haystack: &str) -> bool {
        self.branches
            .iter()
            .any(|branch| branch.iter().all(|literal| haystack.contains(literal)))
    }

    fn any_branch_matches_bytes(&self, haystack: &[u8]) -> bool {
        self.branches.iter().any(|branch| {
            branch
                .iter()
                .all(|literal| contains_bytes(haystack, literal.as_bytes()))
        })
    }
}

impl LiteralFastPath {
    fn find_candidate_line(&self, haystack: &[u8], line_terminator: u8) -> Option<usize> {
        match self {
            Self::AnySingleLiteral(matcher) => matcher.find(haystack).map(|matched| {
                memrchr(line_terminator, &haystack[..matched.start()])
                    .map_or(0, |line_end| line_end + 1)
            }),
        }
    }
}

fn build_literal_fast_path(
    branches: &[Vec<String>],
    case_insensitive: bool,
) -> Option<LiteralFastPath> {
    if case_insensitive || !branches.iter().all(|branch| branch.len() == 1) {
        return None;
    }
    let patterns = branches
        .iter()
        .filter_map(|branch| branch.first())
        .collect::<Vec<_>>();
    let matcher = AhoCorasick::new(patterns).ok()?;
    Some(LiteralFastPath::AnySingleLiteral(matcher))
}

pub(super) fn fold_query_literal(literal: &str) -> String {
    if literal.is_ascii() {
        literal.to_ascii_lowercase()
    } else {
        literal.to_lowercase()
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{LinePrefilter, LiteralPrefilter};
    use crate::{config::QueryConfig, planner::QueryPlan};

    #[test]
    fn literal_prefilter_supports_single_literal_branches() {
        let plan = crate::planner::plan(r"\wAh").expect("plan should succeed");
        let prefilter =
            LiteralPrefilter::new(&plan, false).expect("single literal should build prefilter");
        assert!(prefilter.line_may_match_bytes(b"foo Ah bar\n"));
        assert!(!prefilter.line_may_match_bytes(b"foo bar\n"));
        assert_eq!(
            prefilter.find_candidate_line(b"foo\nbar Ah baz\nqux\n", b'\n'),
            Some(4)
        );
    }

    #[test]
    fn line_prefilter_prefers_literal_branch_when_available() {
        let plan = crate::planner::plan(r"\wAh").expect("plan should succeed");
        let prefilter = LinePrefilter::compile(
            &QueryConfig {
                regex_pattern: r"\wAh".into(),
                patterns: vec![r"\wAh".into()],
                ..QueryConfig::default()
            },
            &plan,
        )
        .expect("prefilter should compile");
        assert!(matches!(prefilter, LinePrefilter::Literal(_)));
    }

    #[test]
    fn line_prefilter_is_absent_when_literals_are_unavailable() {
        let plan = QueryPlan {
            branches: Vec::new(),
            fallback_to_scan: true,
            pure_literal_alternation: None,
        };
        let prefilter = LinePrefilter::compile(
            &QueryConfig {
                regex_pattern: r"\p{Greek}".into(),
                patterns: vec![r"\p{Greek}".into()],
                ..QueryConfig::default()
            },
            &plan,
        );
        assert!(prefilter.is_none());
    }
}
