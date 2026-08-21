use crate::{
    classify_tool_call, validate_deferred_tool_usage, validate_tool_allowed_by_list,
    DeferredToolUsageError, LoadedDeferredToolSpec, ToolExecutionAccessError, ToolRestrictionError,
    ToolRuntimeRestrictions,
};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub struct ToolExecutionAdmissionRequest<'a> {
    pub tool_name: &'a str,
    pub allowed_tools: &'a [String],
    pub runtime_tool_restrictions: &'a ToolRuntimeRestrictions,
    /// User-enabled tool set (mode default + agent-profile added/removed
    /// resolution, BEFORE dynamic MCP tools are merged in). The runtime gate
    /// unions this with the role template whitelist so the front-end agent
    /// profile checkbox state and RBAC enforcement stay in sync: a checked
    /// tool executes, an unchecked one stays blocked even when visible.
    pub user_enabled_tools: &'a [String],
    pub tool_arguments: &'a Value,
    pub deferred_tools: &'a [String],
    pub loaded_deferred_tool_specs: &'a [LoadedDeferredToolSpec],
    pub current_catalog_generation: u64,
    pub get_tool_spec_tool_name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionAdmissionRejection {
    AllowedList(ToolExecutionAccessError),
    RuntimeRestriction(ToolRestrictionError),
    Deferred(DeferredToolUsageError),
}

