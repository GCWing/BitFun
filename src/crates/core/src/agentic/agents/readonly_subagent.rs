use super::{Agent, RequestContextPolicy};
use async_trait::async_trait;

/// Internal helper that holds the common metadata and behaviour for
/// read-only subagents.
pub struct ReadonlySubagent {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    prompt_template: &'static str,
    default_tools: &'static [&'static str],
}

impl ReadonlySubagent {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        prompt_template: &'static str,
        default_tools: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            name,
            description,
            prompt_template,
            default_tools,
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

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::instructions_only()
    }

    fn is_readonly(&self) -> bool {
        true
    }
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

            fn request_context_policy(&self) -> $crate::agentic::agents::RequestContextPolicy {
                self.inner.request_context_policy()
            }

            fn is_readonly(&self) -> bool {
                self.inner.is_readonly()
            }
        }
    };
}
