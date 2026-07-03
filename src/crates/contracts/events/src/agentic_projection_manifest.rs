use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticEventProjectionAggregate {
    Session,
    Turn,
    ModelRound,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticEventProjectionReplayPolicy {
    LiveOnly,
    Replayable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticEventProjectionRetentionPolicy {
    Ephemeral,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticEventProjectionUiShape {
    LegacyFlat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgenticEventProjectionManifestEntry {
    pub event_name: &'static str,
    pub event_type: &'static str,
    pub version: u16,
    pub aggregate: AgenticEventProjectionAggregate,
    pub replay: AgenticEventProjectionReplayPolicy,
    pub retention: AgenticEventProjectionRetentionPolicy,
    pub ui_shape: AgenticEventProjectionUiShape,
    pub legacy_websocket: bool,
}

const fn projection_manifest_entry(
    event_name: &'static str,
    event_type: &'static str,
    aggregate: AgenticEventProjectionAggregate,
    replay: AgenticEventProjectionReplayPolicy,
    legacy_websocket: bool,
) -> AgenticEventProjectionManifestEntry {
    AgenticEventProjectionManifestEntry {
        event_name,
        event_type,
        version: 1,
        aggregate,
        replay,
        retention: match replay {
            AgenticEventProjectionReplayPolicy::LiveOnly => {
                AgenticEventProjectionRetentionPolicy::Ephemeral
            }
            AgenticEventProjectionReplayPolicy::Replayable => {
                AgenticEventProjectionRetentionPolicy::Session
            }
        },
        ui_shape: AgenticEventProjectionUiShape::LegacyFlat,
        legacy_websocket,
    }
}

pub const AGENTIC_EVENT_PROJECTION_MANIFEST: &[AgenticEventProjectionManifestEntry] = &[
    projection_manifest_entry(
        "agentic://session-created",
        "session-created",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://session-deleted",
        "session-deleted",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        false,
    ),
    projection_manifest_entry(
        "agentic://image-analysis-started",
        "image-analysis-started",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        true,
    ),
    projection_manifest_entry(
        "agentic://image-analysis-completed",
        "image-analysis-completed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        true,
    ),
    projection_manifest_entry(
        "agentic://dialog-turn-started",
        "dialog-turn-started",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://subagent-session-linked",
        "subagent-session-linked",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://model-round-started",
        "model-round-started",
        AgenticEventProjectionAggregate::ModelRound,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://text-chunk",
        "text-chunk",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        true,
    ),
    projection_manifest_entry(
        "agentic://tool-event",
        "tool-event",
        AgenticEventProjectionAggregate::Tool,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        true,
    ),
    projection_manifest_entry(
        "agentic://dialog-turn-completed",
        "dialog-turn-completed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "session_title_generated",
        "session_title_generated",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        false,
    ),
    projection_manifest_entry(
        "agentic://dialog-turn-cancelled",
        "dialog-turn-cancelled",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://dialog-turn-failed",
        "dialog-turn-failed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://token-usage-updated",
        "token-usage-updated",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://context-compression-started",
        "context-compression-started",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        false,
    ),
    projection_manifest_entry(
        "agentic://context-compression-completed",
        "context-compression-completed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://context-compression-failed",
        "context-compression-failed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://thread-goal-updated",
        "thread-goal-updated",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://session-state-changed",
        "session-state-changed",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://session-model-auto-migrated",
        "session-model-auto-migrated",
        AgenticEventProjectionAggregate::Session,
        AgenticEventProjectionReplayPolicy::Replayable,
        false,
    ),
    projection_manifest_entry(
        "agentic://deep-review-queue-state-changed",
        "deep-review-queue-state-changed",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        true,
    ),
    projection_manifest_entry(
        "agentic://model-round-completed",
        "model-round-completed",
        AgenticEventProjectionAggregate::ModelRound,
        AgenticEventProjectionReplayPolicy::Replayable,
        true,
    ),
    projection_manifest_entry(
        "agentic://user-steering-injected",
        "user-steering-injected",
        AgenticEventProjectionAggregate::Turn,
        AgenticEventProjectionReplayPolicy::LiveOnly,
        false,
    ),
];

pub fn public_agentic_event_projection_manifest() -> &'static [AgenticEventProjectionManifestEntry]
{
    AGENTIC_EVENT_PROJECTION_MANIFEST
}

pub fn agentic_event_projection_manifest_entry(
    event_type: &str,
) -> Option<&'static AgenticEventProjectionManifestEntry> {
    AGENTIC_EVENT_PROJECTION_MANIFEST
        .iter()
        .find(|entry| entry.event_type == event_type)
}

pub fn is_legacy_websocket_agentic_event_type(event_type: &str) -> bool {
    agentic_event_projection_manifest_entry(event_type).is_some_and(|entry| entry.legacy_websocket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_event_projection_manifest_describes_projected_events_and_websocket_allowlist() {
        let manifest = public_agentic_event_projection_manifest();
        let text_chunk = agentic_event_projection_manifest_entry("text-chunk")
            .expect("text chunk manifest entry");

        assert!(manifest.len() >= 20);
        assert_eq!(text_chunk.event_name, "agentic://text-chunk");
        assert_eq!(text_chunk.version, 1);
        assert_eq!(text_chunk.aggregate, AgenticEventProjectionAggregate::Turn);
        assert_eq!(
            text_chunk.retention,
            AgenticEventProjectionRetentionPolicy::Ephemeral
        );
        assert_eq!(
            text_chunk.ui_shape,
            AgenticEventProjectionUiShape::LegacyFlat
        );
        assert!(is_legacy_websocket_agentic_event_type("text-chunk"));
        assert!(!is_legacy_websocket_agentic_event_type("session-deleted"));
    }

    #[test]
    fn public_event_projection_manifest_covers_current_frontend_projection_types() {
        let mut projected_event_types = [
            "session-created",
            "session-deleted",
            "image-analysis-started",
            "image-analysis-completed",
            "dialog-turn-started",
            "subagent-session-linked",
            "model-round-started",
            "text-chunk",
            "tool-event",
            "dialog-turn-completed",
            "session_title_generated",
            "dialog-turn-cancelled",
            "dialog-turn-failed",
            "token-usage-updated",
            "context-compression-started",
            "context-compression-completed",
            "context-compression-failed",
            "thread-goal-updated",
            "session-state-changed",
            "session-model-auto-migrated",
            "deep-review-queue-state-changed",
            "model-round-completed",
            "user-steering-injected",
        ];

        for event_type in projected_event_types.iter().copied() {
            assert!(
                agentic_event_projection_manifest_entry(event_type).is_some(),
                "missing projection manifest entry for projected event type {event_type}"
            );
        }

        let mut manifest_event_types = public_agentic_event_projection_manifest()
            .iter()
            .map(|entry| entry.event_type)
            .collect::<Vec<_>>();
        manifest_event_types.sort_unstable();
        projected_event_types.sort_unstable();

        assert_eq!(manifest_event_types, projected_event_types);
    }

    #[test]
    fn public_event_projection_manifest_has_unique_event_types_and_exact_legacy_websocket_allowlist(
    ) {
        let manifest = public_agentic_event_projection_manifest();
        let mut event_types = manifest
            .iter()
            .map(|entry| entry.event_type)
            .collect::<Vec<_>>();
        event_types.sort_unstable();
        event_types.dedup();

        assert_eq!(event_types.len(), manifest.len());

        let legacy_websocket_event_types = manifest
            .iter()
            .filter_map(|entry| entry.legacy_websocket.then_some(entry.event_type))
            .collect::<Vec<_>>();

        assert_eq!(
            legacy_websocket_event_types,
            vec![
                "image-analysis-started",
                "image-analysis-completed",
                "dialog-turn-started",
                "subagent-session-linked",
                "model-round-started",
                "text-chunk",
                "tool-event",
                "dialog-turn-completed",
                "token-usage-updated",
                "thread-goal-updated",
                "deep-review-queue-state-changed",
                "model-round-completed",
            ]
        );
    }
}
