// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  readFileContent: vi.fn(),
  writeFileContent: vi.fn(),
  onDialogTurnCompleted: vi.fn(),
  onDialogTurnFailed: vi.fn(),
  onDialogTurnCancelled: vi.fn(),
  onDialogTurnInterrupted: vi.fn(),
  dialogTurnCompleted: undefined as ((event: { sessionId: string; turnId: string }) => void) | undefined,
  dialogTurnFailed: undefined as ((event: { sessionId: string; turnId: string }) => void) | undefined,
  dialogTurnCancelled: undefined as ((event: { sessionId: string; turnId: string }) => void) | undefined,
  dialogTurnInterrupted: undefined as ((event: { sessionId: string; turnId: string }) => void) | undefined,
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    onDialogTurnCompleted: mocks.onDialogTurnCompleted.mockImplementation((callback) => {
      mocks.dialogTurnCompleted = callback;
      return vi.fn();
    }),
    onDialogTurnFailed: mocks.onDialogTurnFailed.mockImplementation((callback) => {
      mocks.dialogTurnFailed = callback;
      return vi.fn();
    }),
    onDialogTurnCancelled: mocks.onDialogTurnCancelled.mockImplementation((callback) => {
      mocks.dialogTurnCancelled = callback;
      return vi.fn();
    }),
    onDialogTurnInterrupted: mocks.onDialogTurnInterrupted.mockImplementation((callback) => {
      mocks.dialogTurnInterrupted = callback;
      return vi.fn();
    }),
  },
}));

vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({
  workspaceAPI: {
    readFileContent: mocks.readFileContent,
    writeFileContent: mocks.writeFileContent,
  },
}));

import {
  planBuildStateService,
  type PlanFileRef,
} from './PlanBuildStateService';

const PLAN_CONTENT = `---
name: Example Plan
overview: Verify session-scoped build tracking.
todos:
  - id: shared-todo
    content: Update the implementation
    status: pending
---

Plan body.`;

describe('PlanBuildStateService', () => {
  const trackedTargets: PlanFileRef[] = [];

  function target(name: string): PlanFileRef {
    const value = {
      planFilePath: `/workspace/.openbitfun/plans/${name}.plan.md`,
      workspacePath: '/workspace',
    };
    trackedTargets.push(value);
    return value;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.readFileContent.mockResolvedValue(PLAN_CONTENT);
    mocks.writeFileContent.mockResolvedValue(undefined);
  });

  afterEach(() => {
    for (const value of trackedTargets.splice(0)) {
      planBuildStateService.cancelBuild(value);
    }
  });

  it('cancels only builds owned by the cancelled session', () => {
    const first = target('first');
    const second = target('second');
    planBuildStateService.startBuild({
      ...first,
      sessionId: 'session-a',
      todoIds: ['shared-todo'],
    });
    planBuildStateService.startBuild({
      ...second,
      sessionId: 'session-b',
      todoIds: ['shared-todo'],
    });

    window.dispatchEvent(new CustomEvent('openbitfun:dialog-cancelled', {
      detail: { sessionId: 'session-a' },
    }));

    expect(planBuildStateService.isBuildActive(first)).toBe(false);
    expect(planBuildStateService.isBuildActive(second)).toBe(true);
  });

  it('applies TodoWrite updates only to builds owned by that session', async () => {
    const first = target('todo-first');
    const second = target('todo-second');
    const firstTurnId = planBuildStateService.startBuild({
      ...first,
      sessionId: 'session-a',
      todoIds: ['shared-todo'],
    });
    planBuildStateService.startBuild({
      ...second,
      sessionId: 'session-b',
      todoIds: ['shared-todo'],
    });

    window.dispatchEvent(new CustomEvent('openbitfun:todowrite-update', {
      detail: {
        sessionId: 'session-a',
        turnId: firstTurnId,
        todos: [{
          id: 'shared-todo',
          content: 'Update the implementation',
          status: 'completed',
        }],
        merge: false,
      },
    }));

    await vi.waitFor(() => {
      expect(mocks.writeFileContent).toHaveBeenCalledOnce();
    });
    expect(mocks.readFileContent).toHaveBeenCalledWith(
      first.planFilePath,
      undefined,
      undefined,
    );
    expect(mocks.writeFileContent).toHaveBeenCalledWith(
      first.workspacePath,
      first.planFilePath,
      expect.stringContaining('status: completed'),
      undefined,
    );
    expect(planBuildStateService.isBuildActive(first)).toBe(false);
    expect(planBuildStateService.isBuildActive(second)).toBe(true);
  });

  it('does not register a build without valid todo IDs', () => {
    const plan = target('empty-todos');

    const turnId = planBuildStateService.startBuild({
      ...plan,
      sessionId: 'session-a',
      todoIds: ['', '   '],
    });

    expect(turnId).toBeNull();
    expect(planBuildStateService.isBuildActive(plan)).toBe(false);
  });

  it.each([
    ['completed', () => mocks.dialogTurnCompleted],
    ['failed', () => mocks.dialogTurnFailed],
    ['cancelled', () => mocks.dialogTurnCancelled],
    ['interrupted', () => mocks.dialogTurnInterrupted],
  ])('stops only the owning build when its turn is %s', (_status, getListener) => {
    const first = target(`settled-${_status}`);
    const second = target(`other-${_status}`);
    const firstTurnId = planBuildStateService.startBuild({
      ...first,
      sessionId: 'session-a',
      todoIds: ['shared-todo'],
    });
    planBuildStateService.startBuild({
      ...second,
      sessionId: 'session-a',
      todoIds: ['shared-todo'],
    });

    expect(firstTurnId).not.toBeNull();
    getListener()?.({ sessionId: 'session-a', turnId: firstTurnId! });

    expect(planBuildStateService.isBuildActive(first)).toBe(false);
    expect(planBuildStateService.isBuildActive(second)).toBe(true);
  });
});
