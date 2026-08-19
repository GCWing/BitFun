import { describe, expect, it } from 'vitest';
import {
  resolveComposerExecutionLevelSelection,
  resolveChatInputExecutionLevelPolicy,
  resolveSelectedComposerExecutionLevel,
} from './chatInputExecutionLevelPolicy';

describe('chatInputExecutionLevelPolicy', () => {
  it('maps presentation levels directly to Agent types', () => {
    expect(resolveComposerExecutionLevelSelection('ultimate', 'agentic')).toEqual({
      modeId: 'Ultra',
    });
    expect(resolveComposerExecutionLevelSelection('minimal', 'agentic')).toEqual({
      modeId: 'minimal',
    });
    expect(resolveSelectedComposerExecutionLevel({
      currentMode: ' Ultra ',
    })).toBe('ultimate');
  });

  it('maps Balanced to agentic only when leaving a tier Agent', () => {
    expect(resolveComposerExecutionLevelSelection('minimal', 'Ultra')).toEqual({
      modeId: 'minimal',
    });
    expect(resolveComposerExecutionLevelSelection('balanced', 'Plan')).toEqual({
      modeId: 'Plan',
    });
    expect(resolveComposerExecutionLevelSelection('balanced', 'minimal')).toEqual({
      modeId: 'agentic',
    });
  });

  it('lets a root project composer configure its execution level', () => {
    const policy = resolveChatInputExecutionLevelPolicy({
      isAssistantWorkspace: false,
      isAcpTargetSession: false,
      isSubagentInputTarget: false,
    });

    expect(policy).toEqual({ owner: 'composer', userConfigurable: true });
  });

  it('keeps Assistant execution level selection with the runtime default', () => {
    const policy = resolveChatInputExecutionLevelPolicy({
      isAssistantWorkspace: true,
      isAcpTargetSession: false,
      isSubagentInputTarget: false,
    });

    expect(policy).toEqual({
      owner: 'assistant-runtime-default',
      userConfigurable: false,
    });
  });

  it('hides execution-level configuration for a Claw Assistant session when workspace state is stale', () => {
    const policy = resolveChatInputExecutionLevelPolicy({
      isAssistantWorkspace: false,
      sessionMode: ' claw ',
      isAcpTargetSession: false,
      isSubagentInputTarget: false,
    });

    expect(policy).toEqual({
      owner: 'assistant-runtime-default',
      userConfigurable: false,
    });
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
  ] as const)('does not submit a composer execution-level selection for $label', ({ params, owner }) => {
    const policy = resolveChatInputExecutionLevelPolicy(params);

    expect(policy).toEqual({ owner, userConfigurable: false });
  });

  it('lets the external ACP owner win when target facts overlap', () => {
    const policy = resolveChatInputExecutionLevelPolicy({
      isAssistantWorkspace: true,
      isAcpTargetSession: true,
      isSubagentInputTarget: true,
    });

    expect(policy.owner).toBe('acp-host');
    expect(policy.userConfigurable).toBe(false);
  });
});
