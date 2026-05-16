use log::{debug, info, warn};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use globset::{GlobBuilder, GlobMatcher};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;

const MAX_DISPLAY_COLUMNS: usize = 500;
const VCS_DIRECTORIES_TO_EXCLUDE: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// Output mode enumeration
#[derive(Debug, Clone, Copy)]
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
    output: Arc<Mutex<Vec<u8>>>,
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

    fn get_output(&self) -> String {
        let output = lock_recover(&self.output, "output");
        String::from_utf8_lossy(&output).to_string()
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

    fn write_line(&self, line: &[u8]) {
        if self.increment_line_count() {
            let mut output = lock_recover(&self.output, "output");
            output.extend_from_slice(line);
            output.push(b'\n');
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
                output.extend_from_slice(b"--\n");
            }
        }
        *last_line = Some(current_line);
    }

    /// Format output line (rg style: only show line number and content, no path)
    fn format_line(&self, line_number: u64, line: &[u8], is_match: bool) -> Vec<u8> {
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
            format!("{}{}{}:{}", path_prefix, separator, line_number, line_str).into_bytes()
        } else {
            format!("{}{}{}", path_prefix, separator, line_str).into_bytes()
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
                self.write_line(&formatted);
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
            self.write_line(&formatted);
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

    // Build searcher
    let mut searcher_builder = SearcherBuilder::new();
    searcher_builder
        .line_number(true)
        .before_context(before_context)
        .after_context(after_context);

    if multiline {
        searcher_builder.multi_line(true);
    }

    let mut searcher = searcher_builder.build();

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

    let walker = walk_builder.build();

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

    // Collect all results
    let mut content_lines = Vec::new();
    let mut total_matches = 0;
    let mut file_count = 0;
    let mut file_match_counts: Vec<(String, usize)> = Vec::new();
    let mut matched_files_with_mtime: Vec<(String, SystemTime)> = Vec::new();

    // Progress tracking
    let mut files_processed = 0;
    let mut last_progress_time = std::time::Instant::now();
    let progress_interval_millis = progress_interval_millis.unwrap_or(500);

    // Traverse files and search
    for result in walker {
        match result {
            Ok(entry) => {
                let path = entry.path();

                files_processed += 1;

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

                // Check if it's a file
                if !path.is_file() {
                    continue;
                }

                if is_vcs_path(path) {
                    continue;
                }

                if !glob_matchers.is_empty()
                    && !glob_matchers.iter().any(|matcher| matcher.is_match(path))
                {
                    continue;
                }

                let sink = GrepSink::new(
                    output_mode,
                    show_line_numbers,
                    before_context,
                    after_context,
                    None,
                    path.to_path_buf(),
                    display_base.clone(),
                );

                // Execute search
                if let Err(e) = searcher.search_path(&matcher, path, sink.clone()) {
                    warn!("Error searching file {}: {}", path.display(), e);
                    continue;
                }

                let file_matches = sink.get_match_count();
                if file_matches > 0 {
                    file_count += 1;
                    total_matches += file_matches;
                    match output_mode {
                        OutputMode::Content => {
                            let output = sink.get_output();
                            if !output.is_empty() {
                                content_lines.extend(
                                    output
                                        .lines()
                                        .filter(|line| !line.is_empty())
                                        .map(|line| line.to_string()),
                                );
                            }
                        }
                        OutputMode::FilesWithMatches => {
                            matched_files_with_mtime.push((
                                relativize_display_path(path, display_base.as_deref()),
                                modified_time(path),
                            ));
                        }
                        OutputMode::Count => {
                            file_match_counts.push((
                                relativize_display_path(path, display_base.as_deref()),
                                file_matches,
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error walking files: {}", e);
            }
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
                    result_text: lines.join("\n").trim_end_matches('\n').to_string(),
                    applied_limit,
                    applied_offset,
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
    })
}

#[cfg(test)]
mod tests {
    use super::{grep_search, GrepOptions, OutputMode};
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
}
