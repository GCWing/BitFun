//! Tool mode-override module.
//!
//! Mirrors `skills/mode_overrides.rs` for the tool side: a user-level global
//! availability switch (`ai.tool_settings`) plus mode-profile tool selection
//! stored through the shared agent-profile canonicalizer (`enabled_tools`).

pub mod mode_overrides;

pub use mode_overrides::{
    clear_user_mode_tool_overrides, filter_globally_disabled_tools,
    load_globally_disabled_user_tools, mode_tool_profile_id, set_global_user_tool_disabled,
};
