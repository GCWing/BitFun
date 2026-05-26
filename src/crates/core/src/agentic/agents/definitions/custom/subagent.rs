use crate::agentic::agents::Agent;
use crate::agentic::agents::{PromptBuilder, PromptBuilderContext, UserContextPolicy};
use crate::agentic::session::SystemPromptCacheIdentity;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::FrontMatterMarkdown;
use async_trait::async_trait;
use serde_yaml::Value;
use sha2::{Digest, Sha256};

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
    pub review: bool,
    pub path: String,
    pub kind: CustomSubagentKind,
    /// Model ID to use, default "fast"
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

    fn system_prompt_cache_identity(&self, _model_name: Option<&str>) -> SystemPromptCacheIdentity {
        let prompt_hash = hex::encode(Sha256::digest(self.prompt.as_bytes()));
        SystemPromptCacheIdentity::new(format!("custom_prompt_sha256:{prompt_hash}"))
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

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_project_layout()
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
            review: false,
            path,
            kind,
            model: "fast".to_string(),
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

        let review = metadata
            .get("review")
            .and_then(|v| v.as_bool())
            .unwrap_or(Self::DEFAULT_REVIEW);

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
            review,
            path: path.to_string(),
            kind,
            model,
        })
    }

    const DEFAULT_TOOLS: &'static [&'static str] = &["LS", "Read", "Glob", "Grep"];
    const DEFAULT_READONLY: bool = true;
    const DEFAULT_REVIEW: bool = false;
    const DEFAULT_MODEL: &'static str = "fast";

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
    /// - `model`: Override model value, None uses self.model
    ///
    /// Fields equal to default values are not saved
    pub fn save_to_file(&self, model: Option<&str>) -> BitFunResult<()> {
        let model = model.unwrap_or(&self.model);

        let mut metadata = serde_yaml::Mapping::new();
        metadata.insert(
            Value::String("name".into()),
            Value::String(self.name.clone()),
        );
        metadata.insert(
            Value::String("description".into()),
            Value::String(self.description.clone()),
        );
        if !Self::is_default_tools(&self.tools) {
            metadata.insert(
                Value::String("tools".into()),
                Value::String(self.tools.join(", ")),
            );
        }
        if self.readonly != Self::DEFAULT_READONLY {
            metadata.insert(Value::String("readonly".into()), Value::Bool(self.readonly));
        }
        if self.review != Self::DEFAULT_REVIEW {
            metadata.insert(Value::String("review".into()), Value::Bool(self.review));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn join(&self, name: &str) -> String {
            self.path.join(name).to_string_lossy().to_string()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn review_metadata_round_trips_through_front_matter() {
        let dir = TestTempDir::new("bitfun-subagent-test");
        let path = dir.join("review-agent.md");
        let mut subagent = CustomSubagent::new(
            "ReviewExtra".to_string(),
            "Additional code reviewer".to_string(),
            vec!["Read".to_string(), "Grep".to_string()],
            "Review the selected files.".to_string(),
            true,
            path.clone(),
            CustomSubagentKind::User,
        );
        subagent.review = true;

        subagent
            .save_to_file(None)
            .expect("review subagent should save");

        let saved = fs::read_to_string(&path).expect("saved subagent should be readable");
        assert!(saved.contains("review: true"));

        let loaded = CustomSubagent::from_file(&path, CustomSubagentKind::User)
            .expect("review subagent should load");
        assert!(loaded.review);
        assert!(loaded.readonly);
    }
}
