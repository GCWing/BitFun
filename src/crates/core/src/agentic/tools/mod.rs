//! Tool system - includes Tool interface, tool registry and tool executor

pub mod browser_control;
pub mod computer_use_capability;
pub mod computer_use_host;
pub mod computer_use_optimizer;
pub mod computer_use_verification;
pub mod framework;
pub mod image_context;
pub mod implementations;
pub mod manifest_resolver;
pub mod pipeline;
pub(crate) mod post_call_hooks;
pub mod registry;
pub mod restrictions;
pub mod user_input_manager;
pub mod workspace_paths;
pub use bitfun_agent_tools::input_validator;

pub use framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
pub use image_context::{ImageContextData, ImageContextProvider, ImageContextProviderRef};
pub use input_validator::InputValidator;
pub use manifest_resolver::{resolve_tool_manifest, resolve_visible_tools, ResolvedToolManifest, ResolvedVisibleTools};
pub use pipeline::*;
pub use registry::{
    create_tool_registry, get_all_registered_tool_names, get_all_registered_tools, get_all_tools,
    get_readonly_registered_tool_names, get_readonly_tools,
};
pub use restrictions::{ToolPathOperation, ToolPathPolicy, ToolRuntimeRestrictions};