impl fmt::Display for ToolExecutionAdmissionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowedList(error) => write!(formatter, "{error}"),
            Self::RuntimeRestriction(error) => write!(formatter, "{error}"),
            Self::Deferred(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ToolExecutionAdmissionRejection {}

pub fn validate_tool_execution_admission(
    request: ToolExecutionAdmissionRequest<'_>,
) -> Result<(), ToolExecutionAdmissionRejection> {
    validate_tool_allowed_by_list(request.tool_name, request.allowed_tools)
        .map_err(ToolExecutionAdmissionRejection::AllowedList)?;
    // RBAC ↔ config 联动：模板白名单 ∪ 用户启用集合（前端勾选即执行可用）。
    // deny 列表语义不变（降级角色/子代理 deny 优先于放行）；user_enabled_tools
    // 为空（SubAgent/Hidden/无 profile 覆盖）时并集 = 模板白名单，行为逐字节不变。
    //
    // 内部网关（GetToolSpec/CallDeferredTool）不参与 user_enabled 并集：
    // 它们由 runtime_tool_restrictions 模板独立管辖（Commander/GeneralPurpose
    // 模板已显式包含）。若把网关从并集结果中排除（旧实现），主会话
    // （agentic/Legion 等 Mode 类，user_enabled_tools = 模式 default 工具集非空）
    // 的并集白名单会变成不含网关的窄集，导致 GetToolSpec 被
    // ensure_tool_allowed 拦截 → 全部 deferred 工具（SessionMessage/
    // SessionControl/ListModels 等）无法解锁（2026-08-10 实测回归）。
    // 网关工具跳过并集路径，直接用原始模板校验（模板含网关或白名单空
    // = 全放行时均通过）。
    let is_internal_gateway = request.tool_name == request.get_tool_spec_tool_name
        || request.tool_name == "CallDeferredTool";
    let effective_restrictions = if request.user_enabled_tools.is_empty() || is_internal_gateway {
        request.runtime_tool_restrictions.clone()
    } else {
        let mut expanded = request.runtime_tool_restrictions.clone();
        for tool_name in request.user_enabled_tools {
            // 不把内部网关纳入联动放行（仅模型可见性管辖）；deny 仍优先。
            if tool_name == request.get_tool_spec_tool_name || tool_name == "CallDeferredTool" {
                continue;
            }
            expanded.allowed_tool_names.insert(tool_name.clone());
        }
        expanded
    };
    effective_restrictions
        .ensure_tool_allowed(request.tool_name)
        .map_err(ToolExecutionAdmissionRejection::RuntimeRestriction)?;
    request
        .runtime_tool_restrictions
        .ensure_operation_allowed(
            classify_tool_call(request.tool_name, request.tool_arguments),
            request.tool_name,
        )
        .map_err(ToolExecutionAdmissionRejection::RuntimeRestriction)?;
    validate_deferred_tool_usage(
        request.tool_name,
        request.deferred_tools,
        request.loaded_deferred_tool_specs,
        request.current_catalog_generation,
        request.get_tool_spec_tool_name,
    )
    .map_err(ToolExecutionAdmissionRejection::Deferred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GET_TOOL_SPEC_TOOL_NAME;
    use serde_json::json;

    /// Commander 模板（窄白名单）＋ 前端勾选（user_enabled_tools）联动的执行准入。
    fn admission(
        tool_name: &str,
        restrictions: &ToolRuntimeRestrictions,
        user_enabled_tools: &[&str],
        allowed_tools: &[&str],
        deferred_tools: &[&str],
    ) -> Result<(), ToolExecutionAdmissionRejection> {
        let user_enabled: Vec<String> = user_enabled_tools.iter().map(|s| s.to_string()).collect();
        let allowed: Vec<String> = allowed_tools.iter().map(|s| s.to_string()).collect();
        let deferred: Vec<String> = deferred_tools.iter().map(|s| s.to_string()).collect();
        validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name,
            allowed_tools: &allowed,
            runtime_tool_restrictions: restrictions,
            user_enabled_tools: &user_enabled,
            tool_arguments: &json!({}),
            deferred_tools: &deferred,
            loaded_deferred_tool_specs: &[],
            current_catalog_generation: 0,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        })
    }

    fn commander_template() -> ToolRuntimeRestrictions {
        // 模拟 Commander 模板：白名单只含 subagent_default_tools 子集，
        // 操作类全量（ReadOnly + WriteFile + ExecuteCode，与真实模板一致）。
        let mut restrictions = ToolRuntimeRestrictions::default();
        restrictions.allowed_tool_names.insert("Read".to_string());
        restrictions.allowed_tool_names.insert("Write".to_string());
        restrictions
            .allowed_operation_classes
            .insert(crate::OperationClass::ReadOnly);
        restrictions
            .allowed_operation_classes
            .insert(crate::OperationClass::WriteFile);
        restrictions
            .allowed_operation_classes
            .insert(crate::OperationClass::ExecuteCode);
        restrictions
    }

    #[test]
    fn checked_tool_is_executable_through_user_enabled_union() {
        // WorkspaceScan 不在 Commander 模板白名单，但前端勾选 → 执行放行。
        let restrictions = commander_template();
        let result = admission(
            "WorkspaceScan",
            &restrictions,
            &["WorkspaceScan"],
            &["WorkspaceScan"],
            &[],
        );
        assert!(result.is_ok(), "checked tool must execute: {result:?}");
    }

    #[test]
    fn unchecked_tool_stays_blocked_even_when_visible() {
        // 未勾选的 MCP 工具在 allowed_tools（可见）但不在 user_enabled_tools →
        // 仍被门2a 模板白名单拦截。
        let restrictions = commander_template();
        let result = admission(
            "mcp__github__search_repos",
            &restrictions,
            &[],
            &["mcp__github__search_repos"],
            &[],
        );
        assert!(matches!(
            result,
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));
    }

    #[test]
    fn checked_mcp_tool_is_executable_through_user_enabled_union() {
        let restrictions = commander_template();
        let result = admission(
            "mcp__github__search_repos",
            &restrictions,
            &["mcp__github__search_repos"],
            &["mcp__github__search_repos"],
            &[],
        );
        assert!(result.is_ok(), "checked MCP tool must execute: {result:?}");
    }

    #[test]
    fn deny_list_still_prevails_over_user_enabled_union() {
        // 子代理 deny（ReviewPlatform）即使被勾选也拦截——安全层保留。
        let mut restrictions = commander_template();
        restrictions
            .denied_tool_names
            .insert("ReviewPlatform".to_string());
        let result = admission(
            "ReviewPlatform",
            &restrictions,
            &["ReviewPlatform"],
            &["ReviewPlatform"],
            &[],
        );
        assert!(matches!(
            result,
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));
    }

    #[test]
    fn empty_user_enabled_preserves_template_behavior() {
        // user_enabled_tools 为空（SubAgent/无 profile）→ 行为与原来完全一致。
        let restrictions = commander_template();
        assert!(admission("Read", &restrictions, &[], &["Read"], &[]).is_ok());
        assert!(admission("Write", &restrictions, &[], &["Write"], &[]).is_ok());
        assert!(matches!(
            admission("TodoWrite", &restrictions, &[], &["TodoWrite"], &[]),
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));
    }

    #[test]
    fn internal_gateway_names_are_not_expanded_by_union() {
        // 内部网关不放行逻辑不变：即使出现在 user_enabled_tools 也不并集。
        let restrictions = commander_template();
        let result = admission(
            "GetToolSpec",
            &restrictions,
            &["GetToolSpec"],
            &["GetToolSpec"],
            &[],
        );
        assert!(matches!(
            result,
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));
    }

    #[test]
    fn internal_gateway_bypasses_user_enabled_union_when_template_is_open() {
        // 主会话回归（2026-08-10）：agentic/Legion 等 Mode 类 agent 的
        // user_enabled_tools = 模式 default 工具集（非空，不含 GetToolSpec），
        // runtime_tool_restrictions = 空白名单（全放行）。旧实现把网关从
        // 并集结果中排除 → 白名单变成不含网关的窄集 → GetToolSpec 被拦 →
        // 全部 deferred 工具死循环。修复后网关跳过并集，直接走空白名单模板
        // = 全放行。
        let restrictions = ToolRuntimeRestrictions::default(); // 主会话 context 级默认
        let result = admission(
            "GetToolSpec",
            &restrictions,
            &["Read", "Write", "Grep", "Glob"], // 模式 default 工具集（不含网关）
            &[
                "Read",
                "Write",
                "Grep",
                "Glob",
                "GetToolSpec",
                "CallDeferredTool",
            ],
            &["WebFetch", "SessionMessage", "SessionControl", "ListModels"],
        );
        assert!(
            result.is_ok(),
            "GetToolSpec must pass when template allowlist is open: {result:?}"
        );

        let deferred = admission(
            "CallDeferredTool",
            &restrictions,
            &["Read", "Write"],
            &["Read", "Write", "GetToolSpec", "CallDeferredTool"],
            &["WebFetch"],
        );
        assert!(
            deferred.is_ok(),
            "CallDeferredTool must pass when template allowlist is open: {deferred:?}"
        );
    }

    #[test]
    fn internal_gateway_stays_blocked_when_template_denies() {
        // 网关放行仍受模板 deny 约束：模板显式 deny GetToolSpec 时必须拦截。
        let mut restrictions = ToolRuntimeRestrictions::default();
        restrictions
            .denied_tool_names
            .insert("GetToolSpec".to_string());
        let result = admission(
            "GetToolSpec",
            &restrictions,
            &["Read", "Write"],
            &["Read", "Write", "GetToolSpec", "CallDeferredTool"],
            &["WebFetch"],
        );
        assert!(matches!(
            result,
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));
    }

    #[test]
    fn main_session_open_template_still_blocks_unchecked_tools() {
        // 主会话语义（d1-P1-1 / L5-P1-1）：主会话 context 级限制为空模板
        // （全放行）。门 2a 在 user_enabled_tools 非空（Mode 类 agent 恒非空
        // = 模式 default 工具集）时并集出「精确勾选集合」，未勾选工具（含
        // MCP）必须被拦截——"未勾选=禁用"在主会话同样成立。
        let restrictions = ToolRuntimeRestrictions::default(); // 主会话 context 级空模板
        let result = admission(
            "mcp__github__search_repos",
            &restrictions,
            &["Read", "Write", "Grep", "Glob"], // 模式 default，未勾选 MCP
            &["Read", "Write", "Grep", "Glob", "mcp__github__search_repos"],
            &[],
        );
        assert!(matches!(
            result,
            Err(ToolExecutionAdmissionRejection::RuntimeRestriction(_))
        ));

        // 勾选后（进入 user_enabled）即可执行。
        let checked = admission(
            "mcp__github__search_repos",
            &restrictions,
            &["Read", "Write", "mcp__github__search_repos"],
            &["Read", "Write", "mcp__github__search_repos"],
            &[],
        );
        assert!(
            checked.is_ok(),
            "checked MCP tool must execute in main session: {checked:?}"
        );
    }
}
