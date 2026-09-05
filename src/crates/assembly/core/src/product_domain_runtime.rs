//! Core-owned adapters for product-domain runtime ports.
//!
//! Product-domain crates own stable contracts and pure orchestration. This
//! module keeps the concrete MiniApp and function-agent runtime bindings in
//! core so filesystem, process, Git, and AI behavior stays on the legacy path.

#[cfg(feature = "function-agents")]
use std::path::Path;
#[cfg(feature = "function-agents")]
use std::sync::Arc;

#[cfg(feature = "function-agents")]
use log::info;
#[cfg(feature = "function-agents")]
use openbitfun_product_domains::function_agents::ports::{
    FunctionAgentAiPort, FunctionAgentGitPort, FunctionAgentRuntimeFacade,
};
#[cfg(feature = "tools-miniapp")]
use openbitfun_product_domains::miniapp::ports::{MiniAppRuntimeFacade, MiniAppStoragePort};

#[cfg(feature = "function-agents")]
use crate::function_agents::common::AgentResult;
#[cfg(feature = "function-agents")]
use crate::function_agents::port_adapters::{
    CoreFunctionAgentAiAdapter, CoreFunctionAgentGitAdapter,
};
#[cfg(feature = "function-agents")]
use crate::function_agents::{CommitMessage, CommitMessageOptions};
#[cfg(feature = "function-agents")]
use crate::infrastructure::ai::AIClientFactory;

pub(crate) struct CoreProductDomainRuntime;

impl CoreProductDomainRuntime {
    #[cfg(feature = "tools-miniapp")]
    pub(crate) fn miniapp_runtime_facade(
        storage: &dyn MiniAppStoragePort,
    ) -> MiniAppRuntimeFacade<'_> {
        MiniAppRuntimeFacade::new(storage)
    }

    #[cfg(feature = "function-agents")]
    pub(crate) fn function_agent_git_adapter() -> CoreFunctionAgentGitAdapter {
        CoreFunctionAgentGitAdapter
    }

    #[cfg(feature = "function-agents")]
    pub(crate) fn function_agent_ai_adapter(
        factory: Arc<AIClientFactory>,
    ) -> CoreFunctionAgentAiAdapter {
        CoreFunctionAgentAiAdapter::new(factory)
    }

    #[cfg(feature = "function-agents")]
    pub(crate) fn function_agent_runtime_facade<'a>(
        git: &'a dyn FunctionAgentGitPort,
        ai: &'a dyn FunctionAgentAiPort,
    ) -> FunctionAgentRuntimeFacade<'a> {
        FunctionAgentRuntimeFacade::new(git, ai)
    }

    #[cfg(feature = "function-agents")]
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
}
