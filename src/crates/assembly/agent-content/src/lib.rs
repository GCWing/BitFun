//! Immutable built-in Agent prompt content.
//!
//! Prompt selection, rendering, and runtime policy remain owned by the
//! product runtime. This crate owns only the release-bound text bytes.

include!(concat!(env!("OUT_DIR"), "/embedded_agent_prompts.rs"));

/// Prompt content used by the Memory product feature outside the generated catalog.
pub mod memories {
    pub const PHASE1_SYSTEM: &str = include_str!("../prompts/memories/phase1_system.md");
}

/// Prompt content used by the Insights product feature.
pub mod insights {
    pub const FACET_EXTRACTION: &str = include_str!("../prompts/insights/facet_extraction.md");
    pub const SUGGESTIONS: &str = include_str!("../prompts/insights/suggestions.md");
    pub const AREAS: &str = include_str!("../prompts/insights/areas.md");
    pub const WINS: &str = include_str!("../prompts/insights/wins.md");
    pub const FRICTION: &str = include_str!("../prompts/insights/friction.md");
    pub const INTERACTION_STYLE: &str = include_str!("../prompts/insights/interaction_style.md");
    pub const AT_A_GLANCE: &str = include_str!("../prompts/insights/at_a_glance.md");
    pub const HORIZON: &str = include_str!("../prompts/insights/horizon.md");
    pub const FUN_ENDING: &str = include_str!("../prompts/insights/fun_ending.md");
}
