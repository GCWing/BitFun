//! Multi-agent debate orchestrator.
//!
//! Three roles (Bull / Bear / Neutral) debate market direction,
//! with a DecisionAgent synthesizing the final verdict. Only triggered
//! when agent signals conflict or confidence variance exceeds threshold.

pub mod agents;
pub mod decision;
pub mod orchestrator;
pub mod record;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Output from a single analysis agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentOutput {
    /// Unique agent identifier, e.g. "trend_agent", "volume_agent"
    pub agent_id: String,
    /// Trading direction: "long" | "short" | "hold"
    pub direction: String,
    /// Confidence [0.0, 1.0]
    pub confidence: f64,
    /// Natural-language reasoning
    pub reasoning: String,
}

/// Context fed into a debate round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateContext {
    /// Instrument identifier, e.g. "rb9999"
    pub instrument: String,
    /// Timestamp when the debate was triggered
    pub timestamp: DateTime<Utc>,
    /// Full market state as JSON string (bars, indicators, etc.)
    pub state_json: String,
    /// Outputs from all analysis agents
    pub agent_outputs: Vec<AgentOutput>,
    /// Agent IDs whose signals conflict
    pub conflicting_agents: Vec<String>,
}

/// Token usage for a single LLM call within a debate turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Debate configuration loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateConfig {
    /// Maximum debate rounds (default 3)
    #[serde(default = "default_max_rounds")]
    pub max_rounds: usize,
    /// Bull role prompt template
    pub bull_prompt_template: String,
    /// Bear role prompt template
    pub bear_prompt_template: String,
    /// Neutral observer prompt template
    pub neutral_prompt_template: String,
    /// Decision agent prompt template
    pub decision_prompt_template: String,
    /// LLM model to use for debate agents
    #[serde(default = "default_debate_model")]
    pub model: String,
    /// Temperature for debate LLM calls
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_rounds() -> usize {
    3
}

fn default_debate_model() -> String {
    "deepseek-chat".into()
}

fn default_temperature() -> f32 {
    0.7
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            max_rounds: default_max_rounds(),
            bull_prompt_template: String::new(),
            bear_prompt_template: String::new(),
            neutral_prompt_template: String::new(),
            decision_prompt_template: String::new(),
            model: default_debate_model(),
            temperature: default_temperature(),
        }
    }
}

impl DebateConfig {
    /// Load debate configuration from a YAML string.
    pub fn from_yaml(yaml_str: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml_str)
    }

    /// Load the default debate_roles.yaml bundled with the crate.
    pub fn load_default() -> Self {
        let default_yaml = include_str!("../../config/debate_roles.yaml");
        Self::from_yaml(default_yaml).expect("bundle debate_roles.yaml must be valid")
    }
}
