/** Maps the three product execution levels onto the host's canonical Agents. */
export class HarnessProfilePolicy {
  static agentType(profileId: string): string {
    switch (profileId.trim().toLowerCase()) {
      case 'minimal': return 'minimal';
      case 'ultimate': return 'Ultra';
      default: return 'agentic';
    }
  }
}
