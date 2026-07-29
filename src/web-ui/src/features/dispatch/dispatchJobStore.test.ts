/**
 * @vitest-environment jsdom
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { dispatchJobStore } from './dispatchJobStore';

function registerJob(state: 'running' | 'succeeded' = 'running'): void {
  dispatchJobStore.getState().registerJob({
    jobId: 'job-1',
    sessionId: 'session-1',
    targetRequest: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
    },
    target: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
      displayName: 'build-host',
    },
    title: 'Dispatch test',
    agentType: 'agentic',
    approvalPolicy: 'reject-and-report',
    cursor: 10,
    state,
    terminalDrained: state === 'succeeded',
    appliedEventIds: [],
    createdAt: 1,
    updatedAt: 1,
  });
}

describe('dispatchJobStore', () => {
  beforeEach(() => {
    dispatchJobStore.getState().clear();
  });

  it('keeps cursors monotonic and clears terminal-drained state on progress', () => {
    registerJob();
    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 20,
      terminalDrained: true,
    });
    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 12,
    });

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      cursor: 20,
      state: 'running',
      terminalDrained: true,
    });

    dispatchJobStore.getState().updateProgress('job-1', {
      cursor: 21,
    });
    expect(dispatchJobStore.getState().jobs['job-1'].terminalDrained).toBe(false);
  });

  it('never regresses a terminal state from a stale status or outbound record', () => {
    registerJob('succeeded');
    dispatchJobStore.getState().updateProgress('job-1', {
      state: 'running',
      cursor: 11,
    });
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 9,
      lastState: 'queued',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'succeeded',
      cursor: 11,
    });
  });

  it('persists a dismissal tombstone so reconciliation cannot reopen the projection', () => {
    registerJob();
    dispatchJobStore.getState().dismissJob('job-1');
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 10,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-1');
  });

  it('keeps transport reachability transient and separate from authoritative job state', () => {
    registerJob();
    dispatchJobStore.getState().setTransportState(
      'job-1',
      'unreachable',
      'SSH target is offline',
    );

    expect(dispatchJobStore.getState().jobs['job-1'].state).toBe('running');
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toEqual({
      reachability: 'unreachable',
      lastTransportError: 'SSH target is offline',
    });

    const partialize = dispatchJobStore.persist.getOptions().partialize;
    const persistedState = partialize?.(
      dispatchJobStore.getState(),
    ) as Record<string, unknown> | undefined;
    expect(persistedState?.transportByJobId).toBeUndefined();
  });
});
