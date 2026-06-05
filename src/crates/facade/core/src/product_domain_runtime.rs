//! Core-owned adapters for product-domain runtime ports.
//!
//! Product-domain crates own stable contracts and pure orchestration. This
//! module keeps the concrete MiniApp and function-agent runtime bindings in
//! core so filesystem, process, Git, and AI behavior stays on the legacy path.

use std::path::Path;
use std::sync::Arc;

use bitfun_product_domains::function_agents::ports::{
    FunctionAgentAiPort, FunctionAgentGitPort, FunctionAgentRuntimeFacade,
};
use bitfun_product_domains::miniapp::ports::{MiniAppRuntimeFacade, MiniAppStoragePort};
use chrono::{Local, Timelike};
use log::info;

use crate::function_agents::common::AgentResult;
use crate::function_agents::port_adapters::{
    CoreFunctionAgentAiAdapter, CoreFunctionAgentGitAdapter,
};
use crate::function_agents::{
    CommitMessage, CommitMessageOptions, WorkStateAnalysis, WorkStateOptions,
};
use crate::infrastructure::ai::AIClientFactory;

pub(crate) struct CoreProductDomainRuntime;

impl CoreProductDomainRuntime {
    pub(crate) fn miniapp_runtime_facade(
        storage: &dyn MiniAppStoragePort,
    ) -> MiniAppRuntimeFacade<'_> {
        MiniAppRuntimeFacade::new(storage)
    }

    pub(crate) fn function_agent_git_adapter() -> CoreFunctionAgentGitAdapter {
        CoreFunctionAgentGitAdapter::default()
    }

    pub(crate) fn function_agent_ai_adapter(
        factory: Arc<AIClientFactory>,
    ) -> CoreFunctionAgentAiAdapter {
        CoreFunctionAgentAiAdapter::new(factory)
    }

    pub(crate) fn function_agent_runtime_facade<'a>(
        git: &'a dyn FunctionAgentGitPort,
        ai: &'a dyn FunctionAgentAiPort,
    ) -> FunctionAgentRuntimeFacade<'a> {
        FunctionAgentRuntimeFacade::new(git, ai)
    }

    pub(crate) async fn generate_function_agent_commit_message(
        factory: Arc<AIClientFactory>,
        repo_path: &Path,
        options: CommitMessageOptions,
    ) -> AgentResult<CommitMessage> {
        info!(
            "Generating commit message (AI-driven): repo_path={:?}",
            repo_path
        );

        let git_adapter = Self::function_agent_git_adapter();
        let ai_adapter = Self::function_agent_ai_adapter(factory);
        let facade = Self::function_agent_runtime_facade(&git_adapter, &ai_adapter);
        facade
            .generate_commit_message(repo_path.to_path_buf(), options)
            .await
    }

    pub(crate) async fn analyze_function_agent_work_state(
        factory: Arc<AIClientFactory>,
        repo_path: &Path,
        options: WorkStateOptions,
    ) -> AgentResult<WorkStateAnalysis> {
        info!("Analyzing work state: repo_path={:?}", repo_path);

        let now = Local::now();
        let git_adapter = Self::function_agent_git_adapter();
        let ai_adapter = Self::function_agent_ai_adapter(factory);
        let facade = Self::function_agent_runtime_facade(&git_adapter, &ai_adapter);
        // Keep the legacy analyzed_at timing in core: assign it after AI analysis completes.
        let mut analysis = facade
            .analyze_work_state(
                repo_path.to_path_buf(),
                options,
                now.timestamp(),
                now.hour(),
                String::new(),
            )
            .await?;
        analysis.analyzed_at = Local::now().to_rfc3339();
        Ok(analysis)
    }
}
