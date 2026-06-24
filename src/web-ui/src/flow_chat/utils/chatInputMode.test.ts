import { describe, expect, it } from 'vitest';

import {
  normalizeUserDefaultChatInputModeId,
  resolveAvailableChatInputMode,
  resolveWorkspaceChatInputMode,
} from './chatInputMode';

describe('normalizeUserDefaultChatInputModeId', () => {
  it('normalizes non-empty strings and rejects blank values', () => {
    expect(normalizeUserDefaultChatInputModeId(' PlannerPlus ')).toBe('PlannerPlus');
    expect(normalizeUserDefaultChatInputModeId('   ')).toBeNull();
    expect(normalizeUserDefaultChatInputModeId(null)).toBeNull();
  });
});

describe('resolveWorkspaceChatInputMode', () => {
  it('forces Claw inside assistant workspaces', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: true,
        sessionMode: 'agentic',
      })
    ).toBe('Claw');
  });

  it('keeps non-Claw project modes unchanged', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Plan',
        isAssistantWorkspace: false,
        sessionMode: 'Plan',
      })
    ).toBeNull();
  });

  it('syncs when switching between project sessions with different modes', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Plan',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      })
    ).toBe('agentic');
  });

  it('restores a project session mode after a transient assistant workspace state', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: 'agentic',
      })
    ).toBe('agentic');
  });

  it('restores Cowork when a project Cowork session inherited the Claw UI mode', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: 'Cowork',
      })
    ).toBe('Cowork');
  });

  it('falls back to agentic if a project session has no mode yet', () => {
    expect(
      resolveWorkspaceChatInputMode({
        currentMode: 'Claw',
        isAssistantWorkspace: false,
        sessionMode: undefined,
      })
    ).toBe('agentic');
  });
});

describe('resolveAvailableChatInputMode', () => {
  it('returns the synchronized session mode when it is still available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: 'Plan',
        availableModeIds: ['agentic', 'Plan', 'Team'],
      }),
    ).toBe('Plan');
  });

  it('falls back to agentic when the current mode is no longer available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBe('agentic');
  });

  it('keeps the current mode when only the session snapshot is stale', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'Team',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBeNull();
  });

  it('keeps assistant workspaces pinned to Claw when available', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: true,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['agentic', 'Claw'],
      }),
    ).toBe('Claw');
  });

  it('falls back to the first available mode when agentic is unavailable', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'PlannerPlus',
        isAssistantWorkspace: false,
        sessionMode: 'PlannerPlus',
        availableModeIds: ['Team', 'Plan'],
      }),
    ).toBe('Team');
  });

  it('uses the user default mode when starting from the internal project default', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: false,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'PlannerPlus'],
      }),
    ).toBe('PlannerPlus');
  });

  it('does not let the user default override an existing session mode', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'Team',
        isAssistantWorkspace: false,
        sessionMode: 'Team',
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team', 'PlannerPlus'],
      }),
    ).toBeNull();
  });

  it('ignores unavailable user defaults and falls back to agentic', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'MissingMode',
        isAssistantWorkspace: false,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Team'],
      }),
    ).toBe('agentic');
  });

  it('keeps assistant workspaces pinned to Claw even with a user default', () => {
    expect(
      resolveAvailableChatInputMode({
        currentMode: 'agentic',
        isAssistantWorkspace: true,
        sessionMode: undefined,
        userDefaultModeId: 'PlannerPlus',
        availableModeIds: ['agentic', 'Claw', 'PlannerPlus'],
      }),
    ).toBe('Claw');
  });
});
