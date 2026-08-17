import { describe, expect, it } from 'vitest';
import {
  reconcileSubagentIdentityAssignments,
  type SubagentIdentityAssignments,
  type SubagentIdentitySubject,
} from './allocator';

function subjects(count: number, active = true): SubagentIdentitySubject[] {
  return Array.from({ length: count }, (_, index) => ({
    sessionId: `child-${String(index + 1).padStart(2, '0')}`,
    createdAt: index + 1,
    active,
  }));
}

describe('subagent identity allocator', () => {
  it('keeps the first fifteen avatars and first thirty names unique', () => {
    const assignments = reconcileSubagentIdentityAssignments('root', subjects(30, false));
    const ordered = subjects(30, false).map(subject => assignments[subject.sessionId]);

    expect(new Set(ordered.slice(0, 15).map(identity => identity.avatarId))).toHaveLength(15);
    expect(new Set(ordered.map(identity => identity.nameId))).toHaveLength(30);
  });

  it('is deterministic regardless of snapshot input order', () => {
    const input = subjects(12);
    const forward = reconcileSubagentIdentityAssignments('root', input);
    const reversed = reconcileSubagentIdentityAssignments('root', [...input].reverse());

    expect(reversed).toEqual(forward);
  });

  it('preserves existing identities and reuses released capacity for later active agents', () => {
    const historical = subjects(15, false);
    const initial = reconcileSubagentIdentityAssignments('root', historical);
    const nextSubjects = [
      ...historical,
      { sessionId: 'child-16', createdAt: 16, active: true },
      { sessionId: 'child-17', createdAt: 17, active: true },
    ];
    const next = reconcileSubagentIdentityAssignments('root', nextSubjects, initial);

    expect(next['child-01']).toEqual(initial['child-01']);
    expect(next['child-16'].avatarId).not.toBe(next['child-17'].avatarId);
    expect(next['child-16'].nameId).not.toBe(next['child-17'].nameId);
  });

  it('repairs collisions between live agents while keeping the older identity', () => {
    const previous: SubagentIdentityAssignments = {
      older: {
        rootSessionId: 'root',
        sessionId: 'older',
        avatarId: 'robot-01',
        nameId: 'name-01',
      },
      newer: {
        rootSessionId: 'root',
        sessionId: 'newer',
        avatarId: 'robot-01',
        nameId: 'name-01',
      },
    };
    const next = reconcileSubagentIdentityAssignments('root', [
      { sessionId: 'newer', createdAt: 2, active: true },
      { sessionId: 'older', createdAt: 1, active: true },
    ], previous);

    expect(next.older).toEqual(previous.older);
    expect(next.newer.avatarId).not.toBe(next.older.avatarId);
    expect(next.newer.nameId).not.toBe(next.older.nameId);
  });
});
