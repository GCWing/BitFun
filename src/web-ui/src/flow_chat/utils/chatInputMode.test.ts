import { describe, expect, it } from 'vitest';

import { resolveWorkspaceChatInputMode } from './chatInputMode';

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
