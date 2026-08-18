/**
 * Built-in agent ids that are displayed with the "Sub-Agent" badge.
 * Other builtin agents keep the "Built-in" badge.
 */
export const BUILTIN_SUB_AGENT_IDS = ['explore'] as const;

export function isBuiltinSubAgent(agentId: string): boolean {
  const id = agentId.toLowerCase().replace(/\s+/g, '_');
  return id === 'explore';
}
