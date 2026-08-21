//! Local customizations ported onto the upstream feature-sliced runtime-ports
//! module structure (20260812 sync of `perf(build)!: slice portable contract
//! capabilities`).
//!
//! These types are local-only (BitFun fork customizations). Upstream split the
//! monolithic `lib.rs` into owner-scoped feature modules; the GroupChatActor /
//! AgentType / steering additions below are not part of upstream and
//! must be preserved for the fork's 群聊参与者标识 / steering.

use serde::{Deserialize, Serialize};

/// Shared agent type used by SessionControl and SessionMessage tools.
///
/// Known built-in variants have canonical serde representations:
/// - `Agentic` → `"agentic"` (canonical)
/// - `Plan` → `"Plan"` (canonical)
/// - `Cowork` → `"Cowork"` (canonical)
/// - `DeepResearch` → `"DeepResearch"` (canonical)
/// - `Group` → `"group"` (canonical)
///
/// Any unrecognised string deserializes into `Other(String)`, so the enum
/// automatically tolerates agent types added by custom or external registries
/// without requiring a crate-level code change.
///
/// Serde is hand-implemented (not derived) because `#[serde(untagged)]` does
/// **not** participate in string matching for unit variants: `rename`/`alias`
/// attributes are ignored and every unit variant would be shadowed by
/// `Other(String)` (serializing a built-in variant produced `null`, and
/// deserializing `"group"` produced `Other("group")`). The hand-written impls
/// route through `as_str()` / `From<&str>`, so the single matching logic is
/// authoritative for both in-memory conversion and the wire representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentType {
    /// Known built-in variant: `agentic`.
    Agentic,
    /// Known built-in variant: `Plan`.
    Plan,
    /// Known built-in variant: `Cowork`.
    Cowork,
    /// Known built-in variant: `DeepResearch` (official research agent).
    DeepResearch,
    /// Known built-in variant: `group`.
    Group,
    /// Catch-all for any agent type string not in the known set (custom / external).
    Other(String),
}

impl Serialize for AgentType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgentType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value.as_str()))
    }
}

impl AgentType {
    /// Returns the canonical wire representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
            Self::DeepResearch => "DeepResearch",
            Self::Group => "group",
            Self::Other(value) => value.as_str(),
        }
    }

    /// Default agent type used when none is specified.
    pub const fn default_value() -> Self {
        Self::Agentic
    }

    /// Returns `true` if this is one of the three known built-in variants.
    pub fn is_known_builtin(&self) -> bool {
        matches!(
            self,
            Self::Agentic | Self::Plan | Self::Cowork | Self::DeepResearch | Self::Group
        )
    }
}

