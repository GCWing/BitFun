use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crc32fast::Hasher;

use crate::{
    config::BuildConfig,
    error::{AppError, Result},
    files::{
        is_workspace_internal_path, read_text_file, repo_relative_path, resolve_repo_path,
        scan_repository, RepositoryFile,
    },
    index::format::{
        activate_generation, ensure_dir, read_doc_terms_file, read_docs_file, read_docs_header,
        validate_index_layout, write_lookup_file, DocMeta, DocTermsWriter, DocsWriter,
        FallbackTrigramSettings, IndexBuildSettings, IndexLayout, IndexMetadata, LookupEntry,
        PostingsWriter,
    },
    progress::{IndexProgress, IndexProgressPhase},
    tokenizer::{create, TokenizerOptions},
};

const INDEX_BUILD_BATCH_SIZE: usize = 512;
const INDEX_BUILD_BUCKET_COUNT: usize = 256;
const BUCKET_RECORD_BYTES: usize = 12;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Build-time hooks for optional progress reporting and cancellation.
///
/// This keeps the public build API stable while allowing callers to opt into
/// richer orchestration behavior.
pub struct IndexBuildOptions<'a> {
    pub progress: Option<&'a mut dyn FnMut(IndexProgress)>,
    pub should_cancel: Option<&'a mut dyn FnMut() -> bool>,
    pub rebuild_mode: RebuildMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildMode {
    ReuseExistingSnapshot,
    ForceRebuildSnapshot,
}

impl<'a> Default for IndexBuildOptions<'a> {
    fn default() -> Self {
        Self {
            progress: None,
            should_cancel: None,
            rebuild_mode: RebuildMode::ReuseExistingSnapshot,
        }
    }
}

impl<'a> IndexBuildOptions<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_progress(mut self, progress: &'a mut dyn FnMut(IndexProgress)) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn with_cancel(mut self, should_cancel: &'a mut dyn FnMut() -> bool) -> Self {
        self.should_cancel = Some(should_cancel);
        self
    }

    pub fn with_rebuild_mode(mut self, rebuild_mode: RebuildMode) -> Self {
        self.rebuild_mode = rebuild_mode;
        self
    }
}

pub fn build_index(config: &BuildConfig) -> Result<usize> {
    build_index_with_options(config, IndexBuildOptions::default())
}

pub fn build_index_with_options(
    config: &BuildConfig,
    mut options: IndexBuildOptions<'_>,
) -> Result<usize> {
    let mut noop_progress = |_event: IndexProgress| {};
    let mut noop_cancel = || false;
    let rebuild_mode = options.rebuild_mode;
    let on_progress = options.progress.take().unwrap_or(&mut noop_progress);
    let should_cancel = options.should_cancel.take().unwrap_or(&mut noop_cancel);

    build_index_internal(
        config,
        rebuild_mode,
        |event| on_progress(event),
        || should_cancel(),
    )
}

fn build_index_internal<F, C>(
    config: &BuildConfig,
    rebuild_mode: RebuildMode,
    on_progress: F,
    should_cancel: C,
) -> Result<usize>
where
    F: FnMut(IndexProgress),
    C: FnMut() -> bool,
{
    let mut on_progress = on_progress;
    let mut should_cancel = should_cancel;
    let config = config.normalized()?;
    check_cancel(&mut should_cancel)?;
    report_progress(
        &mut on_progress,
        IndexProgressPhase::Scanning,
        "Resolving base snapshot",
        0,
        None,
    );
    let snapshot = resolve_base_snapshot(&config, &mut should_cancel)?;
    ensure_dir(&config.index_path)?;
    ensure_dir(&IndexLayout::generations_dir(&config.index_path))?;

    let generation = match rebuild_mode {
        RebuildMode::ReuseExistingSnapshot => snapshot.generation.clone(),
        RebuildMode::ForceRebuildSnapshot => fresh_generation_name(&snapshot.generation),
    };
    let layout = IndexLayout::for_generation(&config.index_path, &generation);
    if snapshot.git_worktree_dirty && rebuild_mode == RebuildMode::ReuseExistingSnapshot {
        if let Some(doc_count) = reusable_generation_doc_count(
            &config,
            &generation,
            snapshot.head_commit.as_deref(),
            &snapshot.config_fingerprint,
        ) {
            check_cancel(&mut should_cancel)?;
            activate_generation(&config.index_path, &generation)?;
            report_progress(
                &mut on_progress,
                IndexProgressPhase::Finalizing,
                "Reused existing dirty worktree snapshot",
                1,
                Some(1),
            );
            return Ok(doc_count);
        }
    }

    let tokenizer_options = TokenizerOptions {
        min_sparse_len: config.min_sparse_len,
        max_sparse_len: config.max_sparse_len,
    };
    ensure_dir(&layout.data_path)?;
    cleanup_build_artifacts(&layout)?;
    check_cancel(&mut should_cancel)?;

    let mut builder = StreamingIndexBuilder::new(&config, &snapshot, &layout)?;
    let cached_docs = load_cached_docs(&config, &snapshot);

    if let Some(head_commit) = snapshot.head_commit.as_deref() {
        if snapshot.git_worktree_dirty {
            build_dirty_head_index(
                &config,
                &layout,
                &mut builder,
                head_commit,
                tokenizer_options,
                &mut on_progress,
                &mut should_cancel,
            )?;
        } else {
            build_clean_git_index(
                &config,
                &cached_docs,
                &mut builder,
                tokenizer_options,
                &mut on_progress,
                &mut should_cancel,
            )?;
        }
    } else {
        check_cancel(&mut should_cancel)?;
        report_progress(
            &mut on_progress,
            IndexProgressPhase::Scanning,
            "Scanning repository files",
            0,
            None,
        );
        let files = scan_repository(&config)?;
        check_cancel(&mut should_cancel)?;
        report_progress(
            &mut on_progress,
            IndexProgressPhase::Scanning,
            "Scanned repository files",
            files.len(),
            Some(files.len()),
        );
        build_clean_index(
            &files,
            config.tokenizer,
            tokenizer_options,
            &cached_docs,
            &mut builder,
            &mut on_progress,
            &mut should_cancel,
        )?;
    }

    let doc_count = builder.finish_with_progress(
        config.tokenizer,
        &layout,
        &mut on_progress,
        &mut should_cancel,
    )?;
    check_cancel(&mut should_cancel)?;
    activate_generation(&config.index_path, &generation)?;
    report_progress(
        &mut on_progress,
        IndexProgressPhase::Finalizing,
        "Activated base snapshot",
        1,
        Some(1),
    );

    Ok(doc_count)
}

