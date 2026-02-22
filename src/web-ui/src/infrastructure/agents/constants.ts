/**
 * Built-in agent ids that are displayed with "Sub-Agent" badge (e.g. Explore, FileFinder).
 * Other builtin agents keep the "Built-in" badge.
 */
export const BUILTIN_SUB_AGENT_IDS = ['explore', 'file_finder'] as const;

export function isBuiltinSubAgent(agentId: string): boolean {
  const id = agentId.toLowerCase().replace(/\s+/g, '_');
  return id === 'explore' || id === 'file_finder' || id === 'filefinder';
}
