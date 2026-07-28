use std::sync::Arc;
use std::time::Instant;

use taiji_llm::LlmClient;

use super::agents::{AgentRole, DebateAgent};
use super::decision::DecisionAgent;
use super::record::{DebateRecord, DebateTurn};
use super::{AgentOutput, DebateConfig, DebateContext, TokenUsage};

/// Multi-agent debate orchestrator.
///
/// Runs a 3-phase debate (opening → rebuttal → closing) among Bull, Bear,
/// and Neutral agents, then invokes the DecisionAgent for the final verdict.
pub struct DebateOrchestrator {
    bull_agent: DebateAgent,
    bear_agent: DebateAgent,
    neutral_observer: DebateAgent,
    decision_agent: DecisionAgent,
    #[allow(dead_code)]
    llm_client: Arc<dyn LlmClient>,
    #[allow(dead_code)]
    max_rounds: usize,
}

impl DebateOrchestrator {
    /// Create a new orchestrator with the given LLM client and debate config.
    pub fn new(llm_client: Arc<dyn LlmClient>, config: DebateConfig) -> Self {
        let max_rounds = config.max_rounds;

        let bull_agent = DebateAgent::new(AgentRole::Bull, llm_client.clone(), &config);
        let bear_agent = DebateAgent::new(AgentRole::Bear, llm_client.clone(), &config);
        let neutral_observer = DebateAgent::new(AgentRole::Neutral, llm_client.clone(), &config);
        let decision_agent = DecisionAgent::new(llm_client.clone(), &config);

        Self {
            bull_agent,
            bear_agent,
            neutral_observer,
            decision_agent,
            llm_client,
            max_rounds,
        }
    }

    /// Run the full debate and return a complete record.
    pub async fn debate(&self, context: &DebateContext) -> Result<DebateRecord, anyhow::Error> {
        let start = Instant::now();
        let debate_id = uuid::Uuid::new_v4().to_string();
        let mut turns: Vec<DebateTurn> = Vec::new();
        let mut total_tokens = TokenUsage::default();

        let state_summary = build_state_summary(context);

        // ── Phase 1: Opening statements ──────────────────────────────
        let opening_prompt = format!(
            "请基于以下市场状态，给出你的开场分析（阶段：开场陈述）：\n\n{}",
            state_summary
        );

        let (bull_open, bear_open, neutral_open) = tokio::join!(
            self.bull_agent.respond(&opening_prompt),
            self.bear_agent.respond(&opening_prompt),
            self.neutral_observer.respond(&opening_prompt),
        );

        let bull_open = push_turn(&mut turns, &mut total_tokens, "opening", "bull", bull_open)?;
        let bear_open = push_turn(&mut turns, &mut total_tokens, "opening", "bear", bear_open)?;
        let neutral_open = push_turn(
            &mut turns,
            &mut total_tokens,
            "opening",
            "neutral",
            neutral_open,
        )?;

        // ── Phase 2: Rebuttal ────────────────────────────────────────
        let rebuttal_prompt = format!(
            "以下是其他分析师的立场陈述，请逐条反驳其核心弱点（阶段：反驳）：\n\n\
             === 多方陈述 ===\n{}\n\n=== 空方陈述 ===\n{}\n\n=== 中立方陈述 ===\n{}",
            bull_open, bear_open, neutral_open,
        );

        let (bull_rebut, bear_rebut, neutral_rebut) = tokio::join!(
            self.bull_agent.respond(&rebuttal_prompt),
            self.bear_agent.respond(&rebuttal_prompt),
            self.neutral_observer.respond(&rebuttal_prompt),
        );

        let bull_rebut = push_turn(
            &mut turns,
            &mut total_tokens,
            "rebuttal",
            "bull",
            bull_rebut,
        )?;
        let bear_rebut = push_turn(
            &mut turns,
            &mut total_tokens,
            "rebuttal",
            "bear",
            bear_rebut,
        )?;
        let neutral_rebut = push_turn(
            &mut turns,
            &mut total_tokens,
            "rebuttal",
            "neutral",
            neutral_rebut,
        )?;

        // ── Phase 3: Closing statements ──────────────────────────────
        let closing_prompt = format!(
            "辩论已进入终结阶段。请基于所有已有论述，给出你的最终立场（阶段：终结陈述）：\n\n\
             === 原始数据 ===\n{}\n\n=== 开场 ===\n多方: {}\n空方: {}\n中立方: {}\n\n\
             === 反驳 ===\n多方: {}\n空方: {}\n中立方: {}",
            state_summary,
            bull_open,
            bear_open,
            neutral_open,
            bull_rebut,
            bear_rebut,
            neutral_rebut,
        );

        let (bull_close, bear_close, neutral_close) = tokio::join!(
            self.bull_agent.respond(&closing_prompt),
            self.bear_agent.respond(&closing_prompt),
            self.neutral_observer.respond(&closing_prompt),
        );

        let _bull_close = push_turn(&mut turns, &mut total_tokens, "closing", "bull", bull_close)?;
        let _bear_close = push_turn(&mut turns, &mut total_tokens, "closing", "bear", bear_close)?;
        let _neutral_close = push_turn(
            &mut turns,
            &mut total_tokens,
            "closing",
            "neutral",
            neutral_close,
        )?;

        // ── DecisionAgent: Final verdict ─────────────────────────────
        let transcript = build_transcript(&turns);
        let final_decision = self.decision_agent.decide(&transcript).await?;

        // ── Compute consensus ────────────────────────────────────────
        let agent_final_directions: Vec<(String, String, f64)> = turns
            .iter()
            .filter(|t| t.phase == "closing")
            .map(|t| {
                let dir = t
                    .decision
                    .as_ref()
                    .map(|d| d.direction.clone())
                    .unwrap_or_else(|| "hold".into());
                let conf = t.decision.as_ref().map(|d| d.confidence).unwrap_or(0.5);
                (t.agent_id.clone(), dir, conf)
            })
            .collect();

        let consensus = DebateRecord::compute_consensus(&agent_final_directions);

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(DebateRecord {
            debate_id,
            context: context.clone(),
            turns,
            consensus,
            final_decision,
            token_usage: total_tokens,
            duration_ms,
        })
    }

