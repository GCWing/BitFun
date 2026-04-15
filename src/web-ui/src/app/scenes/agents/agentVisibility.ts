/** Agent IDs hidden from the Agents overview UI (not listed, not counted).
 * Claw is an assistant-workspace persona; Dispatcher is the Agentic OS top-level session
 * type accessible from the nav, not an agent card to configure in the gallery. */
export const HIDDEN_AGENT_IDS = new Set<string>(['Claw', 'Dispatcher']);

/** Core mode agents shown in the top zone only; excluded from overview zone list and counts. */
export const CORE_AGENT_IDS = new Set<string>(['agentic', 'Cowork']);

/** Agents that appear in the bottom “Agent 总览” grid (same pool as filter chip counts). */
export function isAgentInOverviewZone(agent: { id: string }): boolean {
  return !HIDDEN_AGENT_IDS.has(agent.id) && !CORE_AGENT_IDS.has(agent.id);
}
