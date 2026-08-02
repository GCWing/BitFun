import { afterEach, describe, expect, it } from 'vitest';

import {
  dispatchJobStoreObservesSession,
  resolveSessionDriverId,
  resolveSessionDriverIdForCreation,
  resolveSessionDriverIdWith,
} from './resolve';
import { dispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import type { DispatchObserverJob } from '@/features/dispatch/dispatchJobStore';

const never = () => false;

function observerJob(overrides: Partial<DispatchObserverJob>): DispatchObserverJob {
  return {
    jobId: 'job-1',
    sessionId: 'session-1',
    targetRequest: { kind: 'ssh', connectionId: 'ssh-user@host', workspacePath: '/repo' },
    target: {
      kind: 'ssh',
      connectionId: 'ssh-user@host',
      workspacePath: '/repo',
      displayName: 'host',
    },
    sourceWorkspacePath: '/controller/repo',
    title: 'job',
    agentType: 'agentic',
    approvalPolicy: 'remote',
    cursor: 0,
    state: 'submitting',
    appliedEventIds: [],
    pendingPermissions: [],
    eventLogComplete: true,
    historyTruncated: false,
    omittedEventCount: 0,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    ...overrides,
  } as DispatchObserverJob;
}

describe('resolveSessionDriverIdWith', () => {
  it('resolves local when no dispatch signal is present', () => {
    expect(resolveSessionDriverIdWith('s1', { config: {} }, never)).toBe('local');
    expect(resolveSessionDriverIdWith('s1', undefined, never)).toBe('local');
  });

  it('treats a local dispatch target as local execution', () => {
    expect(
      resolveSessionDriverIdWith('s1', { config: { dispatchTarget: { kind: 'local' } } }, never),
    ).toBe('local');
  });

  it('resolves dispatch from a bound non-local target', () => {
    expect(
      resolveSessionDriverIdWith(
        's1',
        {
          config: {
            dispatchTarget: {
              kind: 'ssh',
              connectionId: 'ssh-user@host',
              workspacePath: '/repo',
              displayName: 'host',
            },
          },
        },
        never,
      ),
    ).toBe('dispatch');
  });

  it('resolves dispatch from a configured job id even without a bound target', () => {
    expect(
      resolveSessionDriverIdWith('s1', { config: { dispatchJobId: 'dispatch-abc' } }, never),
    ).toBe('dispatch');
    expect(
      resolveSessionDriverIdWith('s1', { config: { dispatchJobId: '   ' } }, never),
    ).toBe('local');
  });

  it('resolves dispatch from observer-store membership alone', () => {
    // The startup race: ensureProjection has not bound config yet, but the
    // observer store already knows the session belongs to a target-owned job.
    const observes = (sessionId: string) => sessionId === 's1';
    expect(resolveSessionDriverIdWith('s1', { config: {} }, observes)).toBe('dispatch');
    expect(resolveSessionDriverIdWith('s2', { config: {} }, observes)).toBe('local');
  });
});

describe('resolveSessionDriverId (dispatchJobStore integration)', () => {
  afterEach(() => {
    dispatchJobStore.getState().clear();
  });

  it('sees sessions registered in the observer store', () => {
    expect(dispatchJobStoreObservesSession('session-1')).toBe(false);
    dispatchJobStore.getState().registerJob(observerJob({ sessionId: 'session-1' }));
    expect(dispatchJobStoreObservesSession('session-1')).toBe(true);
    expect(resolveSessionDriverId('session-1', { config: {} })).toBe('dispatch');
    expect(resolveSessionDriverId('other-session', { config: {} })).toBe('local');
  });
});

describe('resolveSessionDriverIdForCreation', () => {
  it('keys off the requested dispatch target', () => {
    expect(resolveSessionDriverIdForCreation({})).toBe('local');
    expect(
      resolveSessionDriverIdForCreation({ dispatchTargetRequest: { kind: 'local' } }),
    ).toBe('local');
    expect(
      resolveSessionDriverIdForCreation({
        dispatchTargetRequest: { kind: 'device', deviceId: 'd1', workspacePath: '/repo' },
      }),
    ).toBe('dispatch');
  });
});