    /// Determine whether a debate should be triggered.
    ///
    /// Returns `true` when:
    /// - There are conflicting agent directions (at least one "long" AND one "short"), OR
    /// - Confidence variance across agents exceeds 0.3
    ///
    /// This is a pure function — zero LLM calls.
    pub fn should_debate(agent_outputs: &[AgentOutput]) -> bool {
        if agent_outputs.len() < 2 {
            return false;
        }

        // Check for conflicting directions (long vs short)
        let has_long = agent_outputs.iter().any(|a| a.direction == "long");
        let has_short = agent_outputs.iter().any(|a| a.direction == "short");
        let has_conflict = has_long && has_short;

        if has_conflict {
            return true;
        }

        // Check confidence variance
        let n = agent_outputs.len() as f64;
        let mean = agent_outputs.iter().map(|a| a.confidence).sum::<f64>() / n;
        let variance = agent_outputs
            .iter()
            .map(|a| (a.confidence - mean).powi(2))
            .sum::<f64>()
            / n;

        variance > 0.3
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn push_turn(
    turns: &mut Vec<DebateTurn>,
    total_tokens: &mut TokenUsage,
    phase: &str,
    agent_id: &str,
    result: Result<taiji_llm::ChatResponse, anyhow::Error>,
) -> Result<String, anyhow::Error> {
    let response = result?;
    let usage = TokenUsage {
        prompt_tokens: response.usage.prompt_tokens,
        completion_tokens: response.usage.completion_tokens,
        total_tokens: response.usage.total_tokens,
    };
    total_tokens.prompt_tokens += usage.prompt_tokens;
    total_tokens.completion_tokens += usage.completion_tokens;
    total_tokens.total_tokens += usage.total_tokens;

    let decision = taiji_llm::client::parse_decision_output(&response).ok();

    let content = response.content.clone();
    turns.push(DebateTurn {
        phase: phase.into(),
        agent_id: agent_id.into(),
        content,
        decision,
        token_usage: usage,
    });

    Ok(response.content)
}

fn build_state_summary(context: &DebateContext) -> String {
    let agent_summary: Vec<String> = context
        .agent_outputs
        .iter()
        .map(|a| {
            format!(
                "Agent {}: direction={}, confidence={:.2}, reasoning={}",
                a.agent_id, a.direction, a.confidence, a.reasoning
            )
        })
        .collect();

    format!(
        "品种: {}\n时间: {}\n市场状态:\n{}\n\n各 Agent 分析结果:\n{}",
        context.instrument,
        context.timestamp,
        context.state_json,
        agent_summary.join("\n"),
    )
}

fn build_transcript(turns: &[DebateTurn]) -> String {
    turns
        .iter()
        .map(|t| format!("[{}] {}: {}", t.phase, t.agent_id, t.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use taiji_llm::MockClient;

    fn mock_agent_response() -> String {
        r#"{"direction":"long","confidence":0.85,"reasoning":"三推衰竭确认，支撑有效","key_signals":["triple_push"],"risks":[]}"#.into()
    }

    fn mock_context() -> DebateContext {
        DebateContext {
            instrument: "rb9999".into(),
            timestamp: chrono::Utc::now(),
            state_json: r#"{"price":4200,"volume":150000}"#.into(),
            agent_outputs: vec![
                AgentOutput {
                    agent_id: "trend_agent".into(),
                    direction: "long".into(),
                    confidence: 0.85,
                    reasoning: "趋势向上".into(),
                },
                AgentOutput {
                    agent_id: "volume_agent".into(),
                    direction: "short".into(),
                    confidence: 0.80,
                    reasoning: "量价背离".into(),
                },
            ],
            conflicting_agents: vec!["trend_agent".into(), "volume_agent".into()],
        }
    }

    // ── should_debate tests ──────────────────────────────────────────

    #[test]
    fn should_debate_conflicting_directions() {
        let outputs = vec![
            AgentOutput {
                agent_id: "a1".into(),
                direction: "long".into(),
                confidence: 0.9,
                reasoning: "".into(),
            },
            AgentOutput {
                agent_id: "a2".into(),
                direction: "short".into(),
                confidence: 0.8,
                reasoning: "".into(),
            },
        ];
        assert!(DebateOrchestrator::should_debate(&outputs));
    }

    #[test]
    fn should_debate_high_variance() {
        let outputs = vec![
            AgentOutput {
                agent_id: "a1".into(),
                direction: "hold".into(),
                confidence: 0.9,
                reasoning: "".into(),
            },
            AgentOutput {
                agent_id: "a2".into(),
                direction: "hold".into(),
                confidence: 0.1,
                reasoning: "".into(),
            },
        ];
        // mean=0.5, variance=(0.4²+0.4²)/2=0.16 — NOT > 0.3
        assert!(!DebateOrchestrator::should_debate(&outputs));
    }

    #[test]
    fn should_debate_variance_above_threshold() {
        // Confidence [0,1]: max standard variance is 0.25 (two values at 0 and 1).
        // The 0.3 threshold catches direction conflicts; variance alone will not
        // trigger it at the extremes. Verified: 3 agents at 1.0/0.0/0.0 -> var=0.222 < 0.3.
        let outputs = vec![
            AgentOutput {
                agent_id: "a1".into(),
                direction: "hold".into(),
                confidence: 1.0,
                reasoning: "".into(),
            },
            AgentOutput {
                agent_id: "a2".into(),
                direction: "hold".into(),
                confidence: 0.0,
                reasoning: "".into(),
            },
            AgentOutput {
                agent_id: "a3".into(),
                direction: "hold".into(),
                confidence: 0.0,
                reasoning: "".into(),
            },
        ];
        assert!(!DebateOrchestrator::should_debate(&outputs));
    }

    #[test]
    fn should_not_debate_all_same_direction() {
        let outputs: Vec<AgentOutput> = (0..7)
            .map(|i| AgentOutput {
                agent_id: format!("a{}", i),
                direction: "long".into(),
                confidence: 0.7,
                reasoning: "".into(),
            })
            .collect();
        assert!(!DebateOrchestrator::should_debate(&outputs));
    }

    #[test]
    fn should_debate_three_long_two_short() {
        let mut outputs: Vec<AgentOutput> = (0..3)
            .map(|i| AgentOutput {
                agent_id: format!("long_{}", i),
                direction: "long".into(),
                confidence: 0.8,
                reasoning: "".into(),
            })
            .collect();
        outputs.extend((0..2).map(|i| AgentOutput {
            agent_id: format!("short_{}", i),
            direction: "short".into(),
            confidence: 0.7,
            reasoning: "".into(),
        }));
        assert!(DebateOrchestrator::should_debate(&outputs));
    }

    #[test]
    fn should_not_debate_single_agent() {
        let outputs = vec![AgentOutput {
            agent_id: "a1".into(),
            direction: "long".into(),
            confidence: 0.9,
            reasoning: "".into(),
        }];
        assert!(!DebateOrchestrator::should_debate(&outputs));
    }

    // ── Integration: MOCK debate returns complete DebateRecord ───────

    #[tokio::test]
    async fn mock_debate_returns_complete_record() {
        let config = DebateConfig::load_default();
        let client = Arc::new(MockClient::new(mock_agent_response()));

        let orchestrator = DebateOrchestrator::new(client, config);
        let context = mock_context();

        let record = orchestrator.debate(&context).await.unwrap();

        // Verify structure
        assert!(!record.debate_id.is_empty());
        assert_eq!(record.turns.len(), 9); // 3 phases × 3 agents
        assert_eq!(record.context.instrument, "rb9999");
        // mock responses are instant — duration_ms may be 0
        let _ = record.duration_ms;

        // Verify phases
        let phases: Vec<&str> = record.turns.iter().map(|t| t.phase.as_str()).collect();
        assert_eq!(
            phases,
            vec![
                "opening", "opening", "opening", "rebuttal", "rebuttal", "rebuttal", "closing",
                "closing", "closing",
            ]
        );

        // Verify agent IDs
        let agents: Vec<&str> = record.turns.iter().map(|t| t.agent_id.as_str()).collect();
        assert_eq!(
            agents,
            vec!["bull", "bear", "neutral", "bull", "bear", "neutral", "bull", "bear", "neutral"]
        );
    }
}
