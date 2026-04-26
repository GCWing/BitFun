/** Agent IDs hidden from the Agents overview UI (not listed, not counted). */
export const HIDDEN_AGENT_IDS = new Set<string>([
  'Claw',
  'DeepReview',
  'ReviewBusinessLogic',
  'ReviewPerformance',
  'ReviewSecurity',
  'ReviewJudge',
]);

/** Core mode agents shown in the top zone only; excluded from overview zone list and counts. */
export const CORE_AGENT_IDS = new Set<string>(['agentic', 'Cowork', 'ComputerUse']);

/** Agents that appear in the bottom overview grid (same pool as filter chip counts). */
export function isAgentInOverviewZone(agent: { id: string }): boolean {
  return !HIDDEN_AGENT_IDS.has(agent.id) && !CORE_AGENT_IDS.has(agent.id);
}
