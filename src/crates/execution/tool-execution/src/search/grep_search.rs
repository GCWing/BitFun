use crate::util::string::shell_single_quote;
use log::{debug, info, warn};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use globset::{GlobBuilder, GlobMatcher};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::types::TypesBuilder;
use ignore::{DirEntry, WalkBuilder, WalkState};

const MAX_DISPLAY_COLUMNS: usize = 500;
const MAX_VIRTUAL_GREP_CONTENT_LINES: usize = 4096;
const VCS_DIRECTORIES_TO_EXCLUDE: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// Output mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl std::str::FromStr for OutputMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "content" => Ok(OutputMode::Content),
            "count" => Ok(OutputMode::Count),
            "files_with_matches" => Ok(OutputMode::FilesWithMatches),
            _ => Err(format!("Unknown output mode: {}", s)),
        }
    }
}

impl fmt::Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputMode::Content => write!(f, "content"),
            OutputMode::Count => write!(f, "count"),
            OutputMode::FilesWithMatches => write!(f, "files_with_matches"),
        }
    }
}

/// Sink implementation for collecting search results
#[derive(Clone)]
struct GrepSink {
    output_mode: OutputMode,
    show_line_numbers: bool,
    before_context: usize,
    after_context: usize,
    head_limit: Option<usize>,
    current_file: PathBuf,
    display_base: Option<String>,
    output: Arc<Mutex<Vec<String>>>,
    line_count: Arc<Mutex<usize>>,
    match_count: Arc<Mutex<usize>>,
    /// Last output line number, used to detect discontinuity
    last_line_number: Arc<Mutex<Option<u64>>>,
}

fn lock_recover<'a, T>(mutex: &'a Mutex<T>, name: &str) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("Mutex poisoned in grep search: {}", name);
            poisoned.into_inner()
        }
    }
}

impl GrepSink {
    fn new(
        output_mode: OutputMode,
        show_line_numbers: bool,
        before_context: usize,
        after_context: usize,
        head_limit: Option<usize>,
        current_file: PathBuf,
        display_base: Option<String>,
    ) -> Self {
        Self {
            output_mode,
            show_line_numbers,
            before_context,
            after_context,
            head_limit,
            current_file,
            display_base,
            output: Arc::new(Mutex::new(Vec::new())),
            line_count: Arc::new(Mutex::new(0)),
            match_count: Arc::new(Mutex::new(0)),
            last_line_number: Arc::new(Mutex::new(None)),
        }
    }

    /// Takes the collected output lines, avoiding a split/realloc/join round
    /// trip at the call site.
    fn take_output_lines(&self) -> Vec<String> {
        std::mem::take(&mut *lock_recover(&self.output, "output"))
    }

    fn get_match_count(&self) -> usize {
        *lock_recover(&self.match_count, "match_count")
    }

    fn should_stop(&self) -> bool {
        if let Some(limit) = self.head_limit {
            let count = *lock_recover(&self.line_count, "line_count");
            count >= limit
        } else {
            false
        }
    }

    fn increment_line_count(&self) -> bool {
        let mut count = lock_recover(&self.line_count, "line_count");
        *count += 1;
        if let Some(limit) = self.head_limit {
            *count <= limit
        } else {
            true
        }
    }

    fn write_line(&self, line: String) {
        if self.increment_line_count() {
            let mut output = lock_recover(&self.output, "output");
            output.push(line);
        }
    }

    /// Check if separator (--) needs to be inserted before current line
    /// Insert when previous line and current line are not continuous (only when context is set)
    fn check_and_write_separator(&self, current_line: u64) {
        // Only use separator when context is set (consistent with rg behavior)
        if self.before_context == 0 && self.after_context == 0 {
            return;
        }

        let mut last_line = lock_recover(&self.last_line_number, "last_line_number");
        if let Some(last) = *last_line {
            // If current line number is not continuous with previous line (difference > 1), insert separator
            if current_line > last + 1 {
                let mut output = lock_recover(&self.output, "output");
                output.push("--".to_string());
            }
        }
        *last_line = Some(current_line);
    }

    /// Format output line (rg style: only show line number and content, no path)
    fn format_line(&self, line_number: u64, line: &[u8], is_match: bool) -> String {
        let mut line_str = String::from_utf8_lossy(line).trim_end().to_string();
        if line_str.chars().count() > MAX_DISPLAY_COLUMNS {
            line_str = format!(
                "{} [truncated]",
                line_str
                    .chars()
                    .take(MAX_DISPLAY_COLUMNS)
                    .collect::<String>()
            );
        }
        let separator = if is_match { ":" } else { "-" };
        let path_prefix = relativize_display_path(&self.current_file, self.display_base.as_deref());

        if self.show_line_numbers {
            format!("{}{}{}:{}", path_prefix, separator, line_number, line_str)
        } else {
            format!("{}{}{}", path_prefix, separator, line_str)
        }
    }
}

impl Sink for GrepSink {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.should_stop() {
            return Ok(false);
        }

        *lock_recover(&self.match_count, "match_count") += 1;

        match self.output_mode {
            OutputMode::Content => {
                let line_number = mat.line_number().unwrap_or(0);
                // Check if separator needs to be inserted
                self.check_and_write_separator(line_number);
                let formatted = self.format_line(line_number, mat.bytes(), true);
                self.write_line(formatted);
            }
            OutputMode::FilesWithMatches => {
                return Ok(false); // Only need first match, then stop
            }
            OutputMode::Count => {
                // Count mode doesn't write here, handled uniformly at the end
            }
        }

        Ok(!self.should_stop())
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        if self.should_stop() {
            return Ok(false);
        }

        // Only output context lines in content mode and when context is set
        if matches!(self.output_mode, OutputMode::Content)
            && (self.before_context > 0 || self.after_context > 0)
        {
            let line_number = ctx.line_number().unwrap_or(0);
            // Check if separator needs to be inserted
            self.check_and_write_separator(line_number);
            let formatted = self.format_line(line_number, ctx.bytes(), false);
            self.write_line(formatted);
        }

