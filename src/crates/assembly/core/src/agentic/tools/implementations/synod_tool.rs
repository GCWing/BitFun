//! Synod tool — Multi-model expert consensus engine.
//!
//! Launches independent experts (Councillor sub-agents) in parallel, each with
//! a potentially different model/provider. Collects all results privately
//! (never enters the calling session's context), then dispatches a Judge
//! sub-agent to synthesize a structured verdict.

use crate::agentic::coordination::{get_global_coordinator, SubagentExecutionRequest};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::SubagentContextMode;
use serde_json::{json, Value};
use std::time::Duration;

/// A single councillor definition from the input.
struct CouncillorTask {
    name: String,
    model: String,
    role: String,
}

/// Built-in preset configurations for common scenarios.
const PRESETS: &[(&str, &[(&str, &str, &str)])] = &[
    (
        "review",
        &[
            ("架构", "primary", "关注模块边界、API设计、依赖方向和扩展性"),
            ("安全", "primary", "关注OWASP Top 10、攻击面、权限校验和数据泄露"),
            ("性能", "fast", "关注延迟、并发瓶颈、N+1查询和资源消耗"),
        ],
    ),
    (
        "design",
        &[
            ("技术选型", "primary", "关注方案成熟度、生态系统、学习曲线和长期维护成本"),
            ("成本评估", "fast", "关注开发成本、维护成本、迁移成本和团队技能匹配"),
            ("可行性", "primary", "关注实施风险、外部依赖、时间线合理性"),
        ],
    ),
    (
        "deep",
        &[
            ("主审", "primary", "全面代码审查，关注正确性、边缘条件和稳定性"),
            ("对抗", "primary", "对抗性审查，专门找其他人可能遗漏的极端情况和隐藏假设"),
        ],
    ),
    (
        "startup",
        &[
            ("市场", "fast", "关注市场需求强度、竞品格局、差异化定位"),
            ("技术", "primary", "关注技术可行性、实现复杂度、技术债务积累风险"),
            ("产品", "fast", "关注用户体验、MVP范围、用户获取路径"),
        ],
    ),
];

pub struct SynodTool;