pub fn rebuild_index(config: &BuildConfig) -> Result<usize> {
    rebuild_index_with_options(config, IndexBuildOptions::default())
}

pub fn rebuild_index_with_options(
    config: &BuildConfig,
    options: IndexBuildOptions<'_>,
) -> Result<usize> {
    build_index_with_options(
        config,
        IndexBuildOptions {
            rebuild_mode: RebuildMode::ForceRebuildSnapshot,
            ..options
        },
    )
}

fn compute_high_freq_doc_threshold(doc_count: usize) -> usize {
    (doc_count / 20).max(256)
}

fn build_clean_index(
    files: &[RepositoryFile],
    tokenizer_mode: crate::config::TokenizerMode,
    tokenizer_options: TokenizerOptions,
    cached_docs: &HashMap<String, CachedDoc>,
    builder: &mut StreamingIndexBuilder,
    on_progress: &mut impl FnMut(IndexProgress),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    let total_files = files.len();
    let mut processed_files = 0usize;
    for batch in files.chunks(INDEX_BUILD_BATCH_SIZE) {
        check_cancel(should_cancel)?;
        let mut indexed_files = process_file_batch_in_parallel(
            batch,
            tokenizer_mode,
            tokenizer_options.clone(),
            cached_docs,
        )?;
        processed_files += batch.len();
        report_progress(
            on_progress,
            IndexProgressPhase::Tokenizing,
            format!("Tokenized {processed_files}/{total_files} files"),
            processed_files,
            Some(total_files),
        );
        indexed_files.sort_unstable_by_key(|doc| doc.ordinal);
        builder.write_batch(indexed_files)?;
        report_progress(
            on_progress,
            IndexProgressPhase::Writing,
            format!("Wrote {processed_files}/{total_files} documents"),
            processed_files,
            Some(total_files),
        );
        check_cancel(should_cancel)?;
    }
    Ok(())
}

fn build_dirty_head_index(
    config: &BuildConfig,
    _layout: &IndexLayout,
    builder: &mut StreamingIndexBuilder,
    head_commit: &str,
    tokenizer_options: TokenizerOptions,
    on_progress: &mut impl FnMut(IndexProgress),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    check_cancel(should_cancel)?;
    let checkout = materialize_head_checkout(&config.repo_path, head_commit, should_cancel)?;
    let dirty_paths = dirty_tracked_paths(
        &config.repo_path,
        &config.index_path,
        head_commit,
        should_cancel,
    )?;
    report_progress(
        on_progress,
        IndexProgressPhase::Scanning,
        "Scanning HEAD snapshot files for dirty worktree build",
        0,
        None,
    );
    let files = scan_git_head_files(
        &config.repo_path,
        &checkout.root,
        head_commit,
        config.include_hidden,
        config.max_file_size,
        should_cancel,
    )?;
    report_progress(
        on_progress,
        IndexProgressPhase::Scanning,
        format!(
            "Scanned HEAD snapshot files ({} files, {} dirty paths)",
            files.len(),
            dirty_paths.len()
        ),
        files.len(),
        Some(files.len()),
    );

    let context = HeadSnapshotContext {
        checkout_root: &checkout.root,
        repo_root: &config.repo_path,
        dirty_paths: &dirty_paths,
    };
    let total_files = files.len();
    let mut processed_files = 0usize;
    for batch in files.chunks(INDEX_BUILD_BATCH_SIZE) {
        check_cancel(should_cancel)?;
        let mut indexed_files = process_file_batch_in_parallel(
            batch,
            config.tokenizer,
            tokenizer_options.clone(),
            &HashMap::new(),
        )?;
        processed_files += batch.len();
        report_progress(
            on_progress,
            IndexProgressPhase::Tokenizing,
            format!("Tokenized {processed_files}/{total_files} HEAD snapshot files"),
            processed_files,
            Some(total_files),
        );
        rewrite_head_snapshot_batch(&mut indexed_files, &context)?;
        indexed_files.sort_unstable_by_key(|doc| doc.ordinal);
        builder.write_batch(indexed_files)?;
        report_progress(
            on_progress,
            IndexProgressPhase::Writing,
            format!("Wrote {processed_files}/{total_files} HEAD snapshot documents"),
            processed_files,
            Some(total_files),
        );
        check_cancel(should_cancel)?;
    }
    Ok(())
}

fn build_clean_git_index(
    config: &BuildConfig,
    cached_docs: &HashMap<String, CachedDoc>,
    builder: &mut StreamingIndexBuilder,
    tokenizer_options: TokenizerOptions,
    on_progress: &mut impl FnMut(IndexProgress),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    check_cancel(should_cancel)?;
    report_progress(
        on_progress,
        IndexProgressPhase::Scanning,
        "Scanning git tracked files",
        0,
        None,
    );
    let files = scan_git_tracked_files(
        &config.repo_path,
        &config.repo_path,
        config.include_hidden,
        config.max_file_size,
        should_cancel,
    )?;
    check_cancel(should_cancel)?;
    report_progress(
        on_progress,
        IndexProgressPhase::Scanning,
        "Scanned git tracked files",
        files.len(),
        Some(files.len()),
    );
    build_clean_index(
        &files,
        config.tokenizer,
        tokenizer_options,
        cached_docs,
        builder,
        on_progress,
        should_cancel,
    )
}

