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
    sourceWorkspacePath: '/source',
    title: 'Dispatch test',
    agentType: 'agentic',
    approvalPolicy: 'reject-and-report',
    workspaceDelivery: { kind: 'existing' },
    cursor: 10,
    state,
    terminalDrained: state === 'succeeded',
    appliedEventIds: [],
    pendingPermissions: [],
    eventLogComplete: true,
    historyTruncated: false,
    omittedEventCount: 0,
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
      sourceWorkspacePath: '/source',
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

  it('keeps the renderer cursor independent from controller-wide observer progress', () => {
    registerJob();
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-1',
      sessionId: 'session-1',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/canonical/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/canonical/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 900,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      cursor: 10,
      target: {
        workspacePath: '/canonical/repo',
      },
    });
  });

  it('reconstructs immutable submission metadata from the durable outbound index', () => {
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-restored',
      sessionId: 'session-restored',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      workspacePath: '/repo',
      promptPreview: 'Prompt preview',
      title: 'Remote review',
      agentType: 'debug',
      approvalPolicy: 'remote',
      model: 'configured-model',
      sourceWorkspacePath: '/controller/repo',
      sourceWorkspaceId: 'workspace-1',
      lastCursor: 900,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-restored']).toMatchObject({
      title: 'Remote review',
      agentType: 'debug',
      approvalPolicy: 'remote',
      model: 'configured-model',
      sourceWorkspacePath: '/controller/repo',
      sourceWorkspaceId: 'workspace-1',
      cursor: 0,
    });
  });

  it('drops a legacy outbound job instead of guessing its source workspace', () => {
    const record = {
      jobId: 'job-restored',
      sessionId: 'session-restored',
      target: {
        kind: 'ssh' as const,
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
        displayName: 'build-host',
      },
      workspacePath: '/target/repo',
      promptPreview: 'Prompt preview',
      lastCursor: 0,
      lastState: 'running' as const,
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    };

    // Simulate a cache polluted by the old current-workspace fallback.
    dispatchJobStore.getState().registerJob({
      jobId: 'job-restored',
      sessionId: 'session-restored',
      targetRequest: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
      },
      target: record.target,
      sourceWorkspacePath: '/wrong/current/workspace',
      title: 'Prompt preview',
      agentType: 'agentic',
      approvalPolicy: 'reject-and-report',
      workspaceDelivery: { kind: 'existing' },
      cursor: 0,
      state: 'running',
      appliedEventIds: [],
      pendingPermissions: [],
      eventLogComplete: true,
      historyTruncated: false,
      omittedEventCount: 0,
      createdAt: 1,
      updatedAt: 1,
    });

    dispatchJobStore.getState().mergeOutboundRecords([record]);
    expect(dispatchJobStore.getState().jobs['job-restored']).toBeUndefined();
    expect(
      dispatchJobStore.getState().transportByJobId['job-restored'],
    ).toBeUndefined();
  });

  it('drops acknowledged renderer cache missing from the controller index', () => {
    registerJob();
    dispatchJobStore.getState().mergeOutboundRecords([]);

    expect(dispatchJobStore.getState().jobs['job-1']).toBeUndefined();
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toBeUndefined();
  });

  it('keeps a pre-ack job while the controller index has no record yet', () => {
    registerJob();
    dispatchJobStore.getState().registerJob({
      ...dispatchJobStore.getState().jobs['job-1'],
      state: 'submitting',
    });
    dispatchJobStore.getState().mergeOutboundRecords([]);

    expect(dispatchJobStore.getState().jobs['job-1']?.state).toBe('submitting');
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
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 10,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-1']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedJobIds).toContain('job-1');
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-1');
  });

  it('uses a session tombstone when deletion happens before the job id is known', () => {
    dispatchJobStore.getState().dismissSession('session-late');
    dispatchJobStore.getState().mergeOutboundRecords([{
      jobId: 'job-late',
      sessionId: 'session-late',
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      sourceWorkspacePath: '/source',
      workspacePath: '/repo',
      promptPreview: 'Dispatch test',
      lastCursor: 0,
      lastState: 'running',
      createdAt: '2026-07-28T00:00:00Z',
      updatedAt: '2026-07-28T00:00:01Z',
    }]);

    expect(dispatchJobStore.getState().jobs['job-late']).toBeUndefined();
    expect(dispatchJobStore.getState().dismissedSessionIds).toContain('session-late');
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
    expect(persistedState?.dismissedSessionIds).toEqual([]);
  });
});
