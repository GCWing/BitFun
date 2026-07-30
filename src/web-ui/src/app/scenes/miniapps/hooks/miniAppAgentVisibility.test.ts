import { describe, expect, it } from 'vitest';

import { shouldOpenMiniAppAgentRunInMainScene } from './miniAppAgentVisibility';

describe('shouldOpenMiniAppAgentRunInMainScene', () => {
  it('keeps compatibility-profile sessions on their existing surface', () => {
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        false,
        undefined,
        'app#1',
        'session-1',
        false,
      ),
    ).toBe(false);
  });

  it('opens a strict session in the main scene when no runner owns the composer', () => {
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        undefined,
        'app#1',
        'session-1',
        true,
      ),
    ).toBe(true);
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        { token: 'app#2', sessionId: 'session-1' },
        'app#1',
        'session-1',
        true,
      ),
    ).toBe(true);
  });

  it('keeps a strict run in the bubble only after that session is bound', () => {
    const claim = { token: 'app#1', sessionId: 'session-1' };
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        claim,
        'app#1',
        'session-1',
        true,
      ),
    ).toBe(false);
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        claim,
        'app#1',
        'session-2',
        true,
      ),
    ).toBe(true);
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        { token: 'app#1' },
        'app#1',
        'session-1',
        true,
      ),
    ).toBe(true);
  });

  it('opens a background MiniApp run in the main scene', () => {
    expect(
      shouldOpenMiniAppAgentRunInMainScene(
        true,
        { token: 'app#1', sessionId: 'session-1' },
        'app#1',
        'session-1',
        false,
      ),
    ).toBe(true);
  });
});
