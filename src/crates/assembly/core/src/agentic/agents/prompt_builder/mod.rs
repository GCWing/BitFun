mod prompt_builder_impl;
mod user_context;

pub use bitfun_agent_runtime::prompt::{
    PrependedPromptReminders, RemoteExecutionHints, RuntimeContextNeeds, ToolListingSections,
};
pub use prompt_builder_impl::{
    build_prompt_context_for_workspace, PromptBuilder, PromptBuilderContext,
};
pub use user_context::{UserContextPolicy, UserContextSection};
