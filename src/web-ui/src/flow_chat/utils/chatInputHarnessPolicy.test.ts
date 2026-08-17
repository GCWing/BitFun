import { describe, expect, it } from 'vitest';
import {
  resolveChatInputHarnessProfilePolicy,
  resolvePendingHarnessProfileForCreation,
} from './chatInputHarnessPolicy';

describe('chatInputHarnessPolicy', () => {
  it('lets a root project composer configure the Harness Profile', () => {
    const policy = resolveChatInputHarnessProfilePolicy({
      isAssistantWorkspace: false,
      isAcpTargetSession: false,
      isSubagentInputTarget: false,
    });

    expect(policy).toEqual({ owner: 'composer', userConfigurable: true });
    expect(resolvePendingHarnessProfileForCreation(policy, 'minimal')).toBe('minimal');
  });

  it('keeps Assistant Harness selection with the runtime default', () => {
    const policy = resolveChatInputHarnessProfilePolicy({
      isAssistantWorkspace: true,
      isAcpTargetSession: false,
      isSubagentInputTarget: false,
    });

    expect(policy).toEqual({
      owner: 'assistant-runtime-default',
      userConfigurable: false,
    });
    expect(resolvePendingHarnessProfileForCreation(policy, 'minimal')).toBeNull();
  });

  it.each([
    {
      label: 'ACP target',
      params: {
        isAssistantWorkspace: false,
        isAcpTargetSession: true,
        isSubagentInputTarget: false,
      },
      owner: 'acp-host',
    },
    {
      label: 'subagent target',
      params: {
        isAssistantWorkspace: false,
        isAcpTargetSession: false,
        isSubagentInputTarget: true,
      },
      owner: 'parent-session',
    },
  ] as const)('does not submit a composer Harness selection for $label', ({ params, owner }) => {
    const policy = resolveChatInputHarnessProfilePolicy(params);

    expect(policy).toEqual({ owner, userConfigurable: false });
    expect(resolvePendingHarnessProfileForCreation(policy, 'balanced')).toBeNull();
  });

  it('lets the external ACP owner win when target facts overlap', () => {
    const policy = resolveChatInputHarnessProfilePolicy({
      isAssistantWorkspace: true,
      isAcpTargetSession: true,
      isSubagentInputTarget: true,
    });

    expect(policy.owner).toBe('acp-host');
    expect(policy.userConfigurable).toBe(false);
  });
});
