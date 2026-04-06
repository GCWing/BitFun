use crate::agentic::agents::Agent;
use crate::agentic::agents::{PromptBuilder, PromptBuilderContext};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::FrontMatterMarkdown;
use async_trait::async_trait;
use serde_yaml::Value;

/// Subagent type: project-level or user-level
#[derive(Debug, Clone, Copy)]
pub enum CustomSubagentKind {
    /// Project subagent
    Project,
    /// User subagent
    User,
}

pub struct CustomSubagent {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub prompt: String,
    pub readonly: bool,
    pub path: String,
    pub kind: CustomSubagentKind,
    /// Whether this subagent is enabled, default true
    pub enabled: bool,
    /// Model ID to use, default "primary"
    pub model: String,
}

#[async_trait]
impl Agent for CustomSubagent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.name
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        ""
    }

    async fn build_prompt(&self, context: &PromptBuilderContext) -> BitFunResult<String> {
        let prompt_builder = PromptBuilder::new(context.clone());

        let prompt = prompt_builder
            .build_prompt_from_template(&self.prompt)
            .await?;

        Ok(prompt)
    }

    fn default_tools(&self) -> Vec<String> {
        self.tools.clone()
    }

    fn is_readonly(&self) -> bool {
        self.readonly
    }
}

impl CustomSubagent {
    pub fn new(
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        path: String,
        kind: CustomSubagentKind,
    ) -> Self {
        Self {
            name,
            description,
            tools,
            prompt,
            readonly,
            path,
            kind,
            enabled: true,
            model: "primary".to_string(),
        }
    }

    pub fn from_file(path: &str, kind: CustomSubagentKind) -> BitFunResult<Self> {
        let (metadata, content) = FrontMatterMarkdown::load(path)?;
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::Agent("Missing name field".to_string()))?
            .to_string();
        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::Agent("Missing description field".to_string()))?
            .to_string();
        let tools: Vec<String> = metadata
            .get("tools")
            .and_then(|v| v.as_str())
            .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
            .unwrap_or_else(|| Self::DEFAULT_TOOLS.iter().map(|s| s.to_string()).collect());

        let readonly = metadata
            .get("readonly")
            .and_then(|v| v.as_bool())
            .unwrap_or(Self::DEFAULT_READONLY);

        let enabled = metadata
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(Self::DEFAULT_ENABLED);

        let model = metadata
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(Self::DEFAULT_MODEL)
            .to_string();

        Ok(Self {
            name,
            description,
            tools,
            prompt: content,
            readonly,
            path: path.to_string(),
            kind,
            enabled,
            model,
        })
    }

    const DEFAULT_TOOLS: &'static [&'static str] = &["LS", "Read", "Glob", "Grep"];
    const DEFAULT_READONLY: bool = true;
    const DEFAULT_ENABLED: bool = true;
    const DEFAULT_MODEL: &'static str = "primary";

    /// Check if tools match default values
    fn is_default_tools(tools: &[String]) -> bool {
        if tools.len() != Self::DEFAULT_TOOLS.len() {
            return false;
        }
        tools
            .iter()
            .zip(Self::DEFAULT_TOOLS.iter())
            .all(|(a, b)| a == *b)
    }

    /// Save current subagent as markdown file with YAML front matter
    ///
    /// # Parameters
    /// - `enabled`: Override enabled value, None uses self.enabled
    /// - `model`: Override model value, None uses self.model
    ///
    /// Fields equal to default values are not saved
    pub fn save_to_file(&self, enabled: Option<bool>, model: Option<&str>) -> BitFunResult<()> {
        let enabled = enabled.unwrap_or(self.enabled);
        let model = model.unwrap_or(&self.model);

        let mut metadata = serde_yaml::Mapping::new();
        // Required fields
        metadata.insert(
            Value::String("name".into()),
            Value::String(self.name.clone()),
        );
        metadata.insert(
            Value::String("description".into()),
            Value::String(self.description.clone()),
        );
        // Optional fields: only save if not default values
        if !Self::is_default_tools(&self.tools) {
            metadata.insert(
                Value::String("tools".into()),
                Value::String(self.tools.join(", ")),
            );
        }
        if self.readonly != Self::DEFAULT_READONLY {
            metadata.insert(Value::String("readonly".into()), Value::Bool(self.readonly));
        }
        if enabled != Self::DEFAULT_ENABLED {
            metadata.insert(Value::String("enabled".into()), Value::Bool(enabled));
        }
        if model != Self::DEFAULT_MODEL {
            metadata.insert(
                Value::String("model".into()),
                Value::String(model.to_string()),
            );
        }
        let metadata = Value::Mapping(metadata);
        FrontMatterMarkdown::save(&self.path, &metadata, &self.prompt).map_err(BitFunError::Agent)
    }
}
