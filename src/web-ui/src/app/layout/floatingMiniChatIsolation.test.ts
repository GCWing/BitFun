import { describe, expect, it } from 'vitest';

import {
  canRenderFloatingMiniChatSession,
  isFloatingMiniChatIsolated,
} from './floatingMiniChatIsolation';

describe('floating MiniApp chat isolation', () => {
  it('does not reserve the ordinary floating chat outside a MiniApp scene', () => {
    expect(isFloatingMiniChatIsolated({
      activeMiniAppId: null,
      permissions: undefined,
      hasComposerClaim: false,
    })).toBe(false);
  });

  it('reserves an Agentic composer surface before its iframe claim arrives', () => {
    expect(isFloatingMiniChatIsolated({
      activeMiniAppId: 'market-lens',
      permissions: {
        agent: { enabled: true },
      },
      hasComposerClaim: false,
    })).toBe(true);
  });

  it('fails closed while active MiniApp metadata is still loading', () => {
    expect(isFloatingMiniChatIsolated({
      activeMiniAppId: 'market-lens',
      permissions: undefined,
      hasComposerClaim: false,
    })).toBe(true);
  });

  it('keeps a claimed legacy MiniApp isolated regardless of manifest flags', () => {
    expect(isFloatingMiniChatIsolated({
      activeMiniAppId: 'legacy-agent-app',
      permissions: {},
      hasComposerClaim: true,
    })).toBe(true);
  });

  it('leaves the normal bubble available to a known non-Agentic MiniApp', () => {
    expect(isFloatingMiniChatIsolated({
      activeMiniAppId: 'regex-playground',
      permissions: {},
      hasComposerClaim: false,
    })).toBe(false);
  });

  it('never mounts a normal or unrelated session into an isolated surface', () => {
    expect(canRenderFloatingMiniChatSession({
      isolated: true,
      claimedSessionId: 'miniapp-session',
      activeSessionId: 'normal-session',
    })).toBe(false);
    expect(canRenderFloatingMiniChatSession({
      isolated: true,
      claimedSessionId: undefined,
      activeSessionId: 'normal-session',
    })).toBe(false);
  });

  it('mounts an isolated surface only for its exact claimed session', () => {
    expect(canRenderFloatingMiniChatSession({
      isolated: true,
      claimedSessionId: 'miniapp-session',
      activeSessionId: 'miniapp-session',
    })).toBe(true);
  });

  it('does not constrain the ordinary floating chat session', () => {
    expect(canRenderFloatingMiniChatSession({
      isolated: false,
      claimedSessionId: undefined,
      activeSessionId: 'normal-session',
    })).toBe(true);
  });
});
