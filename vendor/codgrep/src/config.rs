use std::path::PathBuf;

use crate::error::Result;
use crate::path_utils::{normalize_existing_path, normalize_path_from_cwd};
use crate::search::SearchMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerMode {
    Trigram,
    SparseNgram,
}

impl TokenizerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trigram => "trigram",
            Self::SparseNgram => "sparse-ngram",
        }
    }

    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Trigram),
            2 => Some(Self::SparseNgram),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::Trigram => 1,
            Self::SparseNgram => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusMode {
    RespectIgnore,
    NoIgnore,
}

impl CorpusMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RespectIgnore => "ignore",
            Self::NoIgnore => "no-ignore",
        }
    }

    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::RespectIgnore),
            2 => Some(Self::NoIgnore),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::RespectIgnore => 1,
            Self::NoIgnore => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub repo_path: PathBuf,
    pub index_path: PathBuf,
    pub tokenizer: TokenizerMode,
    pub corpus_mode: CorpusMode,
    pub include_hidden: bool,
    pub max_file_size: u64,
    pub min_sparse_len: usize,
    pub max_sparse_len: usize,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::new(),
            index_path: PathBuf::new(),
            tokenizer: TokenizerMode::SparseNgram,
            corpus_mode: CorpusMode::RespectIgnore,
            include_hidden: false,
            max_file_size: 2 * 1024 * 1024,
            min_sparse_len: 3,
            max_sparse_len: 8,
        }
    }
}

impl BuildConfig {
    pub(crate) fn normalized(&self) -> Result<Self> {
        Ok(Self {
            repo_path: normalize_existing_path(&self.repo_path)?,
            index_path: normalize_path_from_cwd(&self.index_path)?,
            tokenizer: self.tokenizer,
            corpus_mode: self.corpus_mode,
            include_hidden: self.include_hidden,
            max_file_size: self.max_file_size,
            min_sparse_len: self.min_sparse_len,
            max_sparse_len: self.max_sparse_len,
        })
    }

    pub(crate) fn normalize_lossy(&self) -> Self {
        self.normalized().unwrap_or_else(|_| self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct QueryConfig {
    pub regex_pattern: String,
    pub patterns: Vec<String>,
    pub case_insensitive: bool,
    pub multiline: bool,
    pub dot_matches_new_line: bool,
    pub fixed_strings: bool,
    pub word_regexp: bool,
    pub line_regexp: bool,
    pub before_context: usize,
    pub after_context: usize,
    pub top_k_tokens: usize,
    pub max_count: Option<usize>,
    pub global_max_results: Option<usize>,
    pub search_mode: SearchMode,
}

impl QueryConfig {
    pub fn has_context(&self) -> bool {
        self.before_context > 0 || self.after_context > 0
    }

    pub fn effective_global_max_results(&self) -> Option<usize> {
        self.global_max_results
            .filter(|limit| *limit > 0 && self.search_mode.materializes_matches())
    }
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            regex_pattern: String::new(),
            patterns: Vec::new(),
            case_insensitive: false,
            multiline: false,
            dot_matches_new_line: false,
            fixed_strings: false,
            word_regexp: false,
            line_regexp: false,
            before_context: 0,
            after_context: 0,
            top_k_tokens: 6,
            max_count: None,
            global_max_results: None,
            search_mode: SearchMode::MaterializeMatches,
        }
    }
}