fn process_file_batch_in_parallel(
    files: &[RepositoryFile],
    tokenizer_mode: crate::config::TokenizerMode,
    tokenizer_options: TokenizerOptions,
    cached_docs: &HashMap<String, CachedDoc>,
) -> Result<Vec<IndexedFile>> {
    let mut indexed_files = Vec::with_capacity(files.len());
    let mut files_to_process = Vec::new();

    for file in files {
        let path = file.path.to_string_lossy().into_owned();
        if let Some(cached) = cached_docs.get(&path) {
            if cached.size == file.size && cached.mtime_nanos == file.mtime_nanos {
                indexed_files.push(IndexedFile {
                    ordinal: file.ordinal,
                    path,
                    size: file.size,
                    mtime_nanos: file.mtime_nanos,
                    token_hashes: cached.token_hashes.clone(),
                    fallback_token_hashes: cached.fallback_token_hashes.clone(),
                });
                continue;
            }
        }
        files_to_process.push(file.clone());
    }

    if files_to_process.is_empty() {
        return Ok(indexed_files);
    }

    let worker_count = thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1)
        .min(files_to_process.len());
    let chunk_size = files_to_process.len().div_ceil(worker_count);

    thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in files_to_process.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let options = tokenizer_options.clone();
            handles.push(scope.spawn(move || process_chunk(chunk, tokenizer_mode, options)));
        }

        let mut rebuilt = Vec::new();
        for handle in handles {
            let output = handle
                .join()
                .map_err(|_| AppError::InvalidIndex("index worker panicked".into()))??;
            rebuilt.extend(output);
        }
        indexed_files.extend(rebuilt);
        Ok(indexed_files)
    })
}

fn process_chunk(
    files: Vec<RepositoryFile>,
    tokenizer_mode: crate::config::TokenizerMode,
    tokenizer_options: TokenizerOptions,
) -> Result<Vec<IndexedFile>> {
    let tokenizer = create(tokenizer_mode, tokenizer_options);
    let mut docs = Vec::with_capacity(files.len());

    for file in files {
        let Some(text) = read_text_file(&file.path)? else {
            continue;
        };
        let folded = fold_for_index(text);
        let mut token_hashes = Vec::new();
        tokenizer.collect_document_token_hashes(&folded, &mut token_hashes);
        token_hashes.sort_unstable();
        token_hashes.dedup();
        let fallback_token_hashes = collect_fallback_trigram_hashes(tokenizer_mode, &folded);

        docs.push(IndexedFile {
            ordinal: file.ordinal,
            path: file.path.to_string_lossy().into_owned(),
            size: file.size,
            mtime_nanos: file.mtime_nanos,
            token_hashes,
            fallback_token_hashes,
        });
    }

    Ok(docs)
}

fn usize_to_u32(value: usize, context: &str) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| AppError::ValueOutOfRange(format!("{context} exceeds u32 range")))
}

fn fold_for_index(mut text: String) -> String {
    if text.is_ascii() {
        text.make_ascii_lowercase();
        text
    } else {
        text.to_lowercase()
    }
}

struct HeadSnapshotContext<'a> {
    checkout_root: &'a Path,
    repo_root: &'a Path,
    dirty_paths: &'a HashSet<String>,
}

fn rewrite_head_snapshot_batch(
    indexed_files: &mut [IndexedFile],
    context: &HeadSnapshotContext<'_>,
) -> Result<()> {
    for file in indexed_files {
        let logical_path = repo_relative_path(Path::new(&file.path), context.checkout_root);
        let worktree_path = context.repo_root.join(&logical_path);
        if !context.dirty_paths.contains(&logical_path) {
            if let Some((size, mtime_nanos)) = worktree_file_identity(&worktree_path)? {
                file.size = size;
                file.mtime_nanos = mtime_nanos;
            }
        } else {
            file.mtime_nanos = 0;
        }
        file.path = worktree_path.to_string_lossy().into_owned();
    }
    Ok(())
}

