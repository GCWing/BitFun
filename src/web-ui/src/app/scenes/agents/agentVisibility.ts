/** Agent IDs hidden from the Agents overview UI (not listed, not counted). */
export const STATIC_HIDDEN_AGENT_IDS = new Set<string>();

export const FALLBACK_REVIEW_HIDDEN_AGENT_IDS = new Set<string>([
  'DeepReview',
  'ReviewWorker',
  'ReviewBusinessLogic',
  'ReviewPerformance',
  'ReviewSecurity',
  'ReviewArchitecture',
  'ReviewFrontend',
  'ReviewGeneral',
  'ReviewJudge',
]);

export const HIDDEN_AGENT_IDS = new Set<string>([
  ...STATIC_HIDDEN_AGENT_IDS,
  ...FALLBACK_REVIEW_HIDDEN_AGENT_IDS,
]);

/** Runtime invocation role is independent of source and presentation. */
export function isPrimaryAgent<T extends { agentKind?: string }>(agent: T | null | undefined): agent is T & { agentKind: 'harness' | 'agent' } {
  return agent?.agentKind === 'harness' || agent?.agentKind === 'agent';
}

export function isOrdinaryAgent(agent: { agentKind?: string }): boolean {
  return agent.agentKind === 'agent';
}

/** Agents that appear in the bottom overview grid (same pool as filter chip counts). */
export function isAgentInOverviewZone(
  agent: { id: string; agentKind?: string },
  hiddenAgentIds: ReadonlySet<string> = HIDDEN_AGENT_IDS,
): boolean {
  return !hiddenAgentIds.has(agent.id) && agent.agentKind !== 'harness' && !isOrdinaryAgent(agent);
}

/** External subagents are visible in the overview but managed by their source adapter. */
export function isLocallyManageableSubagent(
  subagent: { source?: string | null; subagentSource?: string | null },
): boolean {
  return (subagent.subagentSource ?? subagent.source) !== 'external';
}