        Ok(!self.should_stop())
    }

    fn begin(&mut self, _searcher: &Searcher) -> Result<bool, Self::Error> {
        Ok(!self.should_stop())
    }

    fn finish(
        &mut self,
        _searcher: &Searcher,
        _: &grep_searcher::SinkFinish,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Progress report callback type
pub type ProgressCallback = Arc<dyn Fn(usize, usize, usize) + Send + Sync>;

/// Cooperative cancellation for an in-flight [`grep_search`].
///
/// Cheap to clone and share: the walker threads, the per-worker searchers and the reducer all hold
/// the same flag. Cancelling is one-way — a cancelled search never becomes runnable again, so a
/// caller that wants to retry builds a fresh token.
///
/// The contract is *cooperative*, not preemptive: a cancel is observed between filesystem entries,
/// so a single very large file still finishes being searched. What it bounds is the walk, which is
/// where the wall-clock actually goes on a large repository.
#[derive(Debug, Clone, Default)]
pub struct SearchCancellation {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl SearchCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the search to stop. Idempotent, and safe to call from any thread.
    pub fn cancel(&self) {
        // Relaxed is enough: the flag guards no other data, and every reader is a plain poll whose
        // only requirement is that the store eventually becomes visible.
        self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// grep search options
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Regular expression pattern
    pub pattern: String,
    /// Search path
    pub path: String,
    /// Whether to ignore case
    pub case_insensitive: bool,
    /// Whether to enable multiline mode
    pub multiline: bool,
    /// Output mode
    pub output_mode: OutputMode,
    /// Whether to show line numbers
    pub show_line_numbers: bool,
    /// Context line count (sets both before and after)
    pub context: Option<usize>,
    /// Context lines before match
    pub before_context: Option<usize>,
    /// Context lines after match
    pub after_context: Option<usize>,
    /// Limit output lines/files
    pub head_limit: Option<usize>,
    /// Number of lines/files to skip before limiting output
    pub offset: usize,
    /// Glob pattern filters
    pub globs: Vec<String>,
    /// File type filter
    pub file_type: Option<String>,
    /// Prefer displaying paths relative to this base when possible
    pub display_base: Option<String>,
    /// Exact files omitted from this search by the caller's scope policy.
    pub excluded_paths: Vec<String>,
    /// Reject linked file entries when the caller requires workspace identity.
    pub reject_linked_files: bool,
    /// Cooperative cancellation. `None` means the search cannot be cancelled.
    pub cancellation: Option<SearchCancellation>,
}

impl Default for GrepOptions {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            path: String::from("."),
            case_insensitive: false,
            multiline: false,
            output_mode: OutputMode::Content,
            show_line_numbers: true,
            context: None,
            before_context: None,
            after_context: None,
            head_limit: None,
            offset: 0,
            globs: Vec::new(),
            file_type: None,
            display_base: None,
            excluded_paths: Vec::new(),
            reject_linked_files: false,
            cancellation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteGrepCommandRequest {
    pub pattern: String,
    pub path: String,
    pub case_insensitive: bool,
    pub output_mode: OutputMode,
    pub show_line_numbers: bool,
    pub context: Option<usize>,
    pub before_context: Option<usize>,
    pub after_context: Option<usize>,
    pub glob_patterns: Vec<String>,
    pub file_type: Option<String>,
    pub head_limit: Option<usize>,
    pub offset: usize,
}

pub fn build_remote_grep_command(request: &RemoteGrepCommandRequest) -> String {
    let offset_cmd = if request.offset > 0 {
        format!(" | tail -n +{}", request.offset + 1)
    } else {
        String::new()
    };
    let limit_cmd = request
        .head_limit
        .map(|limit| format!(" | head -n {}", limit))
        .unwrap_or_default();

    let mut cmd = "rg --no-heading --hidden --max-columns 500".to_string();
    if request.case_insensitive {
        cmd.push_str(" -i");
    }
    if request.output_mode == OutputMode::FilesWithMatches {
        cmd.push_str(" -l");
    } else if request.output_mode == OutputMode::Count {
        cmd.push_str(" -c");
    } else if request.show_line_numbers {
        cmd.push_str(" --line-number");
    }
    if request.output_mode == OutputMode::Content {
        if let Some(context) = request.context {
            cmd.push_str(&format!(" -C {}", context));
        } else {
            if let Some(before) = request.before_context {
                cmd.push_str(&format!(" -B {}", before));
            }
            if let Some(after) = request.after_context {
                cmd.push_str(&format!(" -A {}", after));
            }
        }
    }
    for glob_pattern in &request.glob_patterns {
        cmd.push_str(&format!(" --glob {}", shell_single_quote(glob_pattern)));
    }
    if let Some(file_type) = &request.file_type {
        cmd.push_str(&format!(" --type {}", shell_single_quote(file_type)));
    }
    cmd.push_str(&format!(
        " -e {} {} 2>/dev/null{}{}",
        shell_single_quote(&request.pattern),
        shell_single_quote(&request.path),
        offset_cmd,
        limit_cmd
    ));

    format!(
        "if command -v rg >/dev/null 2>&1; then {}; else grep -rn{} -e {} {} 2>/dev/null{}{}; fi",
        cmd,
        if request.case_insensitive { "i" } else { "" },
        shell_single_quote(&request.pattern),
        shell_single_quote(&request.path),
        offset_cmd,
        limit_cmd,
    )
}

pub fn count_remote_grep_matches(stdout: &str) -> usize {
    stdout.lines().count()
}

pub fn relativize_result_text(result_text: &str, display_base: Option<&str>) -> String {
    let Some(base) = display_base else {
        return result_text.to_string();
    };

    let normalized_base = base.replace('\\', "/").trim_end_matches('/').to_string();
    if normalized_base.is_empty() {
        return result_text.to_string();
    }

    result_text
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix(&(normalized_base.clone() + "/")) {
                rest.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_remote_grep_result_text(
    stdout: &str,
    pattern: &str,
    display_base: Option<&str>,
) -> String {
    if stdout.lines().next().is_none() {
        format!("No matches found for pattern '{}'", pattern)
    } else {
        relativize_result_text(stdout, display_base)
    }
}

pub fn apply_offset_and_limit(items: &mut Vec<String>, offset: usize, head_limit: Option<usize>) {
    if offset > 0 {
        if offset >= items.len() {
            items.clear();
        } else {
            *items = items[offset..].to_vec();
        }
    }

    if let Some(limit) = head_limit {
        if items.len() > limit {
            items.truncate(limit);
        }
    }
}

impl GrepOptions {
    /// Create a new GrepOptions with required pattern and path
    pub fn new(pattern: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            path: path.into(),
            ..Default::default()
        }
    }

    /// Set whether to ignore case
    pub fn case_insensitive(mut self, value: bool) -> Self {
        self.case_insensitive = value;
        self
    }

    /// Attach a cancellation token so the caller can stop this search early.
    pub fn cancellation(mut self, value: SearchCancellation) -> Self {
        self.cancellation = Some(value);
        self
    }

    /// Set whether to enable multiline mode
    pub fn multiline(mut self, value: bool) -> Self {
        self.multiline = value;
        self
    }

    /// Set output mode
    pub fn output_mode(mut self, mode: OutputMode) -> Self {
        self.output_mode = mode;
        self
    }

    /// Set whether to show line numbers
    pub fn show_line_numbers(mut self, value: bool) -> Self {
        self.show_line_numbers = value;
        self
    }

    pub fn excluded_paths(mut self, paths: Vec<String>) -> Self {
        self.excluded_paths = paths;
        self
    }

    pub fn reject_linked_files(mut self, reject: bool) -> Self {
        self.reject_linked_files = reject;
        self
    }

    /// Set context line count (sets both before and after)
    pub fn context(mut self, lines: usize) -> Self {
        self.context = Some(lines);
        self
    }

    /// Set context lines before match
    pub fn before_context(mut self, lines: usize) -> Self {
        self.before_context = Some(lines);
        self
    }

    /// Set context lines after match
    pub fn after_context(mut self, lines: usize) -> Self {
        self.after_context = Some(lines);
        self
    }

    /// Set output lines/files limit
    pub fn head_limit(mut self, limit: usize) -> Self {
        self.head_limit = Some(limit);
        self
    }

    /// Set glob pattern filter
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn globs(mut self, patterns: Vec<String>) -> Self {
        self.globs = patterns;
        self
    }

    /// Set file type filter
    pub fn file_type(mut self, ftype: impl Into<String>) -> Self {
        self.file_type = Some(ftype.into());
        self
    }

    pub fn display_base(mut self, base: impl Into<String>) -> Self {
        self.display_base = Some(base.into());
        self
    }
}

/// Execute grep search
///
/// # Parameters
/// - `options`: Search options
/// - `progress_callback`: Progress callback (optional)
/// - `progress_interval_millis`: Progress report interval (milliseconds, optional, default 500)
///
/// # Returns
/// - `Ok((file_count, match_count, result_text))`: Number of matching files, number of matches, and result text
/// - `Err(error_message)`: Error message
///
/// # Example
/// ```ignore
/// use tool_runtime::search::{grep_search, GrepOptions, OutputMode};
///
/// let options = GrepOptions::new("pattern", "path/to/search")
///     .case_insensitive(true)
///     .context(2);
///
/// let result = grep_search(options, None, None);
/// ```
pub struct GrepSearchResult {
    pub file_count: usize,
    pub total_matches: usize,
    pub result_text: String,
    pub applied_limit: Option<usize>,
    pub applied_offset: Option<usize>,
    /// The search stopped early because its [`SearchCancellation`] fired. Everything already
    /// aggregated is still returned — it is a correct result over the subset of the tree that was
    /// walked, not over the whole tree — so callers that care about completeness must check this.
    pub cancelled: bool,
}

#[derive(Clone)]
struct GrepWorkerConfig {
    output_mode: OutputMode,
    show_line_numbers: bool,
    before_context: usize,
    after_context: usize,
    display_base: Option<String>,
    globs: Vec<GlobMatcher>,
    excluded_paths: Vec<String>,
    reject_linked_files: bool,
}

struct GrepFileResult {
    path: PathBuf,
    file_matches: usize,
    output_lines: Vec<String>,
    modified_time: SystemTime,
}

enum GrepWorkerEvent {
    Processed(Option<GrepFileResult>),
    Error(String),
}

fn build_grep_searcher(before_context: usize, after_context: usize, multiline: bool) -> Searcher {
    let mut builder = SearcherBuilder::new();
    builder
        .line_number(true)
        .before_context(before_context)
        .after_context(after_context);
    if multiline {
        builder.multi_line(true);
    }
    builder.build()
}

fn search_entry(
    entry: Result<DirEntry, ignore::Error>,
    searcher: &mut Searcher,
    matcher: &RegexMatcher,
    config: &GrepWorkerConfig,
) -> GrepWorkerEvent {
    let entry = match entry {
        Ok(entry) => entry,
        Err(error) => return GrepWorkerEvent::Error(format!("Error walking files: {error}")),
    };
    let path = entry.path();

    let entry_file_type = entry.file_type();
    let is_file = entry_file_type
        .is_some_and(|file_type| file_type.is_file() || (file_type.is_symlink() && path.is_file()));
    if !is_file {
        return GrepWorkerEvent::Processed(None);
    }

    let path_is_symlink = entry_file_type.is_some_and(|file_type| file_type.is_symlink());
    let path_has_multiple_hard_links =
        config.reject_linked_files && crate::fs::path_has_multiple_hard_links(path).unwrap_or(true);
    if config.reject_linked_files && (path_is_symlink || path_has_multiple_hard_links) {
        return GrepWorkerEvent::Processed(None);
    }
    if config
        .excluded_paths
        .iter()
        .any(|excluded| paths_equal_for_exclusion(path, excluded))
        || is_vcs_path(path)
        || (!config.globs.is_empty() && !config.globs.iter().any(|glob| glob.is_match(path)))
    {
        return GrepWorkerEvent::Processed(None);
    }

    let sink = GrepSink::new(
        config.output_mode,
        config.show_line_numbers,
        config.before_context,
        config.after_context,
        None,
        path.to_path_buf(),
        config.display_base.clone(),
    );
    if let Err(error) = searcher.search_path(matcher, path, sink.clone()) {
        return GrepWorkerEvent::Error(format!("Error searching file {}: {error}", path.display()));
    }

    let file_matches = sink.get_match_count();
    if file_matches == 0 {
        return GrepWorkerEvent::Processed(None);
    }
    let output_lines = if config.output_mode == OutputMode::Content {
        sink.take_output_lines()
    } else {
        Vec::new()
    };
    GrepWorkerEvent::Processed(Some(GrepFileResult {
        path: path.to_path_buf(),
        file_matches,
        output_lines,
        modified_time: modified_time(path),
    }))
}

fn is_vcs_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if VCS_DIRECTORIES_TO_EXCLUDE
                    .iter()
                    .any(|excluded| name.to_string_lossy() == *excluded)
        )
    })
}

