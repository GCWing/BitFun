import { describe, expect, it } from 'vitest';
import { resolveChatInputTargetSessionId } from './chatInputTarget';

describe('resolveChatInputTargetSessionId', () => {
  it('keeps the main session as the stop target when main is selected', () => {
    expect(resolveChatInputTargetSessionId({
      currentSessionId: 'main-session',
      inputTarget: 'main',
      activeBtwSessionId: 'btw-session',
    })).toBe('main-session');
  });

  it('uses the side-question session when it is selected', () => {
    expect(resolveChatInputTargetSessionId({
      currentSessionId: 'main-session',
      inputTarget: 'btw',
      activeBtwSessionId: 'btw-session',
    })).toBe('btw-session');
  });

  it('falls back to the main session when the side-question tab is gone', () => {
    expect(resolveChatInputTargetSessionId({
      currentSessionId: 'main-session',
      inputTarget: 'btw',
    })).toBe('main-session');
  });
});
