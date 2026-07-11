use async_trait::async_trait;

use super::error::Result;
use super::types::{
    GlobOutcome, GlobRequest, RepoStatus, SearchOutcome, SearchRequest, TaskStatus,
};

#[async_trait]
pub(crate) trait FlashgrepRepoSession: Send + Sync {
    async fn status(&self) -> Result<RepoStatus>;
    async fn task_status(&self, task_id: String) -> Result<TaskStatus>;
    async fn build_index(&self) -> Result<TaskStatus>;
    async fn rebuild_index(&self) -> Result<TaskStatus>;
    async fn search(&self, request: SearchRequest) -> Result<SearchOutcome>;
    async fn glob(&self, request: GlobRequest) -> Result<GlobOutcome>;
    async fn close(&self) -> Result<()>;
}
