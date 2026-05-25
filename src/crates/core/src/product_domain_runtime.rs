//! Core-owned adapters for product-domain runtime ports.
//!
//! Product-domain crates own stable contracts and pure orchestration. This
//! module keeps the concrete MiniApp and function-agent runtime bindings in
//! core so filesystem, process, Git, and AI behavior stays on the legacy path.

use std::sync::Arc;

use bitfun_product_domains::function_agents::ports::{
    FunctionAgentAiPort, FunctionAgentGitPort, FunctionAgentRuntimeFacade,
};
use bitfun_product_domains::miniapp::ports::{MiniAppRuntimeFacade, MiniAppStoragePort};

use crate::function_agents::port_adapters::{
    CoreFunctionAgentAiAdapter, CoreFunctionAgentGitAdapter,
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
}
