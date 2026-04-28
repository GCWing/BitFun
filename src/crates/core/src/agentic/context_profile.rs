//! Adaptive context profile policy.
//!
//! Profiles keep context behavior aligned with the shape of the agent workload
//! without exposing more knobs to the UI.

use crate::agentic::session::compression::microcompact::MicrocompactConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextProfile {
    LongTask,
    Conversation,
}

impl ContextProfile {
    pub fn for_agent_type(agent_type: &str) -> Self {
        Self::for_agent_context(agent_type, false)
    }

    pub fn for_agent_context(agent_type: &str, is_review_subagent: bool) -> Self {
        if is_review_subagent || is_long_task_agent(agent_type) {
            Self::LongTask
        } else {
            Self::Conversation
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCapabilityProfile {
    Standard,
    Weak,
}

impl ModelCapabilityProfile {
    pub fn from_model_id(model_id: Option<&str>) -> Self {
        let Some(model_id) = model_id.map(str::trim).filter(|id| !id.is_empty()) else {
            return Self::Standard;
        };
        let normalized = model_id.to_ascii_lowercase();
        if matches!(normalized.as_str(), "auto" | "fast" | "primary") {
            return Self::Standard;
        }

        let weak_markers = ["haiku", "mini", "small", "lite", "flash", "nano"];
        if weak_markers
            .iter()
            .any(|marker| normalized.contains(marker))
        {
            Self::Weak
        } else {
            Self::Standard
        }
    }

    pub fn from_resolved_model(resolved_model_id: &str, provider_model_name: &str) -> Self {
        let resolved = Self::from_model_id(Some(resolved_model_id));
        if resolved == Self::Weak {
            resolved
        } else {
            Self::from_model_id(Some(provider_model_name))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextProfilePolicy {
    pub profile: ContextProfile,
    pub microcompact_keep_recent: usize,
    pub microcompact_trigger_ratio: f32,
    pub compression_contract_limit: usize,
    pub subagent_concurrency_cap: usize,
    pub repeated_tool_signature_threshold: usize,
    pub consecutive_failed_command_threshold: usize,
}

impl ContextProfilePolicy {
    pub fn for_agent_context(
        agent_type: &str,
        is_review_subagent: bool,
        model_capability: ModelCapabilityProfile,
    ) -> Self {
        let profile = ContextProfile::for_agent_context(agent_type, is_review_subagent);
        let mut policy = match profile {
            ContextProfile::LongTask => Self::long_task(),
            ContextProfile::Conversation => Self::conversation(),
        };

        if model_capability == ModelCapabilityProfile::Weak {
            policy.apply_weak_model_override();
        }

        policy
    }

    pub fn for_agent_context_and_model(
        agent_type: &str,
        is_review_subagent: bool,
        resolved_model_id: &str,
        provider_model_name: &str,
    ) -> Self {
        Self::for_agent_context(
            agent_type,
            is_review_subagent,
            ModelCapabilityProfile::from_resolved_model(resolved_model_id, provider_model_name),
        )
    }

    pub fn for_subagent_context_and_models(
        agent_type: &str,
        is_review_subagent: bool,
        subagent_model_id: Option<&str>,
        parent_agent_type: Option<&str>,
        parent_is_review_subagent: bool,
        parent_model_id: Option<&str>,
    ) -> Self {
        let child_profile = ContextProfile::for_agent_context(agent_type, is_review_subagent);
        let parent_profile = parent_agent_type
            .map(|agent_type| {
                ContextProfile::for_agent_context(agent_type, parent_is_review_subagent)
            })
            .unwrap_or(ContextProfile::Conversation);
        let profile = if child_profile == ContextProfile::LongTask
            || parent_profile == ContextProfile::LongTask
        {
            ContextProfile::LongTask
        } else {
            ContextProfile::Conversation
        };
        let model_capability = subagent_model_id
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
            .map(|model_id| ModelCapabilityProfile::from_model_id(Some(model_id)))
            .or_else(|| {
                parent_model_id
                    .map(str::trim)
                    .filter(|model_id| !model_id.is_empty())
                    .map(|model_id| ModelCapabilityProfile::from_model_id(Some(model_id)))
            })
            .unwrap_or(ModelCapabilityProfile::Standard);

        let mut policy = match profile {
            ContextProfile::LongTask => Self::long_task(),
            ContextProfile::Conversation => Self::conversation(),
        };
        if model_capability == ModelCapabilityProfile::Weak {
            policy.apply_weak_model_override();
        }
        policy
    }

    pub fn microcompact_config(&self) -> MicrocompactConfig {
        MicrocompactConfig {
            keep_recent: self.microcompact_keep_recent,
            trigger_ratio: self.microcompact_trigger_ratio,
        }
    }

    pub fn effective_subagent_max_concurrency(&self, configured: usize) -> usize {
        configured.clamp(1, self.subagent_concurrency_cap)
    }

    pub fn effective_loop_threshold(&self, configured: usize) -> usize {
        configured
            .max(1)
            .min(self.repeated_tool_signature_threshold.max(1))
    }

    pub fn has_repeated_tool_loop(&self, repeated_tool_signature_count: usize) -> bool {
        repeated_tool_signature_count >= self.repeated_tool_signature_threshold.max(1)
    }

    pub fn has_consecutive_command_failure_loop(&self, consecutive_failed_commands: usize) -> bool {
        consecutive_failed_commands >= self.consecutive_failed_command_threshold.max(1)
    }

    fn long_task() -> Self {
        let default_microcompact = MicrocompactConfig::default();
        Self {
            profile: ContextProfile::LongTask,
            microcompact_keep_recent: default_microcompact.keep_recent,
            microcompact_trigger_ratio: default_microcompact.trigger_ratio,
            compression_contract_limit: 8,
            subagent_concurrency_cap: 5,
            repeated_tool_signature_threshold: 3,
            consecutive_failed_command_threshold: 2,
        }
    }

    fn conversation() -> Self {
        Self {
            profile: ContextProfile::Conversation,
            microcompact_keep_recent: 12,
            microcompact_trigger_ratio: 0.65,
            compression_contract_limit: 4,
            subagent_concurrency_cap: 2,
            repeated_tool_signature_threshold: 4,
            consecutive_failed_command_threshold: 3,
        }
    }

    fn apply_weak_model_override(&mut self) {
        self.microcompact_keep_recent = self.microcompact_keep_recent.min(8);
        self.compression_contract_limit = self.compression_contract_limit.min(4);
        self.subagent_concurrency_cap = self.subagent_concurrency_cap.min(2);
        self.repeated_tool_signature_threshold = self.repeated_tool_signature_threshold.min(2);
        self.consecutive_failed_command_threshold =
            self.consecutive_failed_command_threshold.min(2);
    }
}

fn is_long_task_agent(agent_type: &str) -> bool {
    matches!(
        agent_type,
        "agentic" | "DeepReview" | "DeepResearch" | "ComputerUse" | "Team"
    ) || agent_type.starts_with("Review")
}
