import { describe, expect, it } from 'vitest';

import { acpAgentTypeFromSession, isAcpFlowSession } from './acpSession';

describe('acpSession utilities', () => {
  it('resolves ACP agent type from session config first', () => {
    expect(
      acpAgentTypeFromSession({
        config: { agentType: 'acp:opencode' },
        mode: 'agentic',
      } as any)
    ).toBe('acp:opencode');
  });

  it('falls back to ACP mode for older or partial session state', () => {
    expect(
      acpAgentTypeFromSession({
        config: { agentType: 'agentic' },
        mode: 'acp:codex',
      } as any)
    ).toBe('acp:codex');
  });

  it('does not classify normal sessions as ACP sessions', () => {
    const session = {
      config: { agentType: 'agentic' },
      mode: 'Plan',
    } as any;

    expect(acpAgentTypeFromSession(session)).toBeNull();
    expect(isAcpFlowSession(session)).toBe(false);
  });
});
