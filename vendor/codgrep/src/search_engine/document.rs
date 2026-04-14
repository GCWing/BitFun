use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    config::{QueryConfig, TokenizerMode},
    error::Result,
    files::RepositoryFile,
    path_filter::PathFilter,
    tokenizer::{create, TokenizerOptions},
};

#[derive(Debug, Clone)]
pub(crate) struct SearchDocument {
    pub(crate) logical_path: String,
    pub(crate) size: u64,
    pub(crate) mtime_nanos: u64,
    pub(crate) source: SearchDocumentSource,
}

#[derive(Debug, Clone)]
pub(crate) enum SearchDocumentSource {
    Path(PathBuf),
    LoadedBytes(Arc<[u8]>),
}

impl SearchDocument {
    pub(crate) fn from_repository_file(file: &RepositoryFile) -> Self {
        Self {
            logical_path: file.path.to_string_lossy().into_owned(),
            size: file.size,
            mtime_nanos: file.mtime_nanos,
            source: SearchDocumentSource::Path(file.path.clone()),
        }
    }

    pub(crate) fn from_loaded_bytes(
        logical_path: String,
        size: u64,
        mtime_nanos: u64,
        bytes: Arc<[u8]>,
    ) -> Self {
        Self {
            logical_path,
            size,
            mtime_nanos,
            source: SearchDocumentSource::LoadedBytes(bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchDocumentIndex {
    documents: Vec<Option<SearchDocument>>,
    folded_text: Vec<Option<Arc<str>>>,
    token_hashes: Vec<Vec<u64>>,
    postings: HashMap<u64, Vec<u32>>,
    path_to_doc_id: HashMap<String, u32>,
    unindexed_doc_ids: BTreeSet<u32>,
    free_doc_ids: Vec<u32>,
    tokenizer_mode: TokenizerMode,
    tokenizer_options: TokenizerOptions,
}

impl SearchDocumentIndex {
    pub(crate) fn build(
        tokenizer_mode: TokenizerMode,
        tokenizer_options: TokenizerOptions,
        documents: Vec<SearchDocument>,
    ) -> Self {
        let mut index = Self {
            documents: Vec::new(),
            folded_text: Vec::new(),
            token_hashes: Vec::new(),
            postings: HashMap::new(),
            path_to_doc_id: HashMap::new(),
            unindexed_doc_ids: BTreeSet::new(),
            free_doc_ids: Vec::new(),
            tokenizer_mode,
            tokenizer_options,
        };

        for document in documents {
            index.upsert_document(document);
        }

        index
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.path_to_doc_id.is_empty()
    }

    pub(crate) fn upsert_document(&mut self, document: SearchDocument) {
        let doc_id = self
            .path_to_doc_id
            .get(&document.logical_path)
            .copied()
            .unwrap_or_else(|| self.allocate_doc_id());
        self.remove_doc_contents(doc_id);
        self.path_to_doc_id
            .insert(document.logical_path.clone(), doc_id);

        let (folded_text, token_hashes) = self.index_document_contents(&document);
        let slot = usize::try_from(doc_id).expect("doc id should fit usize");
        self.documents[slot] = Some(document);
        self.folded_text[slot] = folded_text;
        self.token_hashes[slot] = token_hashes.clone();

        if self.folded_text[slot].is_some() {
            self.unindexed_doc_ids.remove(&doc_id);
            for token_hash in token_hashes {
                insert_sorted_unique(self.postings.entry(token_hash).or_default(), doc_id);
            }
        } else {
            self.unindexed_doc_ids.insert(doc_id);
        }
    }

    pub(crate) fn remove_document(&mut self, path: &str) {
        let Some(doc_id) = self.path_to_doc_id.remove(path) else {
            return;
        };
        self.remove_doc_contents(doc_id);
        self.free_doc_ids.push(doc_id);
    }

    pub(crate) fn search(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<crate::search::SearchResults> {
        let candidate_ids = self.candidate_doc_ids(config, filter)?;
        let documents = self.documents_for_ids(&candidate_ids);
        super::scan::search_documents(config, &documents)
    }

    fn candidate_doc_ids(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<Vec<u32>> {
        let plan = crate::planner::plan(&config.regex_pattern)?;
        if plan.fallback_to_scan {
            return Ok(self.filtered_doc_ids(filter));
        }

        let tokenizer = create(self.tokenizer_mode, self.tokenizer_options.clone());
        let mut all_candidates = Vec::new();
        for branch in &plan.branches {
            let Some(branch_candidates) =
                self.select_branch_candidates(branch, tokenizer.as_ref())?
            else {
                return Ok(self.filtered_doc_ids(filter));
            };
            all_candidates = union_sorted_u32(&all_candidates, &branch_candidates);
        }
        let unindexed = self.unindexed_doc_ids.iter().copied().collect::<Vec<_>>();
        all_candidates = union_sorted_u32(&all_candidates, &unindexed);
        Ok(self.filter_candidate_ids(&all_candidates, filter))
    }

    fn select_branch_candidates(
        &self,
        branch: &crate::planner::QueryBranch,
        tokenizer: &dyn crate::tokenizer::Tokenizer,
    ) -> Result<Option<Vec<u32>>> {
        let mut branch_candidates: Option<Vec<u32>> = None;

        for literal in &branch.literals {
            let query_literal = fold_for_search_index(literal);
            let literal_candidates = self.literal_candidate_doc_ids(tokenizer, &query_literal)?;
            branch_candidates = Some(match branch_candidates.take() {
                Some(existing) => intersect_sorted_u32(&existing, &literal_candidates),
                None => literal_candidates,
            });
            if branch_candidates.as_ref().is_some_and(Vec::is_empty) {
                return Ok(Some(Vec::new()));
            }
        }

        Ok(branch_candidates)
    }

    fn literal_candidate_doc_ids(
        &self,
        tokenizer: &dyn crate::tokenizer::Tokenizer,
        query_literal: &str,
    ) -> Result<Vec<u32>> {
        let mut covering_hashes = Vec::new();
        tokenizer.collect_query_token_hashes(query_literal, &mut covering_hashes);
        covering_hashes.sort_unstable();
        covering_hashes.dedup();

        let tolerate_missing_covering_hashes =
            matches!(self.tokenizer_mode, TokenizerMode::SparseNgram);
        let mut selected_hashes = Vec::new();
        let mut seen_hashes = HashSet::new();

        for token_hash in covering_hashes {
            if !seen_hashes.insert(token_hash) {
                continue;
            }
            if self.postings.contains_key(&token_hash) {
                selected_hashes.push(token_hash);
            } else if !tolerate_missing_covering_hashes {
                return Ok(Vec::new());
            }
        }

        if matches!(self.tokenizer_mode, TokenizerMode::Trigram) && selected_hashes.len() < 2 {
            let mut fallback_hashes = Vec::new();
            tokenizer.collect_document_token_hashes(query_literal, &mut fallback_hashes);
            fallback_hashes.sort_unstable();
            fallback_hashes.dedup();
            for token_hash in fallback_hashes {
                if seen_hashes.insert(token_hash) && self.postings.contains_key(&token_hash) {
                    selected_hashes.push(token_hash);
                }
            }
        }

        if selected_hashes.is_empty() {
            return Ok(self.scan_literal_doc_ids(query_literal, None));
        }

        selected_hashes.sort_unstable_by_key(|token_hash| {
            self.postings
                .get(token_hash)
                .map_or(usize::MAX, std::vec::Vec::len)
        });

        let mut candidates: Option<Vec<u32>> = None;
        for token_hash in selected_hashes {
            let docs = self.postings.get(&token_hash).cloned().unwrap_or_default();
            candidates = Some(match candidates.take() {
                Some(existing) => intersect_sorted_u32(&existing, &docs),
                None => docs,
            });
            if candidates.as_ref().is_some_and(Vec::is_empty) {
                return Ok(Vec::new());
            }
        }

        let candidates = candidates.unwrap_or_default();
        Ok(self.scan_literal_doc_ids(query_literal, Some(&candidates)))
    }

    fn scan_literal_doc_ids(&self, literal: &str, candidate_ids: Option<&[u32]>) -> Vec<u32> {
        let iter: Box<dyn Iterator<Item = u32>> = match candidate_ids {
            Some(ids) => Box::new(ids.iter().copied()),
            None => Box::new(0..u32::try_from(self.documents.len()).unwrap_or_default()),
        };

        iter.filter(|doc_id| self.document_contains_literal(*doc_id, literal))
            .collect()
    }

    fn document_contains_literal(&self, doc_id: u32, literal: &str) -> bool {
        let index = match usize::try_from(doc_id) {
            Ok(value) => value,
            Err(_) => return false,
        };
        self.folded_text
            .get(index)
            .and_then(|text| text.as_deref())
            .is_some_and(|text| text.contains(literal))
    }

    fn filtered_doc_ids(&self, filter: Option<&PathFilter>) -> Vec<u32> {
        (0..self.documents.len())
            .filter_map(|index| {
                self.documents.get(index)?.as_ref()?;
                let doc_id = u32::try_from(index).ok()?;
                self.doc_matches_filter(doc_id, filter).then_some(doc_id)
            })
            .collect()
    }

    fn filter_candidate_ids(&self, candidate_ids: &[u32], filter: Option<&PathFilter>) -> Vec<u32> {
        if filter.is_none() {
            return candidate_ids.to_vec();
        }
        candidate_ids
            .iter()
            .copied()
            .filter(|doc_id| self.doc_matches_filter(*doc_id, filter))
            .collect()
    }

    fn doc_matches_filter(&self, doc_id: u32, filter: Option<&PathFilter>) -> bool {
        let Some(filter) = filter else {
            return true;
        };
        let index = match usize::try_from(doc_id) {
            Ok(value) => value,
            Err(_) => return false,
        };
        self.documents
            .get(index)
            .and_then(|document| document.as_ref())
            .is_some_and(|document| filter.matches_file(Path::new(&document.logical_path)))
    }

    fn allocate_doc_id(&mut self) -> u32 {
        if let Some(doc_id) = self.free_doc_ids.pop() {
            return doc_id;
        }

        let doc_id = u32::try_from(self.documents.len()).expect("document index exceeds u32 range");
        self.documents.push(None);
        self.folded_text.push(None);
        self.token_hashes.push(Vec::new());
        doc_id
    }

    fn index_document_contents(&self, document: &SearchDocument) -> (Option<Arc<str>>, Vec<u64>) {
        let Some(text) = folded_document_text(document) else {
            return (None, Vec::new());
        };

        let tokenizer = create(self.tokenizer_mode, self.tokenizer_options.clone());
        let mut token_hashes = Vec::new();
        tokenizer.collect_document_token_hashes(&text, &mut token_hashes);
        token_hashes.sort_unstable();
        token_hashes.dedup();
        (Some(Arc::<str>::from(text)), token_hashes)
    }

    fn remove_doc_contents(&mut self, doc_id: u32) {
        let slot = match usize::try_from(doc_id) {
            Ok(value) => value,
            Err(_) => return,
        };
        if slot >= self.documents.len() {
            return;
        }

        for token_hash in self.token_hashes[slot].drain(..) {
            let remove_entry = if let Some(postings) = self.postings.get_mut(&token_hash) {
                remove_sorted(postings, doc_id);
                postings.is_empty()
            } else {
                false
            };
            if remove_entry {
                self.postings.remove(&token_hash);
            }
        }
        self.unindexed_doc_ids.remove(&doc_id);
        self.documents[slot] = None;
        self.folded_text[slot] = None;
    }

    fn documents_for_ids(&self, candidate_ids: &[u32]) -> Vec<SearchDocument> {
        candidate_ids
            .iter()
            .filter_map(|doc_id| usize::try_from(*doc_id).ok())
            .filter_map(|index| self.documents.get(index))
            .filter_map(|document| document.clone())
            .collect()
    }
}

fn folded_document_text(document: &SearchDocument) -> Option<String> {
    match &document.source {
        SearchDocumentSource::Path(path) => std::fs::read_to_string(path)
            .ok()
            .map(|text| fold_for_search_index(&text)),
        SearchDocumentSource::LoadedBytes(bytes) => {
            std::str::from_utf8(bytes).ok().map(fold_for_search_index)
        }
    }
}

fn fold_for_search_index(text: &str) -> String {
    if text.is_ascii() {
        text.to_ascii_lowercase()
    } else {
        text.to_lowercase()
    }
}

fn intersect_sorted_u32(left: &[u32], right: &[u32]) -> Vec<u32> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(left.len().min(right.len()));
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                result.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    result
}

fn union_sorted_u32(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut result = Vec::with_capacity(left.len() + right.len());
    let mut left_index = 0usize;
    let mut right_index = 0usize;

    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => {
                result.push(left[left_index]);
                left_index += 1;
            }
            std::cmp::Ordering::Greater => {
                result.push(right[right_index]);
                right_index += 1;
            }
            std::cmp::Ordering::Equal => {
                result.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }

    result.extend_from_slice(&left[left_index..]);
    result.extend_from_slice(&right[right_index..]);
    result
}

fn insert_sorted_unique(values: &mut Vec<u32>, needle: u32) {
    match values.binary_search(&needle) {
        Ok(_) => {}
        Err(index) => values.insert(index, needle),
    }
}

fn remove_sorted(values: &mut Vec<u32>, needle: u32) {
    if let Ok(index) = values.binary_search(&needle) {
        values.remove(index);
    }
}
