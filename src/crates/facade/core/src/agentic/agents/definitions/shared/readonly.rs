use crate::agentic::agents::{Agent, AgentToolPolicyOverrides, UserContextPolicy};
use async_trait::async_trait;

/// Internal helper that holds the common metadata and behaviour for
/// read-only subagents.
pub struct ReadonlySubagent {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    prompt_template: &'static str,
    default_tools: &'static [&'static str],
    tool_exposure_overrides: AgentToolPolicyOverrides,
    user_context_policy: UserContextPolicy,
}

impl ReadonlySubagent {
    pub fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        prompt_template: &'static str,
        default_tools: &'static [&'static str],
    ) -> Self {
        Self::with_overrides(
            id,
            name,
            description,
            prompt_template,
            default_tools,
            AgentToolPolicyOverrides::default(),
        )
    }

    pub fn with_overrides(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        prompt_template: &'static str,
        default_tools: &'static [&'static str],
        tool_exposure_overrides: AgentToolPolicyOverrides,
    ) -> Self {
        Self {
            id,
            name,
            description,
            prompt_template,
            default_tools,
            tool_exposure_overrides,
            user_context_policy: UserContextPolicy::empty().with_workspace_instructions(),
        }
    }

    pub fn with_policy(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        prompt_template: &'static str,
        default_tools: &'static [&'static str],
        user_context_policy: UserContextPolicy,
    ) -> Self {
        Self {
            id,
            name,
            description,
            prompt_template,
            default_tools,
            tool_exposure_overrides: AgentToolPolicyOverrides::default(),
            user_context_policy,
        }
    }
}

#[async_trait]
impl Agent for ReadonlySubagent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        self.prompt_template
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.iter().map(|s| s.to_string()).collect()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        self.user_context_policy.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn is_readonly(&self) -> bool {
        true
    }
}

#[macro_export]
macro_rules! define_readonly_subagent_with_context_policy {
    (
        $struct_name:ident,
        $id:expr,
        $name:literal,
        $description:literal,
        $prompt:literal,
        $tools:expr,
        $user_context_policy:expr
    ) => {
        pub struct $struct_name {
            inner: $crate::agentic::agents::ReadonlySubagent,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    inner: $crate::agentic::agents::ReadonlySubagent::with_policy(
                        $id,
                        $name,
                        $description,
                        $prompt,
                        $tools,
                        $user_context_policy,
                    ),
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::agentic::agents::Agent for $struct_name {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn id(&self) -> &str {
                self.inner.id()
            }

            fn name(&self) -> &str {
                self.inner.name()
            }

            fn description(&self) -> &str {
                self.inner.description()
            }

            fn prompt_template_name(&self, model_name: Option<&str>) -> &str {
                self.inner.prompt_template_name(model_name)
            }

            fn default_tools(&self) -> Vec<String> {
                self.inner.default_tools()
            }

            fn user_context_policy(&self) -> $crate::agentic::agents::UserContextPolicy {
                self.inner.user_context_policy()
            }

            fn tool_exposure_overrides(
                &self,
            ) -> &$crate::agentic::agents::AgentToolPolicyOverrides {
                self.inner.tool_exposure_overrides()
            }

            fn is_readonly(&self) -> bool {
                self.inner.is_readonly()
            }
        }
    };
}

/// Define a read-only subagent struct and its `Agent` implementation
/// by delegating to an inner `ReadonlySubagent`.
#[macro_export]
macro_rules! define_readonly_subagent {
    (
        $struct_name:ident,
        $id:expr,
        $name:literal,
        $description:literal,
        $prompt:literal,
        $tools:expr
    ) => {
        pub struct $struct_name {
            inner: $crate::agentic::agents::ReadonlySubagent,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    inner: $crate::agentic::agents::ReadonlySubagent::new(
                        $id,
                        $name,
                        $description,
                        $prompt,
                        $tools,
                    ),
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::agentic::agents::Agent for $struct_name {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn id(&self) -> &str {
                self.inner.id()
            }

            fn name(&self) -> &str {
                self.inner.name()
            }

            fn description(&self) -> &str {
                self.inner.description()
            }

            fn prompt_template_name(&self, model_name: Option<&str>) -> &str {
                self.inner.prompt_template_name(model_name)
            }

            fn default_tools(&self) -> Vec<String> {
                self.inner.default_tools()
            }

            fn user_context_policy(&self) -> $crate::agentic::agents::UserContextPolicy {
                self.inner.user_context_policy()
            }

            fn tool_exposure_overrides(
                &self,
            ) -> &$crate::agentic::agents::AgentToolPolicyOverrides {
                self.inner.tool_exposure_overrides()
            }

            fn is_readonly(&self) -> bool {
                self.inner.is_readonly()
            }
        }
    };
}

#[macro_export]
macro_rules! define_readonly_subagent_with_overrides {
    (
        $struct_name:ident,
        $id:expr,
        $name:literal,
        $description:literal,
        $prompt:literal,
        $tools:expr,
        $overrides:expr
    ) => {
        pub struct $struct_name {
            inner: $crate::agentic::agents::ReadonlySubagent,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    inner: $crate::agentic::agents::ReadonlySubagent::with_overrides(
                        $id,
                        $name,
                        $description,
                        $prompt,
                        $tools,
                        $overrides,
                    ),
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::agentic::agents::Agent for $struct_name {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn id(&self) -> &str {
                self.inner.id()
            }

            fn name(&self) -> &str {
                self.inner.name()
            }

            fn description(&self) -> &str {
                self.inner.description()
            }

            fn prompt_template_name(&self, model_name: Option<&str>) -> &str {
                self.inner.prompt_template_name(model_name)
            }

            fn default_tools(&self) -> Vec<String> {
                self.inner.default_tools()
            }

            fn user_context_policy(&self) -> $crate::agentic::agents::UserContextPolicy {
                self.inner.user_context_policy()
            }

            fn tool_exposure_overrides(
                &self,
            ) -> &$crate::agentic::agents::AgentToolPolicyOverrides {
                self.inner.tool_exposure_overrides()
            }

            fn is_readonly(&self) -> bool {
                self.inner.is_readonly()
            }
        }
    };
}
