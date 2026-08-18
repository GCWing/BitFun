//! Shadow-only Agentic Core decision boundary.
//!
//! These DTOs let BitFun compare a future Agentic Core decision with existing
//! runtime ownership without adding a second executor, provider, permission
//! manager, session store, or UI truth source.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionSurface {
    Desktop,
    AppServer,
    Cli,
    PeerHost,
    SdkHost,
    Cron,
    DispatchWorker,
    DispatchController,
    ToolApi,
    McpApp,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionTargetKind {
    SessionTurn,
    ToolCall,
    McpCall,
    ProviderCall,
    Job,
    HostAdmin,
    ReadOnly,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionPolicyResult {
    Allow,
    Deny,
    ApprovalRequired,
    NotEvaluated,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionIntent {
    Execute,
    Delegate,
    Ask,
    Pause,
    Deny,
    Resume,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionRouteClass {
    GovernedRuntime,
    SurfaceAdmin,
    ReadOnly,
    DirectGap,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgenticDecisionLineage {
    pub decision_id: String,
    pub core_run_id: String,
    pub task_id: String,
    pub action_id: String,
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgenticDecisionPortRequest {
    pub lineage: AgenticDecisionLineage,
    pub surface: AgenticDecisionSurface,
    pub target_kind: AgenticDecisionTargetKind,
    pub route: String,
    pub intent: AgenticDecisionIntent,
    pub policy_result: AgenticDecisionPolicyResult,
    pub bitfun_policy_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub execute_physical: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgenticDecisionShadowStatus {
    ShadowAllowed,
    ShadowDenied,
    ShadowConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgenticDecisionPortProjection {
    pub adapter_version: String,
    pub lineage: AgenticDecisionLineage,
    pub surface: AgenticDecisionSurface,
    pub target_kind: AgenticDecisionTargetKind,
    pub route: String,
    pub route_class: AgenticDecisionRouteClass,
    pub intent: AgenticDecisionIntent,
    pub policy_result: AgenticDecisionPolicyResult,
    pub status: AgenticDecisionShadowStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denial_reason: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub execute_physical: bool,
}

pub const AGENTIC_DECISION_PORT_ADAPTER_VERSION: &str = "bitfun-agentic-decision-port.v1";

impl AgenticDecisionPortRequest {
    #[must_use]
    pub fn project_shadow(self) -> AgenticDecisionPortProjection {
        let route_class = classify_agentic_decision_route(self.surface, self.route.as_str());
        let denial_reason = self.shadow_denial_reason(route_class);
        let status = match denial_reason.as_deref() {
            Some("policy_conflict") => AgenticDecisionShadowStatus::ShadowConflict,
            Some(_) => AgenticDecisionShadowStatus::ShadowDenied,
            None => AgenticDecisionShadowStatus::ShadowAllowed,
        };

        AgenticDecisionPortProjection {
            adapter_version: AGENTIC_DECISION_PORT_ADAPTER_VERSION.to_string(),
            lineage: self.lineage,
            surface: self.surface,
            target_kind: self.target_kind,
            route: self.route,
            route_class,
            intent: self.intent,
            policy_result: self.policy_result,
            status,
            denial_reason,
            evidence_refs: self.evidence_refs,
            execute_physical: false,
        }
    }

    fn shadow_denial_reason(&self, route_class: AgenticDecisionRouteClass) -> Option<String> {
        if self.execute_physical {
            return Some("shadow_mode_forbids_physical_execution".to_string());
        }
        if route_class == AgenticDecisionRouteClass::DirectGap {
            return Some(format!("route_gap:{}", self.route));
        }
        if route_class == AgenticDecisionRouteClass::Unknown {
            return Some(format!("unknown_route_owner:{}", self.route));
        }
        if matches!(self.policy_result, AgenticDecisionPolicyResult::Conflict) {
            return Some("policy_conflict".to_string());
        }
        if matches!(self.intent, AgenticDecisionIntent::Execute) && !self.bitfun_policy_available {
            return Some("bitfun_policy_unavailable".to_string());
        }
        if matches!(
            self.policy_result,
            AgenticDecisionPolicyResult::ApprovalRequired
        ) && self.approval_id.is_none()
        {
            return Some("approval_required_without_approval_id".to_string());
        }
        if matches!(
            self.target_kind,
            AgenticDecisionTargetKind::SessionTurn
                | AgenticDecisionTargetKind::ToolCall
                | AgenticDecisionTargetKind::McpCall
        ) && (self.lineage.session_id.is_none() || self.lineage.turn_id.is_none())
        {
            return Some("missing_session_turn_lineage".to_string());
        }
        None
    }
}

#[must_use]
pub fn classify_agentic_decision_route(
    surface: AgenticDecisionSurface,
    route: &str,
) -> AgenticDecisionRouteClass {
    match (surface, route) {
        (AgenticDecisionSurface::ToolApi, "execute_tool") => AgenticDecisionRouteClass::DirectGap,
        (AgenticDecisionSurface::McpApp, "send_mcp_app_message.tools_call") => {
            AgenticDecisionRouteClass::DirectGap
        }
        (AgenticDecisionSurface::McpApp, "send_mcp_app_message.resources_read" | "ping") => {
            AgenticDecisionRouteClass::ReadOnly
        }
        (AgenticDecisionSurface::DispatchController, _) => {
            AgenticDecisionRouteClass::SurfaceAdmin
        }
        (
            AgenticDecisionSurface::Desktop
            | AgenticDecisionSurface::AppServer
            | AgenticDecisionSurface::Cli
            | AgenticDecisionSurface::PeerHost
            | AgenticDecisionSurface::SdkHost
            | AgenticDecisionSurface::Cron
            | AgenticDecisionSurface::DispatchWorker,
            _,
        ) => AgenticDecisionRouteClass::GovernedRuntime,
        _ => AgenticDecisionRouteClass::Unknown,
    }
}