fn modified_time(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn normalize_display_base(base: &str) -> String {
    base.replace('\\', "/").trim_end_matches('/').to_string()
}

fn relativize_display_path(path: &Path, display_base: Option<&str>) -> String {
    let normalized = path.display().to_string().replace('\\', "/");
    let Some(base) = display_base else {
        return normalized;
    };

    let normalized_base = normalize_display_base(base);
    if normalized == normalized_base {
        return ".".to_string();
    }

    if let Some(rest) = normalized.strip_prefix(&(normalized_base + "/")) {
        return rest.to_string();
    }

    normalized
}

fn apply_offset_limit<T>(
    items: Vec<T>,
    limit: Option<usize>,
    offset: usize,
) -> (Vec<T>, Option<usize>, Option<usize>)
where
    T: Clone,
{
    let total_len = items.len();
    let sliced = match limit {
        Some(limit) => items
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>(),
        None => items.into_iter().skip(offset).collect::<Vec<_>>(),
    };

    let applied_limit = match limit {
        Some(limit) if total_len.saturating_sub(offset) > limit => Some(limit),
        _ => None,
    };
    let applied_offset = if offset > 0 { Some(offset) } else { None };

    (sliced, applied_limit, applied_offset)
}

pub fn grep_search(
    options: GrepOptions,
    progress_callback: Option<ProgressCallback>,
    progress_interval_millis: Option<u128>,
) -> Result<GrepSearchResult, String> {
    let search_path = &options.path;

    // Validate that search path exists
    let path = std::path::Path::new(search_path);
    if !path.exists() {
        return Err(format!("Search path '{}' does not exist", search_path));
    }

    let cancellation = options.cancellation.clone();
    let is_cancelled = move || {
        cancellation
            .as_ref()
            .is_some_and(SearchCancellation::is_cancelled)
    };
    if is_cancelled() {
        return Ok(GrepSearchResult {
            file_count: 0,
            total_matches: 0,
            result_text: String::new(),
            applied_limit: None,
            applied_offset: None,
            cancelled: true,
        });
    }

    let before_context = options
        .before_context
        .unwrap_or(options.context.unwrap_or(0));
    let after_context = options
        .after_context
        .unwrap_or(options.context.unwrap_or(0));
    let pattern = &options.pattern;
    let case_insensitive = options.case_insensitive;
    let multiline = options.multiline;
    let output_mode = options.output_mode;
    let show_line_numbers = options.show_line_numbers;
    let head_limit = options.head_limit;
    let offset = options.offset;
    let file_type = options.file_type.as_deref();
    let display_base = options.display_base.clone();

    // Build regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline)
        .build(pattern)
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;

    // Build walker
    let mut walk_builder = WalkBuilder::new(search_path);
    walk_builder
        .hidden(false) // Include hidden files, closer to Claude's rg --hidden
        .ignore(true) // Use .gitignore
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    // Add file type filter
    let mut types_builder = TypesBuilder::new();
    types_builder.add_defaults();

    types_builder
        .add("arkts", "*.ets")
        .map_err(|e| format!("Failed to add arkts type: {}", e))?;
    types_builder
        .add("json", "*.json5")
        .map_err(|e| format!("Failed to add json5 type: {}", e))?;

    if let Some(ftype) = file_type {
        // Check if type already exists
        let type_exists = types_builder
            .definitions()
            .iter()
            .any(|def| def.name() == ftype);

        if !type_exists {
            // Type doesn't exist, automatically add *.{ftype}
            let glob_pattern = format!("*.{}", ftype);
            types_builder
                .add(ftype, &glob_pattern)
                .map_err(|e| format!("Failed to add file type '{}': {}", ftype, e))?;
            debug!(
                "Auto-added file type '{}' with glob '{}'",
                ftype, glob_pattern
            );
        }

        // User specified type, use user-specified type
        types_builder.select(ftype);
    } else {
        types_builder.select("all");
    }

    match types_builder.build() {
        Ok(types) => {
            walk_builder.types(types);
        }
        Err(e) => {
            return Err(format!("Invalid file type: {}", e));
        }
    }

    // Pre-build glob matcher
    let glob_matchers = options
        .globs
        .iter()
        .map(|glob| {
            GlobBuilder::new(glob)
                .build()
                .map(|compiled| compiled.compile_matcher())
                .map_err(|e| format!("Invalid glob pattern: {}", e))
        })
        .collect::<Result<Vec<GlobMatcher>, String>>()?;

    let worker_config = GrepWorkerConfig {
        output_mode,
        show_line_numbers,
        before_context,
        after_context,
        display_base,
        globs: glob_matchers,
        excluded_paths: options.excluded_paths.clone(),
        reject_linked_files: options.reject_linked_files,
    };
    let worker_count =
        std::thread::available_parallelism().map_or(1, |parallelism| parallelism.get().min(8));
    walk_builder.threads(worker_count);
    let walker = walk_builder.build_parallel();

    // Collect all results
    let mut total_matches = 0;
    let mut file_count = 0;
    let (event_sender, event_receiver) = std::sync::mpsc::sync_channel(worker_count * 4);
    let worker_matcher = matcher.clone();
    let reducer_config = worker_config.clone();
    let walker_cancellation = options.cancellation.clone();
    let walker_thread = std::thread::spawn(move || {
        let sender = event_sender;
        walker.run(move || {
            let sender = sender.clone();
            let matcher = worker_matcher.clone();
            let config = worker_config.clone();
            let cancellation = walker_cancellation.clone();
            let mut searcher =
                build_grep_searcher(config.before_context, config.after_context, multiline);
            Box::new(move |entry| {
                // Checked before the search, not after: the point of cancelling is to stop paying
                // for work, and the per-entry search is the expensive half.
                if cancellation
                    .as_ref()
                    .is_some_and(SearchCancellation::is_cancelled)
                {
                    return WalkState::Quit;
                }
                let event = search_entry(entry, &mut searcher, &matcher, &config);
                if sender.send(event).is_err() {
                    WalkState::Quit
                } else {
                    WalkState::Continue
                }
            })
        });
    });

    // Progress tracking
    let mut files_processed = 0;
    let mut last_progress_time = std::time::Instant::now();
    let progress_interval_millis = progress_interval_millis.unwrap_or(500);
    let mut file_results = Vec::new();

    let mut cancelled = false;
    for event in event_receiver {
        if is_cancelled() {
            // Stop consuming and let the receiver drop. Every worker's next `send` then fails,
            // which is what unwinds the walk without needing a second signalling path.
            cancelled = true;
            break;
        }
        match event {
            GrepWorkerEvent::Processed(result) => {
                files_processed += 1;
                if let Some(result) = result {
                    file_count += 1;
                    total_matches += result.file_matches;
                    file_results.push(result);
                }
            }
            GrepWorkerEvent::Error(error) => warn!("{}", error),
        }

        if last_progress_time.elapsed().as_millis() >= progress_interval_millis {
            info!(
                "Search progress: processed {} files, found {} matching files, total {} matches",
                files_processed, file_count, total_matches
            );

            if let Some(ref callback) = progress_callback {
                callback(files_processed, file_count, total_matches);
            }

            last_progress_time = std::time::Instant::now();
        }
    }
    // The receiver was moved into the `for` loop above and is dropped as that loop exits, break
    // included — so by the time we join, the workers' sends are already failing.
    if walker_thread.join().is_err() {
        warn!("Parallel search walker thread panicked");
    }
    // The walk can also observe the cancel first and quit on its own, which closes the channel and
    // ends the loop above without setting the flag there.
    let cancelled = cancelled || is_cancelled();

    // Worker completion order is nondeterministic. Stable path order keeps Content and Count
    // output reproducible; FilesWithMatches applies its existing mtime ordering below.
    file_results.sort_by(|left, right| left.path.cmp(&right.path));
    let mut content_lines: Vec<String> =
        Vec::with_capacity(head_limit.map_or(256, |limit| limit.min(4096)));
    let mut file_match_counts: Vec<(String, usize)> = Vec::new();
    let mut matched_files_with_mtime: Vec<(String, SystemTime)> = Vec::new();
    for result in file_results {
        let display_path =
            relativize_display_path(&result.path, reducer_config.display_base.as_deref());
        match output_mode {
            OutputMode::Content => {
                for line in result.output_lines {
                    // In multi-line mode a single sink write can span several physical lines;
                    // keep one entry per physical line so head_limit/offset paginate by line.
                    if line.contains('\n') {
                        content_lines.extend(
                            line.lines()
                                .filter(|part| !part.is_empty())
                                .map(str::to_string),
                        );
                    } else if !line.is_empty() {
                        content_lines.push(line);
                    }
                }
            }
            OutputMode::FilesWithMatches => {
                matched_files_with_mtime.push((display_path, result.modified_time));
            }
            OutputMode::Count => file_match_counts.push((display_path, result.file_matches)),
        }
    }

    // Build result
    let result_text = match output_mode {
        OutputMode::Content => {
            let (lines, applied_limit, applied_offset) =
                apply_offset_limit(content_lines, head_limit, offset);
            if lines.is_empty() {
                format!("No matches found for pattern '{}'", pattern)
            } else {
                return Ok(GrepSearchResult {
                    file_count,
                    total_matches,
                    // join() never yields a trailing newline, no trim needed.
                    result_text: lines.join("\n"),
                    applied_limit,
                    applied_offset,
                    cancelled,
                });
            }
        }
        OutputMode::FilesWithMatches => {
            matched_files_with_mtime
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            let sorted_matches = matched_files_with_mtime
                .into_iter()
                .map(|(path, _)| path)
                .collect::<Vec<_>>();
            let (matches, applied_limit, applied_offset) =
                apply_offset_limit(sorted_matches, head_limit, offset);

            if matches.is_empty() {
                format!("No files found matching pattern '{}'", pattern)
            } else {
                return Ok(GrepSearchResult {
                    file_count,
                    total_matches,
                    result_text: matches.join("\n").trim_end_matches('\n').to_string(),
                    applied_limit,
                    applied_offset,
                    cancelled,
                });
            }
        }
        OutputMode::Count => {
            if file_match_counts.is_empty() {
                format!("No matches found for pattern '{}'", pattern)
            } else {
                let (count_list, applied_limit, applied_offset) =
                    apply_offset_limit(file_match_counts, head_limit, offset);

                let count_lines: Vec<String> = count_list
                    .iter()
                    .map(|(file, count)| format!("{}:{}", file, count))
                    .collect();

                return Ok(GrepSearchResult {
                    file_count,
                    total_matches,
                    result_text: format!(
                        "Total {} matches in {} files:\n{}",
                        total_matches,
                        count_list.len(),
                        count_lines.join("\n")
                    )
                    .trim_end_matches('\n')
                    .to_string(),
                    applied_limit,
                    applied_offset,
                    cancelled,
                });
            }
        }
    };

    Ok(GrepSearchResult {
        file_count,
        total_matches,
        result_text: result_text.trim_end_matches('\n').to_string(),
        applied_limit: None,
        applied_offset: if offset > 0 { Some(offset) } else { None },
        cancelled,
    })
}

/// Search immutable in-memory text files with the same matcher and result
/// presentation used by filesystem Grep.
///
/// This is used for capability-backed virtual files whose contents must not be
/// reopened through a mutable filesystem path.
pub fn grep_search_virtual_files(
    options: GrepOptions,
    files: &[(String, Arc<str>)],
) -> Result<GrepSearchResult, String> {
    let before_context = options
        .before_context
        .unwrap_or(options.context.unwrap_or(0));
    let after_context = options
        .after_context
        .unwrap_or(options.context.unwrap_or(0));
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(options.case_insensitive)
        .multi_line(options.multiline)
        .dot_matches_new_line(options.multiline)
        .build(&options.pattern)
        .map_err(|error| format!("Invalid regex pattern: {error}"))?;
    let glob_matchers = options
        .globs
        .iter()
        .map(|glob| {
            GlobBuilder::new(glob)
                .build()
                .map(|compiled| compiled.compile_matcher())
                .map_err(|error| format!("Invalid glob pattern: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // A MiniApp controls these bounded inputs and may deliberately request an
    // unbounded result (`head_limit: 0`). Keep the virtual capability bounded
    // while scanning, not only while rendering, so millions of matching lines
    // cannot expand into millions of formatted Strings first.
    let content_head_limit = if options.output_mode == OutputMode::Content {
        let requested = options
            .head_limit
            .filter(|limit| *limit > 0)
            .unwrap_or(MAX_VIRTUAL_GREP_CONTENT_LINES);
        let collection_budget = options
            .offset
            .checked_add(requested)
            .filter(|budget| *budget <= MAX_VIRTUAL_GREP_CONTENT_LINES)
            .ok_or_else(|| {
                format!(
                    "Virtual Grep offset + head_limit must not exceed {MAX_VIRTUAL_GREP_CONTENT_LINES} lines"
                )
            })?;
        Some((requested, collection_budget))
    } else {
        None
    };

    let mut file_results = Vec::new();
    let mut collected_content_lines = 0usize;
    for (path, content) in files {
        if content_head_limit.is_some_and(|(_, budget)| collected_content_lines >= budget) {
            break;
        }
        let path_buf = PathBuf::from(path);
        if !glob_matchers.is_empty() && !glob_matchers.iter().any(|glob| glob.is_match(&path_buf)) {
            continue;
        }
        if let Some(file_type) = options.file_type.as_deref() {
            let extension_matches = path_buf
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case(file_type)
                        || (file_type.eq_ignore_ascii_case("json")
                            && extension.eq_ignore_ascii_case("json5"))
                });
            if !extension_matches {
                continue;
            }
        }

        let mut searcher = build_grep_searcher(before_context, after_context, options.multiline);
        let sink_limit =
            content_head_limit.map(|(_, budget)| budget.saturating_sub(collected_content_lines));
        let sink = GrepSink::new(
            options.output_mode,
            options.show_line_numbers,
            before_context,
            after_context,
            sink_limit,
            path_buf.clone(),
            None,
        );
        searcher
            .search_slice(&matcher, content.as_bytes(), sink.clone())
            .map_err(|error| format!("Error searching virtual file {path}: {error}"))?;
        let file_matches = sink.get_match_count();
        if file_matches == 0 {
            continue;
        }
        let output_lines = if options.output_mode == OutputMode::Content {
            let remaining = content_head_limit
                .map(|(_, budget)| budget.saturating_sub(collected_content_lines))
                .unwrap_or(0);
            let mut bounded = Vec::with_capacity(remaining.min(256));
            'writes: for line in sink.take_output_lines() {
                if line.contains('\n') {
                    for part in line.lines().filter(|part| !part.is_empty()) {
                        if bounded.len() >= remaining {
                            break 'writes;
                        }
                        bounded.push(part.to_string());
                    }
                } else if !line.is_empty() {
                    if bounded.len() >= remaining {
                        break;
                    }
                    bounded.push(line);
                }
            }
            collected_content_lines = collected_content_lines.saturating_add(bounded.len());
            bounded
        } else {
            Vec::new()
        };
        file_results.push(GrepFileResult {
            path: path_buf,
            file_matches,
            output_lines,
            modified_time: SystemTime::UNIX_EPOCH,
        });
    }

    file_results.sort_by(|left, right| left.path.cmp(&right.path));
    let file_count = file_results.len();
    let total_matches = file_results.iter().map(|result| result.file_matches).sum();
    let mut content_lines = Vec::new();
    let mut file_match_counts = Vec::new();
    let mut matched_files = Vec::new();
    for result in file_results {
        let path = result.path.to_string_lossy().replace('\\', "/");
        match options.output_mode {
            OutputMode::Content => {
                for line in result.output_lines {
                    if line.contains('\n') {
                        content_lines.extend(
                            line.lines()
                                .filter(|part| !part.is_empty())
                                .map(str::to_string),
                        );
                    } else if !line.is_empty() {
                        content_lines.push(line);
                    }
                }
            }
            OutputMode::FilesWithMatches => matched_files.push(path),
            OutputMode::Count => file_match_counts.push((path, result.file_matches)),
        }
    }

    let (result_text, applied_limit, applied_offset) = match options.output_mode {
        OutputMode::Content => {
            let (lines, applied_limit, applied_offset) = apply_offset_limit(
                content_lines,
                content_head_limit.map(|(limit, _)| limit),
                options.offset,
            );
            (
                if lines.is_empty() {
                    format!("No matches found for pattern '{}'", options.pattern)
                } else {
                    lines.join("\n")
                },
                applied_limit,
                applied_offset,
            )
        }
        OutputMode::FilesWithMatches => {
            let (matches, applied_limit, applied_offset) =
                apply_offset_limit(matched_files, options.head_limit, options.offset);
            (
                if matches.is_empty() {
                    format!("No files found matching pattern '{}'", options.pattern)
                } else {
                    matches.join("\n")
                },
                applied_limit,
                applied_offset,
            )
        }
        OutputMode::Count => {
            let (counts, applied_limit, applied_offset) =
                apply_offset_limit(file_match_counts, options.head_limit, options.offset);
            let lines = counts
                .iter()
                .map(|(file, count)| format!("{file}:{count}"))
                .collect::<Vec<_>>();
            (
                if lines.is_empty() {
                    format!("No matches found for pattern '{}'", options.pattern)
                } else {
                    format!(
                        "Total {} matches in {} files:\n{}",
                        total_matches,
                        counts.len(),
                        lines.join("\n")
                    )
                },
                applied_limit,
                applied_offset,
            )
        }
    };

    Ok(GrepSearchResult {
        file_count,
        total_matches,
        result_text,
        applied_limit,
        applied_offset,
        cancelled: false,
    })
}

fn paths_equal_for_exclusion(path: &Path, excluded: &str) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    let excluded = excluded.replace('\\', "/");
    if path == excluded {
        return true;
    }
    let needs_identity_check = cfg!(windows) && path.eq_ignore_ascii_case(&excluded);
    if !needs_identity_check {
        return false;
    }
    let Ok(path) = std::fs::canonicalize(path) else {
        return false;
    };
    let Ok(excluded) = std::fs::canonicalize(excluded) else {
        return false;
    };
    path == excluded
}

#[cfg(test)]
mod tests {
    use super::{
        grep_search, grep_search_virtual_files, paths_equal_for_exclusion, GrepOptions, OutputMode,
        SearchCancellation, MAX_VIRTUAL_GREP_CONTENT_LINES,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("bitfun-grep-search-{name}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn create_file_symlink(target: &std::path::Path, alias: &std::path::Path) -> bool {
        std::os::unix::fs::symlink(target, alias).expect("file symlink should be available");
        true
    }

    #[test]
    fn virtual_file_search_preserves_regex_modes_filters_and_pagination() {
        let files = vec![
            (
                ".miniapp-context/scope/a.json".to_string(),
                std::sync::Arc::<str>::from("alpha\nNeedle one\nneedle two\n"),
            ),
            (
                ".miniapp-context/scope/b.txt".to_string(),
                std::sync::Arc::<str>::from("needle ignored by type\n"),
            ),
        ];
        let result = grep_search_virtual_files(
            GrepOptions::new("needle", ".miniapp-context/scope")
                .case_insensitive(true)
                .output_mode(OutputMode::Content)
                .file_type("json")
                .offset(1)
                .head_limit(1),
            &files,
        )
        .expect("virtual grep should succeed");

        assert_eq!(result.file_count, 1);
        assert_eq!(result.total_matches, 2);
        assert_eq!(
            result.result_text,
            ".miniapp-context/scope/a.json:3:needle two"
        );
        assert_eq!(result.applied_offset, Some(1));
    }

    #[test]
    fn virtual_file_search_bounds_explicit_unlimited_content_during_collection() {
        let files = vec![(
            ".miniapp-context/scope/large.ndjson".to_string(),
            std::sync::Arc::<str>::from(
                "needle\n".repeat(MAX_VIRTUAL_GREP_CONTENT_LINES.saturating_add(100)),
            ),
        )];
        let result = grep_search_virtual_files(
            GrepOptions::new("needle", ".miniapp-context/scope")
                .output_mode(OutputMode::Content)
                .head_limit(0),
            &files,
        )
        .expect("virtual grep should replace an unlimited request with a hard bound");

        assert_eq!(
            result.result_text.lines().count(),
            MAX_VIRTUAL_GREP_CONTENT_LINES
        );
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &std::path::Path, alias: &std::path::Path) -> bool {
        std::os::windows::fs::symlink_file(target, alias).is_ok()
    }

    #[test]
    fn a_search_that_was_never_cancelled_reports_so() {
        let root = make_temp_dir("cancel-none");
        fs::write(root.join("a.txt"), "needle\n").unwrap();

        let result = grep_search(
            GrepOptions::new("needle", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Content),
            None,
            None,
        )
        .unwrap();

        assert!(!result.cancelled);
        assert_eq!(result.file_count, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancelling_before_the_walk_starts_returns_an_empty_cancelled_result() {
        let root = make_temp_dir("cancel-upfront");
        fs::write(root.join("a.txt"), "needle\n").unwrap();

        let cancellation = SearchCancellation::new();
        cancellation.cancel();

        let result = grep_search(
            GrepOptions::new("needle", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Content)
                .cancellation(cancellation),
            None,
            None,
        )
        .unwrap();

        assert!(result.cancelled);
        assert_eq!(result.file_count, 0);
        assert_eq!(result.total_matches, 0);
        assert!(result.result_text.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancelling_mid_walk_keeps_what_was_already_found() {
        // Every file matches, so an uncancelled run would report all 200. The progress interval is
        // zeroed so the callback fires on every event; we cancel once five matches are in, which
        // puts the cancel mid-walk deterministically instead of racing a timer, and leaves a known
        // non-empty partial result to assert on.
        let root = make_temp_dir("cancel-midwalk");
        for index in 0..200 {
            fs::write(root.join(format!("file-{index:03}.txt")), "needle\n").unwrap();
        }

        let cancellation = SearchCancellation::new();
        let from_callback = cancellation.clone();
        let callback: super::ProgressCallback =
            std::sync::Arc::new(move |_files_processed, file_count, _total_matches| {
                if file_count >= 5 {
                    from_callback.cancel();
                }
            });

        let result = grep_search(
            GrepOptions::new("needle", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Content)
                .cancellation(cancellation),
            Some(callback),
            Some(0),
        )
        .unwrap();

        assert!(result.cancelled);
        // Partial, not empty and not complete: the point of the contract is that a cancelled search
        // still hands back the work it had already paid for.
        assert!(
            result.file_count < 200,
            "expected the walk to stop early, saw {} files",
            result.file_count
        );
        // The cancel is raised from inside the callback once five matches are counted, so a
        // correct implementation never comes back with fewer than that.
        assert!(result.file_count >= 5, "saw {} files", result.file_count);
        assert_eq!(result.result_text.lines().count(), result.file_count);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn truncates_very_long_output_lines() {
        let root = make_temp_dir("truncate");
        let file_path = root.join("sample.txt");
        let long_line = "a".repeat(600);
        fs::write(&file_path, format!("{long_line}\n")).unwrap();

        let result = grep_search(
            GrepOptions::new("a+", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Content)
                .show_line_numbers(true)
                .head_limit(10),
            None,
            None,
        )
        .unwrap();

        assert!(result.result_text.contains("[truncated]"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn multiline_matches_paginate_by_physical_line() {
        // A multiline match reaches the sink as one entry spanning several
        // physical lines; head_limit/offset must still paginate by physical
        // line, not by match block.
        let root = make_temp_dir("multiline-head-limit");
        let file_path = root.join("sample.txt");
        fs::write(&file_path, "alpha\nbeta\nnoise\nalpha\nbeta\n").unwrap();

        let result = grep_search(
            GrepOptions::new("alpha\\nbeta", root.to_string_lossy().to_string())
                .multiline(true)
                .output_mode(OutputMode::Content)
                .show_line_numbers(true)
                .head_limit(3),
            None,
            None,
        )
        .unwrap();

        let lines: Vec<&str> = result.result_text.lines().collect();
        assert_eq!(
            lines.len(),
            3,
            "head_limit must count physical lines, got {lines:?}"
        );
        assert!(lines[0].ends_with(":1:alpha"), "got {lines:?}");
        assert_eq!(lines[1], "beta", "got {lines:?}");
        assert!(lines[2].ends_with(":4:alpha"), "got {lines:?}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exact_exclusions_remove_unassigned_files_from_results() {
        let root = make_temp_dir("excluded");
        let included = root.join("included.txt");
        let excluded = root.join("excluded.txt");
        fs::write(&included, "review-token\n").unwrap();
        fs::write(&excluded, "review-token\n").unwrap();

        let result = grep_search(
            GrepOptions::new("review-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::FilesWithMatches)
                .excluded_paths(vec![excluded.to_string_lossy().to_string()]),
            None,
            None,
        )
        .unwrap();

        assert!(result.result_text.contains("included.txt"));
        assert!(!result.result_text.contains("excluded.txt"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exclusion_comparison_does_not_merge_distinct_path_spelling() {
        assert!(!paths_equal_for_exclusion(
            PathBuf::from("src/CaseSensitive.rs").as_path(),
            "src/casesensitive.rs",
        ));
    }

    #[test]
    fn exact_exclusions_block_file_symlink_aliases() {
        let root = make_temp_dir("excluded-symlink");
        let excluded = root.join("excluded.txt");
        let alias = root.join("alias.txt");
        fs::write(&excluded, "linked-review-token\n").unwrap();

        if !create_file_symlink(&excluded, &alias) {
            let _ = fs::remove_dir_all(root);
            return;
        }

        let result = grep_search(
            GrepOptions::new("linked-review-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::FilesWithMatches)
                .excluded_paths(vec![excluded.to_string_lossy().to_string()])
                .reject_linked_files(true),
            None,
            None,
        )
        .unwrap();

        assert!(!result.result_text.contains("alias.txt"));
        assert_eq!(result.file_count, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exact_exclusions_block_file_hard_link_aliases() {
        let root = make_temp_dir("excluded-hard-link");
        let excluded = root.join("excluded.txt");
        let alias = root.join("alias.txt");
        fs::write(&excluded, "linked-review-token\n").unwrap();
        fs::hard_link(&excluded, &alias).unwrap();

        let result = grep_search(
            GrepOptions::new("linked-review-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::FilesWithMatches)
                .excluded_paths(vec![excluded.to_string_lossy().to_string()])
                .reject_linked_files(true),
            None,
            None,
        )
        .unwrap();

        assert!(!result.result_text.contains("alias.txt"));
        assert_eq!(result.file_count, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exact_exclusions_block_file_symlinks_outside_the_search_root() {
        let root = make_temp_dir("excluded-outside-link");
        let outside = make_temp_dir("outside-link-target");
        let alias = root.join("outside-alias.txt");
        let secret = outside.join("secret.txt");
        fs::write(&secret, "outside-review-token\n").unwrap();
        if !create_file_symlink(&secret, &alias) {
            let _ = fs::remove_dir_all(root);
            let _ = fs::remove_dir_all(outside);
            return;
        }

        let result = grep_search(
            GrepOptions::new("outside-review-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::FilesWithMatches)
                .reject_linked_files(true),
            None,
            None,
        )
        .unwrap();

        assert_eq!(result.file_count, 0);
        assert!(!result.result_text.contains("outside-alias.txt"));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn parallel_output_modes_keep_stable_order_and_counts() {
        let root = make_temp_dir("parallel-output");
        fs::write(root.join("z-last.txt"), "parallel-token\n").unwrap();
        fs::write(root.join("a-first.txt"), "parallel-token\n").unwrap();
        let display_base = root.to_string_lossy().to_string();

        let content = grep_search(
            GrepOptions::new("parallel-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Content)
                .display_base(display_base.clone()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            content.result_text,
            "a-first.txt:1:parallel-token\nz-last.txt:1:parallel-token"
        );

        let count = grep_search(
            GrepOptions::new("parallel-token", root.to_string_lossy().to_string())
                .output_mode(OutputMode::Count)
                .display_base(display_base),
            None,
            None,
        )
        .unwrap();
        assert_eq!(count.file_count, 2);
        assert_eq!(count.total_matches, 2);
        assert_eq!(
            count.result_text,
            "Total 2 matches in 2 files:\na-first.txt:1\nz-last.txt:1"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parallel_content_order_is_stable_across_repeated_runs() {
        // Two files can agree by luck; sixty spread over eight workers cannot. Names are written in
        // an order unrelated to their sort order so a reducer that just preserved arrival order
        // would produce something different from the sorted answer.
        let root = make_temp_dir("parallel-stable-order");
        for index in (0..60).rev() {
            fs::write(
                root.join(format!("file-{index:02}.txt")),
                "parallel-token\n",
            )
            .unwrap();
        }
        let display_base = root.to_string_lossy().to_string();

        let run = || {
            grep_search(
                GrepOptions::new("parallel-token", root.to_string_lossy().to_string())
                    .output_mode(OutputMode::Content)
                    .display_base(display_base.clone()),
                None,
                None,
            )
            .unwrap()
            .result_text
        };

        let expected: Vec<String> = (0..60)
            .map(|index| format!("file-{index:02}.txt:1:parallel-token"))
            .collect();
        for _ in 0..5 {
            assert_eq!(run(), expected.join("\n"));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pagination_slices_the_stable_parallel_order() {
        // Pagination is only meaningful if the underlying order is fixed: windows taken with
        // different offsets have to tile the full result exactly, with no gaps and no repeats.
        let root = make_temp_dir("parallel-pagination");
        for index in 0..60 {
            fs::write(
                root.join(format!("file-{index:02}.txt")),
                "parallel-token\n",
            )
            .unwrap();
        }
        let display_base = root.to_string_lossy().to_string();

        let page = |offset: usize, limit: usize| {
            grep_search(
                GrepOptions::new("parallel-token", root.to_string_lossy().to_string())
                    .output_mode(OutputMode::Content)
                    .display_base(display_base.clone())
                    .offset(offset)
                    .head_limit(limit),
                None,
                None,
            )
            .unwrap()
        };

        let full = page(0, 60);
        let full_lines: Vec<&str> = full.result_text.lines().collect();
        assert_eq!(full_lines.len(), 60);

        let mut tiled = Vec::new();
        for offset in (0..60).step_by(7) {
            let window = page(offset, 7);
            assert_eq!(window.applied_offset, Some(offset).filter(|it| *it > 0));
            tiled.extend(
                window
                    .result_text
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(tiled, full_lines);

        // Past the end is an empty page, not an error and not a wrapped-around one. Note that the
        // rendered text is the same "No matches found" string an genuinely empty search produces —
        // callers that need to tell the two apart have to look at `total_matches`, which still
        // reports the full unpaginated count.
        let past_end = page(60, 7);
        assert_eq!(
            past_end.result_text,
            "No matches found for pattern 'parallel-token'"
        );
        assert_eq!(past_end.total_matches, 60);

        let _ = fs::remove_dir_all(root);
    }
}
