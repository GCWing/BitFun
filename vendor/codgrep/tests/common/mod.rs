use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use codgrep::{
    advanced::IndexSearcher, build_index, BuildConfig, CorpusMode, QueryConfig, SearchMode,
    TokenizerMode,
};
use tempfile::{tempdir, TempDir};

pub struct TestRepo {
    _temp: TempDir,
    pub repo: PathBuf,
    pub index: PathBuf,
}

impl TestRepo {
    pub fn new() -> Self {
        let temp = tempdir().expect("test should succeed");
        let repo_dir = temp.path().join("repo");
        fs::create_dir_all(&repo_dir).expect("test should succeed");
        let repo = fs::canonicalize(&repo_dir).expect("test should succeed");
        let index = repo.join(".codgrep-index");
        Self {
            _temp: temp,
            repo,
            index,
        }
    }

    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.repo.join(relative)
    }

    pub fn create_dir(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).expect("test should succeed");
        path
    }

    pub fn write(&self, relative: impl AsRef<Path>, contents: &str) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test should succeed");
        }
        fs::write(&path, contents).expect("test should succeed");
        path
    }

    pub fn trigram_build_config(&self) -> BuildConfig {
        BuildConfig {
            repo_path: self.repo.clone(),
            index_path: self.index.clone(),
            tokenizer: TokenizerMode::Trigram,
            corpus_mode: CorpusMode::RespectIgnore,
            include_hidden: false,
            max_file_size: 1024 * 1024,
            min_sparse_len: 3,
            max_sparse_len: 32,
        }
    }

    pub fn sparse_build_config(&self) -> BuildConfig {
        BuildConfig {
            tokenizer: TokenizerMode::SparseNgram,
            min_sparse_len: 3,
            max_sparse_len: 8,
            ..self.trigram_build_config()
        }
    }

    pub fn build(&self) {
        self.build_with(self.trigram_build_config());
    }

    pub fn build_sparse(&self) {
        self.build_with(self.sparse_build_config());
    }

    pub fn build_with(&self, config: BuildConfig) {
        build_index(&config).expect("test should succeed");
    }

    pub fn searcher(&self) -> IndexSearcher {
        IndexSearcher::open(self.index.clone()).expect("test should succeed")
    }

    pub fn init_git(&self) {
        self.run_git(["init"]);
        self.run_git(["config", "user.name", "Test User"]);
        self.run_git(["config", "user.email", "test@example.com"]);
    }

    pub fn commit_all(&self, message: &str) {
        self.run_git(["add", "-A"]);
        self.run_git(["commit", "-m", message]);
    }

    pub fn git_head(&self) -> String {
        let output = self.run_git(["rev-parse", "HEAD"]);
        String::from_utf8(output.stdout)
            .expect("git stdout should be utf-8")
            .trim()
            .to_string()
    }

    pub fn seed_mock_git_repo(&self, module_count: usize, files_per_module: usize) -> Vec<PathBuf> {
        self.init_git();
        let mut created = Vec::new();
        self.write(
            "README.md",
            "# Mock Repo\n\nSynthetic repository fixture for e2e coverage.\n",
        );
        for module in 0..module_count {
            for file in 0..files_per_module {
                let relative = format!("src/module_{module:02}/file_{file:03}.rs");
                let contents = format!(
                    "pub const MODULE_{module:02}_FILE_{file:03}: &str = \"token_{module}_{file}\";\n"
                );
                created.push(self.write(relative, &contents));
            }
        }
        self.commit_all("seed mock repo");
        created
    }

    fn run_git<const N: usize>(&self, args: [&str; N]) -> std::process::Output {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .output()
            .expect("git command should run");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

pub fn query(pattern: &str) -> QueryConfig {
    QueryConfig {
        regex_pattern: pattern.into(),
        patterns: vec![pattern.into()],
        ..QueryConfig::default()
    }
}

pub fn count_query(pattern: &str) -> QueryConfig {
    QueryConfig {
        search_mode: SearchMode::CountOnly,
        ..query(pattern)
    }
}

pub fn count_matches_query(pattern: &str) -> QueryConfig {
    QueryConfig {
        search_mode: SearchMode::CountMatches,
        ..query(pattern)
    }
}
