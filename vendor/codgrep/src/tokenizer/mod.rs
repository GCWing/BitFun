use std::collections::HashSet;

use crate::config::TokenizerMode;

mod sparse;
mod trigram;

#[derive(Debug, Clone)]
pub struct TokenizerOptions {
    pub min_sparse_len: usize,
    pub max_sparse_len: usize,
}

impl Default for TokenizerOptions {
    fn default() -> Self {
        Self {
            min_sparse_len: 3,
            max_sparse_len: 8,
        }
    }
}

pub trait Tokenizer {
    fn collect_document_token_hashes(&self, text: &str, out: &mut Vec<u64>);

    fn collect_query_token_hashes(&self, literal: &str, out: &mut Vec<u64>) {
        self.collect_document_token_hashes(literal, out);
    }
}

pub fn create(mode: TokenizerMode, options: TokenizerOptions) -> Box<dyn Tokenizer + Send + Sync> {
    match mode {
        TokenizerMode::Trigram => Box::new(trigram::TrigramTokenizer),
        TokenizerMode::SparseNgram => Box::new(sparse::SparseNgramTokenizer::new(options)),
    }
}

pub fn hash_token(token: &str) -> u64 {
    u64::from(crc32fast::hash(token.as_bytes()))
}

pub fn unique_sorted(tokens: Vec<String>) -> Vec<String> {
    let mut unique = HashSet::with_capacity(tokens.len());
    let mut values = Vec::new();

    for token in tokens {
        if unique.insert(token.clone()) {
            values.push(token);
        }
    }

    values.sort_unstable();
    values
}
