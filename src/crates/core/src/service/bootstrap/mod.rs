mod bootstrap_impl;

pub use bootstrap_impl::reset_workspace_persona_files_to_default;
pub(crate) use bootstrap_impl::{
    build_workspace_persona_prompt, ensure_workspace_gitignore_ignores_bitfun,
    ensure_workspace_persona_files_for_prompt, initialize_workspace_persona_files,
    is_workspace_bootstrap_pending,
};
