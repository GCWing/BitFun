import { describe, expect, it } from 'vitest';
import { computeAnnouncementMessage } from './AskUserAnnouncer';

type TFunc = (key: string, params?: Record<string, unknown>) => string;

/** Mock t that embeds the key and params so tests can assert on them. */
function mockT(key: string, params?: Record<string, unknown>): string {
  if (!params) return key;
  const parts: string[] = [];
  for (const [k, v] of Object.entries(params)) {
    parts.push(`${k}=${v}`);
  }
  return `${key}[${parts.join(',')}]`;
}

function map(entries: [string, string][]): Map<string, string> {
  return new Map(entries);
}

describe('computeAnnouncementMessage', () => {
  const t = mockT as TFunc;

  it('announces single add with session name', () => {
    const prev = map([]);
    const current = map([['s1', 'Task A']]);
    const msg = computeAnnouncementMessage(prev, current, t);
    expect(msg).toContain('ariaNeedsInputWithName');
    expect(msg).toContain('name=Task A');
  });

  it('announces second consecutive add with the new session name (not skipped)', () => {
    // First add: A starts waiting
    const prev1 = map([]);
    const current1 = map([['s1', 'Task A']]);
    const msg1 = computeAnnouncementMessage(prev1, current1, t);
    expect(msg1).toContain('name=Task A');

    // Second add: B starts waiting while A is still waiting
    const prev2 = current1;
    const current2 = map([['s1', 'Task A'], ['s2', 'Task B']]);
    const msg2 = computeAnnouncementMessage(prev2, current2, t);
    // Must be different from msg1 (not the same string)
    expect(msg2).not.toBe(msg1);
    expect(msg2).toContain('name=Task B');
  });

  it('announces plural count when multiple sessions added simultaneously', () => {
    const prev = map([]);
    const current = map([['s1', 'Task A'], ['s2', 'Task B']]);
    const msg = computeAnnouncementMessage(prev, current, t);
    expect(msg).toContain('ariaNeedsInputPlural');
    expect(msg).toContain('count=2');
  });

  it('announces partial resolution with remaining count', () => {
    const prev = map([['s1', 'Task A'], ['s2', 'Task B']]);
    const current = map([['s2', 'Task B']]);
    const msg = computeAnnouncementMessage(prev, current, t);
    expect(msg).toContain('ariaInputResolvedRemaining');
    expect(msg).toContain('name=Task A');
    expect(msg).toContain('count=1');
  });

  it('announces all resolved when last waiting session is removed', () => {
    const prev = map([['s1', 'Task A']]);
    const current = map([]);
    const msg = computeAnnouncementMessage(prev, current, t);
    expect(msg).toContain('ariaInputResolved');
  });

  it('prioritises added over removed when both happen (swap)', () => {
    const prev = map([['s1', 'Task A']]);
    const current = map([['s2', 'Task B']]);
    const msg = computeAnnouncementMessage(prev, current, t);
    // Added takes priority — should announce the new session, not the resolved one
    expect(msg).toContain('ariaNeedsInputWithName');
    expect(msg).toContain('name=Task B');
  });

  it('returns empty string when nothing changed', () => {
    const prev = map([['s1', 'Task A']]);
    const current = map([['s1', 'Task A']]);
    const msg = computeAnnouncementMessage(prev, current, t);
    expect(msg).toBe('');
  });
});