impl SynodTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SynodTool {
    fn name(&self) -> &str {
        "Synod"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Multi-model expert consensus engine. Launches independent experts (each with \
        a potentially different model/provider), collects their analyses privately \
        (results never enter your context), and returns a synthesized verdict. \
        Use when you need diverse perspectives on a complex decision, conflicting \
        review results, or high-stakes architectural choices."
            .to_string())
    }

    fn short_description(&self) -> String {
        "Multi-model expert consensus: parallel experts + synthesized verdict".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Short label for this Synod session, e.g. '评估微服务拆分方案'"
                },
                "prompt": {
                    "type": "string",
                    "description": "The question or problem for all experts to evaluate. Each expert also receives their own role guidance."
                },
                "preset": {
                    "type": "string",
                    "description": "Optional: name of a built-in expert preset. One of: review, design, deep, startup. When omitted, you must provide councilors explicitly."
                },
                "councillors": {
                    "type": "array",
                    "description": "Custom expert list (overrides preset when provided). Each entry defines name, model_id/slot, and role guidance.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Expert display name, e.g. '安全专家'" },
                            "model": { "type": "string", "description": "Model ID or slot (primary/fast), e.g. 'anthropic/claude-opus-4', 'primary', 'fast'" },
                            "role": { "type": "string", "description": "Role guidance for this expert, e.g. '关注OWASP Top 10'" }
                        }
                    }
                },
                "judge_model": {
                    "type": "string",
                    "description": "Optional model for the Judge synthesizer. Defaults to 'primary'."
                },
                "group_timeout_seconds": {
                    "type": "integer",
                    "description": "Max seconds to wait for ALL experts to complete. Each expert also has individual timeout. Default: 300."
                },
                "partial_tolerance": {
                    "type": "boolean",
                    "description": "If true, synthesize from successful results even when some experts fail/timeout. Default: true."
                }
            },
            "required": ["prompt"]
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::tool("Missing required field: prompt".to_string()))?
            .to_string();

        let description = input
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("Synod evaluation");

        let judge_model = input
            .get("judge_model")
            .and_then(Value::as_str)
            .unwrap_or("primary")
            .to_string();

        let group_timeout = input
            .get("group_timeout_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(300);

        let partial_tolerance = input
            .get("partial_tolerance")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        // Resolve councillor list from preset or inline definition.
        let councillors = parse_councillors(input)?;
        if councillors.is_empty() {
            return Err(BitFunError::tool(
                "No councillors defined. Provide a preset or a councillors list.".to_string(),
            ));
        }

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("Coordinator not initialized".to_string()))?;

        let session_id = context
            .session_id
            .clone()
            .unwrap_or_default();
        let dialog_turn_id = context
            .dialog_turn_id
            .clone()
            .unwrap_or_default();
        let tool_call_id = context
            .tool_call_id
            .clone()
            .unwrap_or_default();

        // Create parent info for subagents.
        let parent_info = SubagentParentInfo {
            tool_call_id,
            session_id,
            dialog_turn_id,
        };

        // Preserve default workspace path from context.
        let workspace_path = context
            .workspace_root()
            .map(|p| p.to_string_lossy().to_string());

        // Phase 1: Launch all councillors in parallel.
        let mut handles = Vec::new();
        for councillor in &councillors {
            let councillor_prompt = councillor_role_prompt(&councillor.role, &prompt);
            let model_id = councillor.model.clone();
            let display_name = councillor.name.clone();
            let ws = workspace_path.clone();
            let pi = parent_info.clone();
            let dp = context.delegation_policy().spawn_child();

            handles.push(tokio::spawn(async move {
                let coord = get_global_coordinator()
                    .ok_or_else(|| "Coordinator lost".to_string())?;
                let result = coord
                    .execute_subagent(
                        SubagentExecutionRequest {
                            task_description: councillor_prompt,
                            context_mode: SubagentContextMode::Fresh,
                            subagent_type: Some("Councillor".to_string()),
                            workspace_path: ws,
                            model_id: Some(model_id),
                            subagent_parent_info: pi,
                            context: std::collections::HashMap::new(),
                            delegation_policy: dp,
                        },
                        None,   // cancellation token
                        Some(300), // per-councillor timeout
                    )
                    .await;
                Ok::<(String, crate::agentic::coordination::SubagentResult), String>((display_name, result.map_err(|e| e.to_string())?))
            }));
        }

        // Wait for all councillors with group_timeout.
        let councillor_results = tokio::time::timeout(
            Duration::from_secs(group_timeout),
            futures::future::join_all(handles),
        )
        .await;

        // Collect results.
        let mut successful: Vec<(String, String)> = Vec::new();
        let mut failed: Vec<(String, String)> = Vec::new();

        match councillor_results {
            Ok(results) => {
                for handle_result in results {
                    match handle_result {
                        Ok(Ok((name, subagent_result))) => {
                            successful.push((name, subagent_result.text));
                        }
                        Ok(Err(e)) => {
                            failed.push(("(unknown)".to_string(), e));
                        }
                        Err(join_err) => {
                            failed.push(("(join_err)".to_string(), join_err.to_string()));
                        }
                    }
                }
            }
            Err(_timeout) => {
                // Group timeout: collect whatever finished and treat rest as failed.
                // Since we used join_all inside timeout, this case means the loop itself
                // timed out, which shouldn't normally happen since all tasks would finish
                // or fail individually. But handle gracefully.
            }
        }

        // Phase 2: Build judge prompt from all councillor results.
        if successful.is_empty() && !partial_tolerance {
            let msg = if failed.is_empty() {
                "All councillors timed out".to_string()
            } else {
                let detail: String = failed
                    .iter()
                    .map(|(n, e)| format!("- {}: {}", n, e))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("All councillors failed:\n{}", detail)
            };
            return Ok(vec![ToolResult::ok(
                json!({ "status": "failed", "reason": msg.clone() }),
                Some(msg),
            )]);
        }

        // Only build judge prompt if we have at least some successful results.
        let mut judge_prompt = String::new();

        if successful.is_empty() {
            // All failed, partial tolerance is true — return what we know.
            let msg = "All councillors failed to produce results.".to_string();
            return Ok(vec![ToolResult::ok(json!({ "status": "failed", "reason": msg.clone() }), Some(msg))]);
        }

        judge_prompt.push_str(&format!(
            "## Original Question\n{}\n\n---\n\n## Expert Opinions\n",
            prompt
        ));

        for (name, text) in &successful {
            judge_prompt.push_str(&format!("\n### {}\n{}\n\n---\n", name, text));
        }

        if !failed.is_empty() && partial_tolerance {
            judge_prompt.push_str("\n## Failed/Timed-out Experts\n");
            for (name, reason) in &failed {
                judge_prompt.push_str(&format!("- **{}**: {}\n", name, reason));
            }
            judge_prompt.push_str("\n---\n");
        }

        // Phase 3: Launch Judge sub-agent.
        let jp = judge_prompt.clone();
        let jws = workspace_path.clone();
        let jpi = parent_info.clone();
        let jdp = context.delegation_policy().spawn_child();

        let judge_result = coordinator
            .execute_subagent(
                SubagentExecutionRequest {
                    task_description: jp,
                    context_mode: SubagentContextMode::Fresh,
                    subagent_type: Some("Judge".to_string()),
                    workspace_path: jws,
                    model_id: Some(judge_model),
                    subagent_parent_info: jpi,
                    context: std::collections::HashMap::new(),
                    delegation_policy: jdp,
                },
                None,
                Some(120), // judge timeout
            )
            .await?;

        let verdict = judge_result.text;

        // Build summary for the calling LLM (concise — full details in Councillor Details).
        let total = councillors.len();
        let succeeded = successful.len();
        let consensus = if failed.is_empty() {
            "unanimous"
        } else if succeeded > 0 {
            "majority"
        } else {
            "split"
        };

        let result_for_assistant = format!(
            "## Synod Council: {}\n\n**Participants**: {} experts ({}/{})\n**Confidence**: {}\n\n{}",
            description,
            total,
            succeeded,
            total,
            consensus,
            verdict
        );

        let data = json!({
            "description": description,
            "status": "completed",
            "total_experts": total,
            "succeeded": succeeded,
            "failed": failed.len(),
            "confidence": consensus,
            "verdict": verdict,
        });

        Ok(vec![ToolResult::ok(data, Some(result_for_assistant))])
    }
}

