export function formatAgentIdForDisplay(agentId: string): string {
  const label = agentId.replace(/[-_]+/g, ' ').trim();
  if (!label) {
    return agentId;
  }
  return label.charAt(0).toUpperCase() + label.slice(1);
}
