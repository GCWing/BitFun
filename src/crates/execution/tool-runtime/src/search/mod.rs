pub mod glob_search;
pub mod grep_search;

pub use glob_search::{
    build_remote_find_command, build_remote_rg_command, collect_remote_glob_matches,
    derive_walk_root, execute_local_glob, extract_glob_base_directory, limit_paths, normalize_path,
    LocalGlobRequest, LocalGlobResult,
};
pub use grep_search::{
    apply_offset_and_limit, build_remote_grep_command, count_remote_grep_matches, grep_search,
    relativize_result_text, render_remote_grep_result_text, GrepOptions, OutputMode,
    ProgressCallback, RemoteGrepCommandRequest,
};
