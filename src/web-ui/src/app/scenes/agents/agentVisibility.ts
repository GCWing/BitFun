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

/** Core mode agents shown in the top zone only; excluded from overview zone list and counts. */
export const CORE_AGENT_IDS = new Set<string>(['agentic', 'Cowork', 'ComputerUse']);

/** Agents that appear in the bottom overview grid (same pool as filter chip counts). */
export function isAgentInOverviewZone(
  agent: { id: string },
  hiddenAgentIds: ReadonlySet<string> = HIDDEN_AGENT_IDS,
): boolean {
  return !hiddenAgentIds.has(agent.id) && !CORE_AGENT_IDS.has(agent.id);
}
