//! Adaptive context profile policy.
//!
//! Profiles keep context behavior aligned with the shape of the agent workload
//! without exposing more knobs to the UI.

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

        // Weak model detection: match suffix-based markers (e.g., "gpt-4o-mini",
        // "gemini-1.5-flash") and exact markers (e.g., "haiku", "mini").
        // Avoid false positives from substring matches (e.g., "gemini-pro" should
        // NOT match "mini" inside "gemini").
        let weak_suffixes = ["-haiku", "-mini", "-small", "-lite", "-flash", "-nano"];
        let weak_exact = ["haiku", "mini", "small", "lite", "flash", "nano"];
        // Also match known weak model name patterns where the marker appears
        // mid-string but is a genuine weak model (e.g., "claude-3-haiku-20240307").
        let weak_mid_patterns = [
            "-haiku-", "-mini-", "-small-", "-lite-", "-flash-", "-nano-",
        ];
        if weak_suffixes.iter().any(|s| normalized.ends_with(s))
            || weak_exact.iter().any(|e| normalized == *e)
            || weak_mid_patterns.iter().any(|p| normalized.contains(p))
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
        Self {
            profile: ContextProfile::LongTask,
            compression_contract_limit: 8,
            subagent_concurrency_cap: 5,
            repeated_tool_signature_threshold: 3,
            consecutive_failed_command_threshold: 2,
        }
    }

    fn conversation() -> Self {
        Self {
            profile: ContextProfile::Conversation,
            compression_contract_limit: 4,
            subagent_concurrency_cap: 2,
            repeated_tool_signature_threshold: 4,
            consecutive_failed_command_threshold: 3,
        }
    }

    fn apply_weak_model_override(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::ModelCapabilityProfile;

    #[test]
    fn model_capability_standard_for_empty_or_none() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(None),
            ModelCapabilityProfile::Standard
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("")),
            ModelCapabilityProfile::Standard
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("  ")),
            ModelCapabilityProfile::Standard
        );
    }

    #[test]
    fn model_capability_standard_for_strong_models() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("gpt-4o")),
            ModelCapabilityProfile::Standard
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("claude-sonnet-4")),
            ModelCapabilityProfile::Standard
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("gemini-pro")),
            ModelCapabilityProfile::Standard
        );
    }

    #[test]
    fn model_capability_weak_for_haiku() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("claude-3-haiku-20240307")),
            ModelCapabilityProfile::Weak
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("anthropic/claude-3-haiku")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_weak_for_mini() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("gpt-4o-mini")),
            ModelCapabilityProfile::Weak
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("openai/gpt-4o-mini")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_weak_for_flash() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("gemini-1.5-flash")),
            ModelCapabilityProfile::Weak
        );
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("google/gemini-flash")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_weak_for_lite() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("qwen-lite")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_weak_for_small() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("llama-small")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_weak_for_nano() {
        assert_eq!(
            ModelCapabilityProfile::from_model_id(Some("gemini-nano")),
            ModelCapabilityProfile::Weak
        );
    }

    #[test]
    fn model_capability_from_resolved_model_prefers_resolved() {
        // resolved is weak → returns weak regardless of provider name
        assert_eq!(
            ModelCapabilityProfile::from_resolved_model("gpt-4o-mini", "gpt-4o"),
            ModelCapabilityProfile::Weak
        );
        // resolved is standard, provider is weak → returns weak
        assert_eq!(
            ModelCapabilityProfile::from_resolved_model("gpt-4o", "gpt-4o-mini"),
            ModelCapabilityProfile::Weak
        );
        // both standard → returns standard
        assert_eq!(
            ModelCapabilityProfile::from_resolved_model("gpt-4o", "claude-sonnet"),
            ModelCapabilityProfile::Standard
        );
    }
}