impl From<&str> for AgentType {
    fn from(value: &str) -> Self {
        match value {
            "agentic" | "Agentic" | "AGENTIC" => Self::Agentic,
            "Plan" | "plan" | "PLAN" => Self::Plan,
            "Cowork" | "cowork" | "COWORK" => Self::Cowork,
            "DeepResearch" | "deepresearch" | "DEEPRESEARCH" => Self::DeepResearch,
            "group" | "Group" | "GROUP" => Self::Group,
            other => Self::Other(other.to_string()),
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// GroupChat actor identifiers (local fork customization, 常开)
// ---------------------------------------------------------------------------

/// 主人保留字（P0-2 修复）：主人无 Claw session_id，用保留字标识。
/// 权限校验对主人开例外通道（建群/拉人/发言全通）。
pub const GROUP_MASTER_ACTOR: &str = "__master__";

/// 群聊参与者（P0-2 修复 + 复审 P0-1 修复：tag 化序列化，对齐 runtime-ports lib.rs 惯例）
/// 序列化形态（internally tagged，与 TS 一致）：
///   Master → {"kind":"master"}
///   Claw   → {"kind":"claw","sessionId":"...","agentType":"Claw"}
///   All    → {"kind":"all"}（@全体，复审 P1-4 修复：显式语义，非空数组哨兵）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupChatActor {
    Master, // 主人（__master__ 保留字）
    #[serde(rename_all = "camelCase")]
    Claw {
        session_id: String,
        agent_type: String,
    }, // Claw 助理会话（字段 camelCase 对齐 TS）
    All,    // @全体（P1-4 修复）
}

// ---------------------------------------------------------------------------
// Steering / fission helpers (local fork customization)
// ---------------------------------------------------------------------------

/// RoundInjection steering-dedup marker (TOKEN-01).
///
/// The caller-supplied steering id uniquely identifies this user-steering
/// event end to end (the scheduler generates it in `buffer_steering` as
/// `Uuid::new_v4()`). `UserSteering` injections always carry it; the other
/// kinds return `None`.
#[cfg(feature = "agent-api")]
pub fn round_injection_dedup_key(injection: &super::RoundInjection) -> Option<&str> {
    use super::RoundInjectionKind;
    match injection.kind {
        RoundInjectionKind::UserSteering => Some(injection.id.as_str()),
        RoundInjectionKind::BackgroundResult | RoundInjectionKind::ThreadGoalObjectiveUpdated => {
            None
        }
    }
}

/// Appends a prepended reminder to a round injection in place (local helper).
#[cfg(feature = "agent-api")]
pub fn round_injection_push_reminder(
    injection: &mut super::RoundInjection,
    kind: impl Into<String>,
    text: impl Into<String>,
) {
    injection
        .prepended_reminders
        .push(super::AgentDialogPrependedReminder {
            kind: kind.into(),
            text: text.into(),
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_type_round_trips_all_variants() {
        assert_eq!(AgentType::from("agentic"), AgentType::Agentic);
        assert_eq!(AgentType::from("Plan"), AgentType::Plan);
        assert_eq!(AgentType::from("cowork"), AgentType::Cowork);
        assert_eq!(AgentType::from("DEEPRESEARCH"), AgentType::DeepResearch);
        assert_eq!(AgentType::from("group"), AgentType::Group);
        assert_eq!(AgentType::from("Group"), AgentType::Group);
        assert_eq!(AgentType::from("GROUP"), AgentType::Group);
        assert_eq!(
            AgentType::from("custom-x"),
            AgentType::Other("custom-x".to_string())
        );
        assert_eq!(AgentType::default_value(), AgentType::Agentic);
        assert!(AgentType::Agentic.is_known_builtin());
        assert!(AgentType::Group.is_known_builtin());
        assert_eq!(AgentType::Group.as_str(), "group");
        assert!(!AgentType::Other("x".to_string()).is_known_builtin());
        assert_eq!(AgentType::Other("x".to_string()).to_string(), "x");
    }

    /// 复审 P0-1 负例断言：`#[serde(untagged)]` 下 unit variant 的
    /// rename/alias 不参与字符串匹配（builtin 会落 Other / 序列化为 null），
    /// 手写 Serialize/Deserialize 后必须全量覆盖——含存量 4 variant。
    #[test]
    fn agent_type_serde_deserializes_string_to_builtin_variants() {
        for (raw, expected) in [
            ("agentic", AgentType::Agentic),
            ("Plan", AgentType::Plan),
            ("Cowork", AgentType::Cowork),
            ("DeepResearch", AgentType::DeepResearch),
            ("group", AgentType::Group),
        ] {
            let parsed: AgentType = serde_json::from_str(&format!("\"{raw}\"")).unwrap();
            assert_eq!(
                parsed, expected,
                "from_str({raw:?}) must map to {expected:?}"
            );
            assert!(
                parsed.is_known_builtin(),
                "deserialized {raw:?} must be a known builtin"
            );
        }
        // 别名也按 From<&str> 语义走（"Group"/"GROUP" → Group）。
        for raw in ["Group", "GROUP", "plan", "PLAN", "cowork", "COWORK"] {
            let parsed: AgentType = serde_json::from_str(&format!("\"{raw}\"")).unwrap();
            assert!(
                parsed.is_known_builtin(),
                "alias {raw:?} must deserialize to a builtin variant"
            );
        }
        // 未知字符串仍落入 Other，且不 panic。
        let parsed: AgentType = serde_json::from_str("\"custom-x\"").unwrap();
        assert_eq!(parsed, AgentType::Other("custom-x".to_string()));
        assert!(!parsed.is_known_builtin());
    }

    #[test]
    fn agent_type_serde_serializes_builtin_variants_as_canonical_strings() {
        for (variant, expected) in [
            (AgentType::Agentic, "agentic"),
            (AgentType::Plan, "Plan"),
            (AgentType::Cowork, "Cowork"),
            (AgentType::DeepResearch, "DeepResearch"),
            (AgentType::Group, "group"),
        ] {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serialized,
                format!("\"{expected}\""),
                "serialize({variant:?}) must be the canonical string, not null"
            );
        }
        assert_eq!(
            serde_json::to_string(&AgentType::Other("x".to_string())).unwrap(),
            "\"x\""
        );
    }
}
