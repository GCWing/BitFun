pub mod builder;
pub mod format;
pub mod searcher;

pub use builder::{
    build_index, build_index_with_options, rebuild_index, rebuild_index_with_options,
    IndexBuildOptions, RebuildMode,
};
pub(crate) use searcher::DirtyPathKind;
pub use searcher::{IndexSearcher, IndexWorktreeDiff};
