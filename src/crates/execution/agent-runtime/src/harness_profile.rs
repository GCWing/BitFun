use bitfun_core_types::{
    HarnessProfileId, BALANCED_HARNESS_PROFILE_ID, MINIMAL_HARNESS_PROFILE_ID,
    ULTIMATE_HARNESS_PROFILE_ID,
};

pub const BALANCED_PROMPT_POLICY_ID: &str = "agentic-mode-v1";
pub const BALANCED_TOOL_PROFILE_ID: &str = "coding-balanced-v1";
pub const MINIMAL_PROMPT_POLICY_ID: &str = "minimal_harness_v1";
pub const MINIMAL_TOOL_PROFILE_ID: &str = "coding-minimal-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessProfileDescriptor {
    pub id: HarnessProfileId,
    pub schema_version: u32,
    pub prompt_policy_id: &'static str,
    pub tool_profile_id: &'static str,
    pub available: bool,
    pub unavailable_reason: Option<&'static str>,
    /// `None` means no fixed model-round ceiling. Cancellation, context,
    /// permission, provider and general no-progress protections still apply.
    pub max_model_rounds: Option<usize>,
}

pub fn harness_profile_catalog(default_max_rounds: usize) -> Vec<HarnessProfileDescriptor> {
    vec![
        HarnessProfileDescriptor {
            id: HarnessProfileId::new(MINIMAL_HARNESS_PROFILE_ID),
            schema_version: 1,
            prompt_policy_id: MINIMAL_PROMPT_POLICY_ID,
            tool_profile_id: MINIMAL_TOOL_PROFILE_ID,
            available: true,
            unavailable_reason: None,
            max_model_rounds: None,
        },
        HarnessProfileDescriptor {
            id: HarnessProfileId::new(BALANCED_HARNESS_PROFILE_ID),
            schema_version: 1,
            prompt_policy_id: BALANCED_PROMPT_POLICY_ID,
            tool_profile_id: BALANCED_TOOL_PROFILE_ID,
            available: true,
            unavailable_reason: None,
            max_model_rounds: Some(default_max_rounds),
        },
        HarnessProfileDescriptor {
            id: HarnessProfileId::new(ULTIMATE_HARNESS_PROFILE_ID),
            schema_version: 1,
            prompt_policy_id: "ultimate-unavailable",
            tool_profile_id: "ultimate-unavailable",
            available: false,
            unavailable_reason: Some("ultimate Harness Profile is not implemented"),
            max_model_rounds: Some(default_max_rounds),
        },
    ]
}

pub fn resolve_harness_profile(
    id: &HarnessProfileId,
    default_max_rounds: usize,
) -> Result<HarnessProfileDescriptor, String> {
    let descriptor = harness_profile_catalog(default_max_rounds)
        .into_iter()
        .find(|descriptor| descriptor.id == *id)
        .ok_or_else(|| format!("unknown Harness Profile: {id}"))?;
    if !descriptor.available {
        return Err(descriptor
            .unavailable_reason
            .unwrap_or("Harness Profile is unavailable")
            .to_string());
    }
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_has_no_fixed_model_round_limit() {
        let profile =
            resolve_harness_profile(&HarnessProfileId::new(MINIMAL_HARNESS_PROFILE_ID), 200)
                .unwrap();
        assert_eq!(profile.max_model_rounds, None);
    }

    #[test]
    fn balanced_keeps_configured_model_round_limit() {
        let profile =
            resolve_harness_profile(&HarnessProfileId::new(BALANCED_HARNESS_PROFILE_ID), 123)
                .unwrap();
        assert_eq!(profile.max_model_rounds, Some(123));
    }
}
