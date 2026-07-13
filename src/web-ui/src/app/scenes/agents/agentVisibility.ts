/** Agent IDs hidden from the Agents overview UI (not listed, not counted). */
export const STATIC_HIDDEN_AGENT_IDS = new Set<string>([
  'Claw',
]);

export const FALLBACK_REVIEW_HIDDEN_AGENT_IDS = new Set<string>([
  'DeepReview',
  'ReviewBusinessLogic',
  'ReviewPerformance',
  'ReviewSecurity',
  'ReviewArchitecture',
  'ReviewFrontend',
  'ReviewJudge',
]);

export const HIDDEN_AGENT_IDS = new Set<string>([
  ...STATIC_HIDDEN_AGENT_IDS,
  ...FALLBACK_REVIEW_HIDDEN_AGENT_IDS,
]);

/** Prefix used by ACP external agents (e.g. acp__codex, acp__Claude_Code). */
export const ACP_CORE_AGENT_PREFIX = 'acp__';

/** Core mode agents shown in the top zone only; excluded from overview zone list and counts. */
const STATIC_CORE_AGENT_IDS = new Set<string>(['agentic', 'Cowork', 'ComputerUse']);

/** Build the effective core-agent id set including enabled ACP external agents. */
export function buildCoreAgentIds(acpEnabledIds: string[]): Set<string> {
  const ids = new Set(STATIC_CORE_AGENT_IDS);
  for (const acpId of acpEnabledIds) {
    ids.add(`${ACP_CORE_AGENT_PREFIX}${acpId}`);
  }
  return ids;
}

/** Legacy static set kept for backward-compat callers that don't inject ACP ids. */
export const CORE_AGENT_IDS = STATIC_CORE_AGENT_IDS;

/** Agents that appear in the bottom overview grid (same pool as filter chip counts). */
export function isAgentInOverviewZone(
  agent: { id: string },
  hiddenAgentIds: ReadonlySet<string> = HIDDEN_AGENT_IDS,
  coreAgentIds: ReadonlySet<string> = CORE_AGENT_IDS,
): boolean {
  return !hiddenAgentIds.has(agent.id) && !coreAgentIds.has(agent.id);
}