fn cleanup_build_artifacts(layout: &IndexLayout) -> Result<()> {
    let spill_dir = spill_dir_path(layout);
    if spill_dir.exists() {
        fs::remove_dir_all(&spill_dir)?;
    }
    for path in [
        &layout.trigram_fallback_doc_terms_path,
        &layout.trigram_fallback_lookup_path,
        &layout.trigram_fallback_postings_path,
    ] {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn spill_dir_path(layout: &IndexLayout) -> PathBuf {
    layout.data_path.join("postings-spill")
}

struct StreamingIndexBuilder {
    repo_path: PathBuf,
    docs_writer: DocsWriter,
    doc_terms_writer: DocTermsWriter,
    bucket_spiller: BucketSpiller,
    fallback_doc_terms_writer: Option<DocTermsWriter>,
    fallback_bucket_spiller: Option<BucketSpiller>,
    base_spill_dir: PathBuf,
    fallback_spill_dir: Option<PathBuf>,
    doc_count: usize,
}

impl StreamingIndexBuilder {
    fn new(config: &BuildConfig, snapshot: &BaseSnapshot, layout: &IndexLayout) -> Result<Self> {
        let build_fallback_trigram = config.tokenizer == crate::config::TokenizerMode::SparseNgram;
        let docs_writer = DocsWriter::create(
            &layout.docs_path,
            IndexMetadata {
                tokenizer: config.tokenizer,
                min_sparse_len: config.min_sparse_len,
                max_sparse_len: config.max_sparse_len,
                fallback_trigram: build_fallback_trigram.then_some(FallbackTrigramSettings {
                    doc_count: 0,
                    key_count: 0,
                }),
                build: Some(snapshot.build_settings(config)),
            },
        )?;
        let doc_terms_writer = DocTermsWriter::create(&layout.doc_terms_path)?;
        let spill_root = spill_dir_path(layout);
        let base_spill_dir = spill_root.join("base");
        let bucket_spiller = BucketSpiller::create(&base_spill_dir)?;
        let (fallback_doc_terms_writer, fallback_bucket_spiller, fallback_spill_dir) =
            if build_fallback_trigram {
                let fallback_spill_dir = spill_root.join("fallback");
                (
                    Some(DocTermsWriter::create(
                        &layout.trigram_fallback_doc_terms_path,
                    )?),
                    Some(BucketSpiller::create(&fallback_spill_dir)?),
                    Some(fallback_spill_dir),
                )
            } else {
                (None, None, None)
            };
        Ok(Self {
            repo_path: config.repo_path.clone(),
            docs_writer,
            doc_terms_writer,
            bucket_spiller,
            fallback_doc_terms_writer,
            fallback_bucket_spiller,
            base_spill_dir,
            fallback_spill_dir,
            doc_count: 0,
        })
    }

    fn write_batch(&mut self, indexed_files: Vec<IndexedFile>) -> Result<()> {
        for file in indexed_files {
            let doc_id = usize_to_u32(self.doc_count, "indexed document id")?;
            self.docs_writer.write_doc(&DocMeta {
                doc_id,
                path: repo_relative_path(Path::new(&file.path), &self.repo_path),
                size: file.size,
                mtime_nanos: file.mtime_nanos,
            })?;
            self.doc_terms_writer.write_doc_terms(&file.token_hashes)?;
            self.bucket_spiller
                .write_doc_terms(&file.token_hashes, doc_id)?;
            if let Some(writer) = self.fallback_doc_terms_writer.as_mut() {
                writer.write_doc_terms(&file.fallback_token_hashes)?;
            }
            if let Some(spiller) = self.fallback_bucket_spiller.as_mut() {
                spiller.write_doc_terms(&file.fallback_token_hashes, doc_id)?;
            }
            self.doc_count += 1;
        }
        Ok(())
    }

    fn finish_with_progress(
        mut self,
        tokenizer: crate::config::TokenizerMode,
        layout: &IndexLayout,
        on_progress: &mut impl FnMut(IndexProgress),
        should_cancel: &mut impl FnMut() -> bool,
    ) -> Result<usize> {
        check_cancel(should_cancel)?;
        report_progress(
            on_progress,
            IndexProgressPhase::Finalizing,
            "Flushing streamed index metadata",
            0,
            Some(4),
        );
        self.bucket_spiller.finish()?;
        if let Some(spiller) = self.fallback_bucket_spiller.as_mut() {
            spiller.finish()?;
        }
        report_progress(
            on_progress,
            IndexProgressPhase::Finalizing,
            "Flushed postings spill buckets",
            1,
            Some(4),
        );
        let docs_count = self.doc_count;
        let doc_terms_count = self.doc_terms_writer.finish()?;
        let fallback_doc_terms_count = if let Some(writer) = self.fallback_doc_terms_writer.take() {
            writer.finish()?
        } else {
            docs_count
        };
        report_progress(
            on_progress,
            IndexProgressPhase::Finalizing,
            "Flushed docs and doc terms",
            2,
            Some(4),
        );
        if docs_count != self.doc_count
            || doc_terms_count != self.doc_count
            || fallback_doc_terms_count != self.doc_count
        {
            return Err(AppError::InvalidIndex(format!(
                "streamed doc count mismatch: docs={docs_count}, doc_terms={doc_terms_count}, fallback_doc_terms={fallback_doc_terms_count}, expected={}",
                self.doc_count,
            )));
        }

        let high_freq_doc_threshold = (tokenizer == crate::config::TokenizerMode::SparseNgram)
            .then(|| compute_high_freq_doc_threshold(self.doc_count));
        let mut lookup_entries = merge_postings_from_bucket_files_with_progress(
            &self.base_spill_dir,
            &layout.postings_path,
            high_freq_doc_threshold,
            on_progress,
            should_cancel,
        )?;
        lookup_entries.sort_unstable_by_key(|entry| entry.token_hash);
        write_lookup_file(&layout.lookup_path, &lookup_entries)?;
        if let Some(fallback_spill_dir) = self.fallback_spill_dir.as_ref() {
            let mut fallback_lookup_entries = merge_postings_from_bucket_files_with_progress(
                fallback_spill_dir,
                &layout.trigram_fallback_postings_path,
                None,
                on_progress,
                should_cancel,
            )?;
            fallback_lookup_entries.sort_unstable_by_key(|entry| entry.token_hash);
            write_lookup_file(
                &layout.trigram_fallback_lookup_path,
                &fallback_lookup_entries,
            )?;
            self.docs_writer
                .update_fallback_trigram(FallbackTrigramSettings {
                    doc_count: self.doc_count,
                    key_count: fallback_lookup_entries.len(),
                })?;
        }
        let docs_count = self.docs_writer.finish()?;
        let spill_root = spill_dir_path(layout);
        if spill_root.exists() {
            fs::remove_dir_all(&spill_root)?;
        }
        report_progress(
            on_progress,
            IndexProgressPhase::Finalizing,
            "Finished base snapshot index",
            4,
            Some(4),
        );
        Ok(docs_count)
    }
}

struct BucketSpiller {
    writers: Vec<BufWriter<File>>,
}

impl BucketSpiller {
    fn create(dir: &Path) -> Result<Self> {
        debug_assert!(INDEX_BUILD_BUCKET_COUNT.is_power_of_two());
        ensure_dir(dir)?;
        let mut writers = Vec::with_capacity(INDEX_BUILD_BUCKET_COUNT);
        for bucket in 0..INDEX_BUILD_BUCKET_COUNT {
            let path = bucket_path(dir, bucket);
            let file = File::create(path)?;
            writers.push(BufWriter::new(file));
        }
        Ok(Self { writers })
    }

    fn write_doc_terms(&mut self, token_hashes: &[u64], doc_id: u32) -> Result<()> {
        for &token_hash in token_hashes {
            let bucket = bucket_for(token_hash);
            let writer = &mut self.writers[bucket];
            writer.write_all(&token_hash.to_le_bytes())?;
            writer.write_all(&doc_id.to_le_bytes())?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        for writer in &mut self.writers {
            writer.flush()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BucketRecord {
    token_hash: u64,
    doc_id: u32,
}

fn merge_postings_from_bucket_files_with_progress(
    spill_dir: &Path,
    postings_path: &Path,
    high_freq_doc_threshold: Option<usize>,
    on_progress: &mut impl FnMut(IndexProgress),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Vec<LookupEntry>> {
    let mut postings_writer = PostingsWriter::create_streaming(postings_path)?;
    for bucket in 0..INDEX_BUILD_BUCKET_COUNT {
        check_cancel(should_cancel)?;
        report_progress(
            on_progress,
            IndexProgressPhase::Writing,
            format!(
                "Merging postings bucket {}/{}",
                bucket + 1,
                INDEX_BUILD_BUCKET_COUNT
            ),
            bucket,
            Some(INDEX_BUILD_BUCKET_COUNT),
        );
        let mut records = read_bucket_records(&bucket_path(spill_dir, bucket))?;
        records.sort_unstable_by_key(|record| (record.token_hash, record.doc_id));

        let mut index = 0usize;
        while index < records.len() {
            let token_hash = records[index].token_hash;
            let mut doc_ids = Vec::new();
            let mut last_doc_id = None;
            while index < records.len() && records[index].token_hash == token_hash {
                let doc_id = records[index].doc_id;
                if last_doc_id != Some(doc_id) {
                    doc_ids.push(doc_id);
                    last_doc_id = Some(doc_id);
                }
                index += 1;
            }
            postings_writer.write_posting_list(token_hash, &doc_ids, high_freq_doc_threshold)?;
        }
    }
    let entries = postings_writer.finish()?;
    report_progress(
        on_progress,
        IndexProgressPhase::Writing,
        "Merged postings buckets",
        INDEX_BUILD_BUCKET_COUNT,
        Some(INDEX_BUILD_BUCKET_COUNT),
    );
    Ok(entries)
}

fn read_bucket_records(path: &Path) -> Result<Vec<BucketRecord>> {
    let file = File::open(path)?;
    let byte_len = usize::try_from(file.metadata()?.len())
        .map_err(|_| AppError::ValueOutOfRange("bucket spill length exceeds usize range".into()))?;
    if byte_len % BUCKET_RECORD_BYTES != 0 {
        return Err(AppError::InvalidIndex(format!(
            "bucket spill file has invalid length: {}",
            path.display()
        )));
    }

    let mut reader = BufReader::new(file);
    let mut records = Vec::with_capacity(byte_len / BUCKET_RECORD_BYTES);
    let mut buffer = [0u8; BUCKET_RECORD_BYTES];
    loop {
        match reader.read_exact(&mut buffer) {
            Ok(()) => {
                let mut token_bytes = [0u8; 8];
                token_bytes.copy_from_slice(&buffer[..8]);
                let mut doc_bytes = [0u8; 4];
                doc_bytes.copy_from_slice(&buffer[8..]);
                records.push(BucketRecord {
                    token_hash: u64::from_le_bytes(token_bytes),
                    doc_id: u32::from_le_bytes(doc_bytes),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(records)
}

fn bucket_path(dir: &Path, bucket: usize) -> PathBuf {
    dir.join(format!("bucket-{bucket:03}.bin"))
}

fn bucket_for(token_hash: u64) -> usize {
    (token_hash as usize) & (INDEX_BUILD_BUCKET_COUNT - 1)
}

fn report_progress(
    on_progress: &mut impl FnMut(IndexProgress),
    phase: IndexProgressPhase,
    message: impl Into<String>,
    processed: usize,
    total: Option<usize>,
) {
    on_progress(IndexProgress::new(phase, message, processed, total));
}

fn check_cancel(should_cancel: &mut impl FnMut() -> bool) -> Result<()> {
    if should_cancel() {
        return Err(AppError::Cancelled);
    }
    Ok(())
}

fn load_cached_docs(config: &BuildConfig, snapshot: &BaseSnapshot) -> HashMap<String, CachedDoc> {
    let layout = IndexLayout::for_generation(&config.index_path, &snapshot.generation);
    if !layout.docs_path.exists() {
        return HashMap::new();
    }
    let Ok((metadata, docs)) = read_docs_file(&layout.docs_path) else {
        return HashMap::new();
    };
    if metadata.tokenizer != config.tokenizer
        || metadata.min_sparse_len != config.min_sparse_len
        || metadata.max_sparse_len != config.max_sparse_len
        || !build_settings_match(
            metadata.build.as_ref(),
            config,
            snapshot.head_commit.as_deref(),
            &snapshot.config_fingerprint,
        )
    {
        return HashMap::new();
    }

    let Ok(doc_terms) = read_doc_terms_file(&layout.doc_terms_path) else {
        return HashMap::new();
    };
    let fallback_doc_terms = if config.tokenizer == crate::config::TokenizerMode::SparseNgram {
        if metadata.fallback_trigram.is_none() {
            return HashMap::new();
        }
        let Ok(doc_terms) = read_doc_terms_file(&layout.trigram_fallback_doc_terms_path) else {
            return HashMap::new();
        };
        doc_terms
    } else {
        vec![Vec::new(); docs.len()]
    };
    if docs.len() != doc_terms.len() || docs.len() != fallback_doc_terms.len() {
        return HashMap::new();
    }

    docs.into_iter()
        .enumerate()
        .map(|(index, doc)| {
            (
                doc.path,
                CachedDoc {
                    size: doc.size,
                    mtime_nanos: doc.mtime_nanos,
                    token_hashes: doc_terms[index].clone(),
                    fallback_token_hashes: fallback_doc_terms[index].clone(),
                },
            )
        })
        .collect()
}

fn collect_fallback_trigram_hashes(
    tokenizer_mode: crate::config::TokenizerMode,
    text: &str,
) -> Vec<u64> {
    if tokenizer_mode != crate::config::TokenizerMode::SparseNgram {
        return Vec::new();
    }

    let bytes = text.as_bytes();
    let mut token_hashes = Vec::new();
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
        for window in bytes[start..end].windows(3) {
            token_hashes.push(u64::from(crc32fast::hash(window)));
        }
        start = end;
    }

    token_hashes.sort_unstable();
    token_hashes.dedup();
    token_hashes
}

fn is_ascii_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn build_settings_match(
    build: Option<&IndexBuildSettings>,
    config: &BuildConfig,
    expected_head_commit: Option<&str>,
    expected_config_fingerprint: &str,
) -> bool {
    let Some(build) = build else {
        return false;
    };
    build.repo_root == canonical_repo_root(&config.repo_path)
        && build.corpus_mode == config.corpus_mode
        && build.include_hidden == config.include_hidden
        && build.max_file_size == config.max_file_size
        && build.head_commit.as_deref() == expected_head_commit
        && build.config_fingerprint.as_deref() == Some(expected_config_fingerprint)
}

fn resolve_base_snapshot(
    config: &BuildConfig,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<BaseSnapshot> {
    let config_fingerprint = build_config_fingerprint(config);
    match resolve_git_head(&config.repo_path, should_cancel)? {
        Some(head_commit) => {
            let stable_generation = format!("base-git-{head_commit}-{config_fingerprint}");
            let worktree_dirty =
                git_worktree_dirty(&config.repo_path, &config.index_path, should_cancel)?;
            let generation = current_compatible_generation(
                config,
                Some(head_commit.as_str()),
                &config_fingerprint,
            )
            .unwrap_or(stable_generation);
            Ok(BaseSnapshot {
                generation,
                head_commit: Some(head_commit),
                config_fingerprint,
                git_worktree_dirty: worktree_dirty,
            })
        }
        None => {
            let repo_fingerprint = repo_fingerprint(&config.repo_path);
            let stable_generation = format!("base-repo-{repo_fingerprint}-{config_fingerprint}");
            let generation = current_compatible_generation(config, None, &config_fingerprint)
                .unwrap_or(stable_generation);
            Ok(BaseSnapshot {
                generation,
                head_commit: None,
                config_fingerprint,
                git_worktree_dirty: false,
            })
        }
    }
}

pub(crate) fn resolve_git_head(
    repo_root: &Path,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Option<String>> {
    let output = run_git_command(repo_root, ["rev-parse", "--verify", "HEAD"], should_cancel)?;
    if !output.status.success() {
        return Ok(None);
    }

    let head_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head_commit.is_empty() {
        return Ok(None);
    }
    Ok(Some(head_commit))
}

fn git_worktree_dirty(
    repo_root: &Path,
    index_path: &Path,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<bool> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args([
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--",
        ".",
    ]);
    if let Some(pathspec) = index_exclude_pathspec(repo_root, index_path) {
        command.arg(pathspec);
    }
    let output = run_command_with_cancellation(&mut command, should_cancel)?;
    if !output.status.success() {
        return Err(AppError::InvalidIndex(format!(
            "failed to inspect git worktree state for {}",
            repo_root.display()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Some(path) = line.get(3..) else {
            return Ok(true);
        };
        if line.starts_with("?? ") && is_ignored_untracked_artifact(path, repo_root, index_path) {
            continue;
        }
        return Ok(true);
    }

    Ok(false)
}

fn dirty_tracked_paths(
    repo_root: &Path,
    index_path: &Path,
    head_commit: &str,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<HashSet<String>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .args(["diff", "--name-only", "-z", head_commit, "--", "."]);
    if let Some(pathspec) = index_exclude_pathspec(repo_root, index_path) {
        command.arg(pathspec);
    }
    let output = run_command_with_cancellation(&mut command, should_cancel)?;
    if !output.status.success() {
        return Err(AppError::InvalidIndex(format!(
            "failed to diff worktree against {head_commit} for {}",
            repo_root.display()
        )));
    }

    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}

fn materialize_head_checkout(
    repo_root: &Path,
    head_commit: &str,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<MaterializedHeadCheckout> {
    let checkout = MaterializedHeadCheckout::new()?;
    fs::create_dir_all(checkout.root.join(".git"))?;
    run_git_with_temp_index(
        repo_root,
        &checkout.index_path,
        ["read-tree", head_commit],
        &format!(
            "failed to load git tree {head_commit} from {} into a temporary index",
            repo_root.display()
        ),
        should_cancel,
    )?;
    run_git_with_temp_index(
        repo_root,
        &checkout.index_path,
        [
            "checkout-index",
            "--all",
            "--force",
            "--prefix",
            checkout.prefix_arg.as_str(),
        ],
        &format!(
            "failed to materialize git tree {head_commit} into {}",
            checkout.root.display()
        ),
        should_cancel,
    )?;
    Ok(checkout)
}

fn scan_git_tracked_files(
    repo_root: &Path,
    checkout_root: &Path,
    include_hidden: bool,
    max_file_size: u64,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Vec<RepositoryFile>> {
    let paths = git_path_list(
        repo_root,
        ["ls-files", "-z", "--cached", "--", "."],
        should_cancel,
    )?;
    collect_git_repository_files(checkout_root, paths, include_hidden, max_file_size)
}

fn scan_git_head_files(
    repo_root: &Path,
    checkout_root: &Path,
    head_commit: &str,
    include_hidden: bool,
    max_file_size: u64,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Vec<RepositoryFile>> {
    let paths = git_path_list(
        repo_root,
        ["ls-tree", "-r", "-z", "--name-only", head_commit, "--", "."],
        should_cancel,
    )?;
    collect_git_repository_files(checkout_root, paths, include_hidden, max_file_size)
}

fn collect_git_repository_files(
    checkout_root: &Path,
    paths: Vec<String>,
    include_hidden: bool,
    max_file_size: u64,
) -> Result<Vec<RepositoryFile>> {
    let mut files = Vec::with_capacity(paths.len());
    for logical_path in paths {
        let path = resolve_repo_path(checkout_root, &logical_path);
        if is_workspace_internal_path(&path)
            || (!include_hidden && path_has_hidden_components(&path, checkout_root))
        {
            continue;
        }
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() || metadata.len() > max_file_size {
            continue;
        }
        let mtime_nanos = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| match u64::try_from(duration.as_nanos()) {
                Ok(value) => value,
                Err(_) => u64::MAX,
            })
            .unwrap_or_default();
        files.push(RepositoryFile {
            ordinal: files.len(),
            path,
            size: metadata.len(),
            mtime_nanos,
        });
    }
    Ok(files)
}

fn git_path_list<const N: usize>(
    repo_root: &Path,
    args: [&str; N],
    _should_cancel: &mut impl FnMut() -> bool,
) -> Result<Vec<String>> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    let output = command.output()?;
    if !output.status.success() {
        return Err(AppError::InvalidIndex(format!(
            "failed to enumerate git files for {}",
            repo_root.display()
        )));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}

fn path_has_hidden_components(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| match component {
        std::path::Component::Normal(value) => value.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

fn run_git_with_temp_index<const N: usize>(
    repo_root: &Path,
    index_path: &Path,
    args: [&str; N],
    error_message: &str,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .env("GIT_INDEX_FILE", index_path)
        .args(args);
    let output = run_command_with_cancellation(&mut command, should_cancel)?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        return Err(AppError::InvalidIndex(error_message.to_string()));
    }
    Err(AppError::InvalidIndex(format!("{error_message}: {stderr}")))
}

fn run_git_command<const N: usize>(
    repo_root: &Path,
    args: [&str; N],
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    run_command_with_cancellation(&mut command, should_cancel)
}

fn run_command_with_cancellation(
    command: &mut Command,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<Output> {
    check_cancel(should_cancel)?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    loop {
        check_child_cancelled(&mut child, should_cancel)?;
        if child.try_wait()?.is_some() {
            return Ok(child.wait_with_output()?);
        }
        thread::sleep(CANCEL_POLL_INTERVAL);
    }
}

fn check_child_cancelled(
    child: &mut Child,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<()> {
    if should_cancel() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::Cancelled);
    }
    Ok(())
}

fn worktree_file_identity(path: &Path) -> Result<Option<(u64, u64)>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mtime_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| match u64::try_from(duration.as_nanos()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        })
        .unwrap_or_default();
    Ok(Some((metadata.len(), mtime_nanos)))
}

pub(crate) fn build_config_fingerprint(config: &BuildConfig) -> String {
    let mut hasher = Hasher::new();
    hasher.update(config.tokenizer.as_str().as_bytes());
    hasher.update(config.corpus_mode.as_str().as_bytes());
    hasher.update(&[u8::from(config.include_hidden)]);
    hasher.update(&config.max_file_size.to_le_bytes());
    hasher.update(&(config.min_sparse_len as u64).to_le_bytes());
    hasher.update(&(config.max_sparse_len as u64).to_le_bytes());
    format!("{:08x}", hasher.finalize())
}

fn repo_fingerprint(repo_root: &Path) -> String {
    let mut hasher = Hasher::new();
    hasher.update(canonical_repo_root(repo_root).as_bytes());
    format!("{:08x}", hasher.finalize())
}

fn current_compatible_generation(
    config: &BuildConfig,
    expected_head_commit: Option<&str>,
    expected_config_fingerprint: &str,
) -> Option<String> {
    let current_path = IndexLayout::current_path(&config.index_path);
    let generation = fs::read_to_string(current_path).ok()?;
    let generation = generation.trim();
    if generation.is_empty() {
        return None;
    }

    reusable_generation_doc_count(
        config,
        generation,
        expected_head_commit,
        expected_config_fingerprint,
    )
    .map(|_| generation.to_string())
}

fn fresh_generation_name(base_generation: &str) -> String {
    let stable_generation = base_generation
        .rsplit_once("-r")
        .and_then(|(prefix, suffix)| {
            suffix
                .chars()
                .all(|value| value.is_ascii_hexdigit())
                .then_some(prefix)
        })
        .unwrap_or(base_generation);
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{stable_generation}-r{now_nanos:x}")
}

fn reusable_generation_doc_count(
    config: &BuildConfig,
    generation: &str,
    expected_head_commit: Option<&str>,
    expected_config_fingerprint: &str,
) -> Option<usize> {
    let layout = IndexLayout::for_generation(&config.index_path, generation);
    let (metadata, doc_count) = read_docs_header(&layout.docs_path).ok()?;
    if metadata.tokenizer != config.tokenizer
        || metadata.min_sparse_len != config.min_sparse_len
        || metadata.max_sparse_len != config.max_sparse_len
        || !build_settings_match(
            metadata.build.as_ref(),
            config,
            expected_head_commit,
            expected_config_fingerprint,
        )
    {
        return None;
    }
    validate_index_layout(&layout, &metadata, doc_count).ok()?;
    Some(doc_count)
}

pub(crate) fn canonical_repo_root(repo_root: &Path) -> String {
    std::fs::canonicalize(repo_root)
        .unwrap_or_else(|_| repo_root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn index_exclude_pathspec(repo_root: &Path, index_path: &Path) -> Option<String> {
    let relative = index_path.strip_prefix(repo_root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    let path = relative.to_string_lossy().replace('\\', "/");
    Some(format!(":(exclude){path}"))
}

fn is_ignored_untracked_artifact(path: &str, repo_root: &Path, index_path: &Path) -> bool {
    let normalized = path.replace('\\', "/");
    if let Some(relative_index_path) = index_path.strip_prefix(repo_root).ok() {
        let relative_index_path = relative_index_path.to_string_lossy().replace('\\', "/");
        if normalized == relative_index_path
            || normalized.starts_with(&format!("{relative_index_path}/"))
        {
            return true;
        }
    }

    normalized
        .split('/')
        .any(|component| component == ".codgrep-index" || component == ".codgrep-bench")
}

struct BaseSnapshot {
    generation: String,
    head_commit: Option<String>,
    config_fingerprint: String,
    git_worktree_dirty: bool,
}

impl BaseSnapshot {
    fn build_settings(&self, config: &BuildConfig) -> IndexBuildSettings {
        IndexBuildSettings {
            repo_root: canonical_repo_root(&config.repo_path),
            corpus_mode: config.corpus_mode,
            include_hidden: config.include_hidden,
            max_file_size: config.max_file_size,
            head_commit: self.head_commit.clone(),
            config_fingerprint: Some(self.config_fingerprint.clone()),
        }
    }
}

struct MaterializedHeadCheckout {
    root: PathBuf,
    index_path: PathBuf,
    prefix_arg: String,
}

impl MaterializedHeadCheckout {
    fn new() -> Result<Self> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!(
            "codgrep-head-{}-{}-{}",
            std::process::id(),
            now.as_secs(),
            now.subsec_nanos()
        ));
        fs::create_dir_all(&root)?;
        let index_path = root.join("head.index");
        let prefix_arg = format!("{}/", root.to_string_lossy());
        Ok(Self {
            root,
            index_path,
            prefix_arg,
        })
    }
}

impl Drop for MaterializedHeadCheckout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, Clone)]
struct IndexedFile {
    ordinal: usize,
    path: String,
    size: u64,
    mtime_nanos: u64,
    token_hashes: Vec<u64>,
    fallback_token_hashes: Vec<u64>,
}

struct CachedDoc {
    size: u64,
    mtime_nanos: u64,
    token_hashes: Vec<u64>,
    fallback_token_hashes: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::TempDir;

    use super::build_index;
    use crate::{
        config::{BuildConfig, CorpusMode, TokenizerMode},
        experimental::index_format::read_docs_file,
        index::IndexSearcher,
    };

    #[test]
    fn git_build_indexes_tracked_files_even_if_gitignored() {
        let temp = TempDir::new().expect("temp dir");
        let repo = temp.path().join("repo");
        let index = repo.join(".bitfun").join("search").join("codgrep-index");
        fs::create_dir_all(&repo).expect("create repo dir");
        run_git(&repo, ["init"]);
        run_git(&repo, ["config", "user.name", "Codgrep Test"]);
        run_git(&repo, ["config", "user.email", "codgrep@example.com"]);

        fs::write(repo.join(".gitignore"), "tracked-ignored.txt\n").expect("write gitignore");
        fs::write(repo.join("tracked-ignored.txt"), "tracked ignored text\n")
            .expect("write tracked ignored");
        fs::write(repo.join("visible.txt"), "visible text\n").expect("write visible");
        run_git(&repo, ["add", ".gitignore", "visible.txt"]);
        run_git(&repo, ["add", "-f", "tracked-ignored.txt"]);
        run_git(&repo, ["commit", "-m", "init"]);

        let config = BuildConfig {
            repo_path: repo.clone(),
            index_path: index.clone(),
            tokenizer: TokenizerMode::Trigram,
            corpus_mode: CorpusMode::RespectIgnore,
            include_hidden: false,
            max_file_size: 1 << 20,
            min_sparse_len: 3,
            max_sparse_len: 64,
        };
        build_index(&config).expect("build index");

        let current = fs::read_to_string(index.join("CURRENT")).expect("read current");
        let docs_path = index
            .join("generations")
            .join(current.trim())
            .join("docs.bin");
        let (_meta, docs) = read_docs_file(&docs_path).expect("read docs");
        assert!(
            docs.iter().any(|doc| doc.path == "tracked-ignored.txt"),
            "tracked gitignored file should be indexed"
        );
        assert!(
            docs.iter().any(|doc| doc.path == "visible.txt"),
            "visible tracked file should be indexed"
        );

        let searcher = IndexSearcher::open(index).expect("open searcher");
        let diff = searcher
            .diff_against_worktree()
            .expect("diff against worktree");
        assert!(
            diff.is_empty(),
            "clean git repo should not look dirty: {diff:?}"
        );
    }

    fn run_git<const N: usize>(repo: &Path, args: [&str; N]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git command failed");
    }
}