/// Parse councillor list from input: preset or explicit list.
fn parse_councillors(input: &Value) -> BitFunResult<Vec<CouncillorTask>> {
    // Try explicit councillors list first.
    if let Some(list) = input.get("councillors").and_then(Value::as_array) {
        if !list.is_empty() {
            let mut councillors = Vec::new();
            for item in list {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Expert");
                let model = item
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("fast");
                let role = item
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                councillors.push(CouncillorTask {
                    name: name.to_string(),
                    model: model.to_string(),
                    role: role.to_string(),
                });
            }
            return Ok(councillors);
        }
    }

    // Fall back to preset.
    let preset_name = input
        .get("preset")
        .and_then(Value::as_str)
        .unwrap_or("review");

    for &(name, entries) in PRESETS {
        if name == preset_name {
            return Ok(entries
                .iter()
                .map(|&(n, m, r)| CouncillorTask {
                    name: n.to_string(),
                    model: m.to_string(),
                    role: r.to_string(),
                })
                .collect());
        }
    }

    Err(BitFunError::tool(format!(
        "Unknown preset '{}'. Available presets: review, design, deep, startup",
        preset_name
    )))
}

/// Format the full prompt for a councillor: role guidance + separator + shared prompt.
fn councillor_role_prompt(role: &str, shared_prompt: &str) -> String {
    if role.is_empty() {
        format!(
            "请独立分析以下问题，输出结构化的分析结果。\n\n---\n\n{}",
            shared_prompt
        )
    } else {
        format!(
            "## 你的角色\n{}\n\n请从这个角度出发分析以下问题。\n\n---\n\n{}",
            role, shared_prompt
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::Tool;

    #[test]
    fn synod_tool_basics() {
        let tool = SynodTool::new();
        assert_eq!(tool.name(), "Synod");
        assert!(tool.is_readonly());
        assert!(!tool.is_concurrency_safe(None));
        assert!(tool.manages_own_execution_timeout());
        assert!(!tool.short_description().is_empty());
        let schema = tool.input_schema();
        let props = schema.get("properties").unwrap();
        assert!(props.get("prompt").is_some());
        assert!(props.get("councillors").is_some());
        assert!(props.get("preset").is_some());
        assert!(props.get("judge_model").is_some());
        assert!(props.get("group_timeout_seconds").is_some());
        assert!(props.get("partial_tolerance").is_some());
    }

    #[test]
    fn parse_preset_review_expands_correctly() {
        let input = json!({ "prompt": "test", "preset": "review" });
        let councillors = parse_councillors(&input).unwrap();
        assert_eq!(councillors.len(), 3);
        assert_eq!(councillors[0].name, "架构");
        assert_eq!(councillors[0].model, "primary");
    }

    #[test]
    fn parse_preset_deep_expands_correctly() {
        let input = json!({ "prompt": "test", "preset": "deep" });
        let councillors = parse_councillors(&input).unwrap();
        assert_eq!(councillors.len(), 2);
        assert_eq!(councillors[0].name, "主审");
        assert_eq!(councillors[1].name, "对抗");
    }

    #[test]
    fn parse_custom_councillors_overrides_preset() {
        let input = json!({
            "prompt": "test",
            "preset": "review",
            "councillors": [{ "name": "自定义A", "model": "primary", "role": "关注安全性" }]
        });
        let councillors = parse_councillors(&input).unwrap();
        assert_eq!(councillors.len(), 1);
        assert_eq!(councillors[0].name, "自定义A");
    }

    #[test]
    fn parse_unknown_preset_returns_error() {
        let input = json!({ "prompt": "test", "preset": "nonexistent" });
        assert!(parse_councillors(&input).is_err());
    }

    #[test]
    fn councillor_prompt_with_role_includes_role_text() {
        let result = councillor_role_prompt("关注安全性", "这个方案安全吗？");
        assert!(result.contains("关注安全性"));
        assert!(result.contains("这个方案安全吗？"));
    }

    #[test]
    fn councillor_prompt_without_role_uses_generic_instruction() {
        let result = councillor_role_prompt("", "这个方案安全吗？");
        assert!(!result.contains("从这个角度"));
        assert!(result.contains("独立分析"));
    }
}
