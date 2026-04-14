use std::env;
use std::path::PathBuf;

use codgrep::{BuildConfig, CorpusMode, TokenizerMode, WorkspaceIndex, WorkspaceIndexOptions};

fn print_section(title: &str, paths: &[String]) {
    println!("{title}: {}", paths.len());
    for path in paths {
        println!("  {path}");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let repo_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: cargo run --example inspect_dirty -- <repo_path> <index_path>")?;
    let index_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: cargo run --example inspect_dirty -- <repo_path> <index_path>")?;

    let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
        build_config: BuildConfig {
            repo_path,
            index_path,
            tokenizer: TokenizerMode::SparseNgram,
            corpus_mode: CorpusMode::RespectIgnore,
            include_hidden: false,
            max_file_size: 2 * 1024 * 1024,
            min_sparse_len: 3,
            max_sparse_len: 8,
        },
    })?;

    let status = workspace.status()?;
    let Some(diff) = status.dirty_files else {
        println!("dirty_files: none");
        return Ok(());
    };

    print_section("modified", &diff.modified_files);
    print_section("deleted", &diff.deleted_files);
    print_section("new", &diff.new_files);
    Ok(())
}
