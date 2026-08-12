// @vitest-environment jsdom

/**
 * PlanBuildStateService contract tests (PLAN-2 / L6-P2-1).
 *
 * Pins the plan build-state service contract that CreatePlanDisplay and
 * PlanViewer both consume:
 * 1. startBuild marks a plan building and notifies subscribers
 * 2. subscribe returns an unsubscribe function
 * 3. TodoWrite-update events update the plan file and re-notify with merged
 *    todos (frontmatter re-serialized, content preserved)
 * 4. all-completed todos emit build-completed and end the active build
 * 5. cancelBuild emits build-cancelled and clears the build
 * 6. path normalization: backslash paths are treated as the same plan
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  readFileContent: vi.fn(),
  writeFileContent: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/WorkspaceAPI', () => ({
  workspaceAPI: {
    readFileContent: mocks.readFileContent,
    writeFileContent: mocks.writeFileContent,
  },
}));

// Re-import after mock registration so the singleton picks up the mocked API.
import { planBuildStateService } from './PlanBuildStateService';

const PLAN_FILE = 'D:/workspace/plan.md';
const PLAN_FILE_BACKSLASH = 'D:\\workspace\\plan.md';
const FRONTMATTER = `---
todos:
  - id: t1
    content: first
    status: pending
  - id: t2
    content: second
    status: pending
---
# Plan body

Keep me.`;

describe('PlanBuildStateService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    // Reset singleton state between tests.
    planBuildStateService.cancelBuild(PLAN_FILE);
  });

  it('startBuild marks the plan building and notifies subscribers', () => {
    const events: string[] = [];
    planBuildStateService.subscribe(PLAN_FILE, (e) => events.push(e.type));

    expect(planBuildStateService.isBuildActive(PLAN_FILE)).toBe(false);
    planBuildStateService.startBuild(PLAN_FILE, ['t1', 't2']);
    expect(planBuildStateService.isBuildActive(PLAN_FILE)).toBe(true);
    expect(events).toEqual(['build-started']);
  });

  it('subscribe returns an unsubscribe function that stops notifications', () => {
    const events: string[] = [];
    const unsubscribe = planBuildStateService.subscribe(PLAN_FILE, (e) =>
      events.push(e.type),
    );

    planBuildStateService.startBuild(PLAN_FILE, ['t1']);
    unsubscribe();
    planBuildStateService.cancelBuild(PLAN_FILE);
    expect(events).toEqual(['build-started']);
  });

  it('normalizes backslash paths to the same plan key', () => {
    const events: string[] = [];
    planBuildStateService.subscribe(PLAN_FILE_BACKSLASH, (e) =>
      events.push(e.type),
    );

    planBuildStateService.startBuild(PLAN_FILE, ['t1']);
    expect(planBuildStateService.isBuildActive(PLAN_FILE_BACKSLASH)).toBe(true);
    expect(events).toEqual(['build-started']);
  });

  it('todowrite-update merges incoming status into todos and writes the file', async () => {
    mocks.readFileContent.mockResolvedValueOnce(FRONTMATTER);
    mocks.writeFileContent.mockResolvedValueOnce(undefined);

    planBuildStateService.startBuild(PLAN_FILE, ['t1', 't2']);
    const events: Array<{ type: string; isBuilding: boolean; updatedTodos?: Array<{ id: string; status: string }> }> = [];
    planBuildStateService.subscribe(PLAN_FILE, (e) => events.push(e));

    window.dispatchEvent(
      new CustomEvent('bitfun:todowrite-update', {
        detail: {
          sessionId: 's1',
          turnId: 't1',
          todos: [{ id: 't1', content: 'first', status: 'completed' }],
          merge: true,
        },
      }),
    );

    // Let the async handler run.
    await vi.waitFor(() => {
      expect(mocks.writeFileContent).toHaveBeenCalledTimes(1);
    });

    expect(mocks.readFileContent).toHaveBeenCalledWith(PLAN_FILE);
    const [, , writtenContent] = mocks.writeFileContent.mock.calls[0];
    expect(writtenContent).toContain('id: t1');
    expect(writtenContent).toContain('status: completed');
    // Body content preserved
    expect(writtenContent).toContain('# Plan body');
    expect(writtenContent).toContain('Keep me.');

    const last = events[events.length - 1];
    expect(last.type).toBe('todos-updated');
    expect(last.isBuilding).toBe(true);
    expect(last.updatedTodos?.find((t) => t.id === 't1')?.status).toBe('completed');
  });

  it('all-completed todos emit build-completed and clear the active build', async () => {
    mocks.readFileContent.mockResolvedValueOnce(FRONTMATTER);
    mocks.writeFileContent.mockResolvedValueOnce(undefined);

    planBuildStateService.startBuild(PLAN_FILE, ['t1', 't2']);
    const events: string[] = [];
    planBuildStateService.subscribe(PLAN_FILE, (e) => events.push(e.type));

    window.dispatchEvent(
      new CustomEvent('bitfun:todowrite-update', {
        detail: {
          sessionId: 's1',
          turnId: 't1',
          todos: [
            { id: 't1', content: 'first', status: 'completed' },
            { id: 't2', content: 'second', status: 'completed' },
          ],
          merge: true,
        },
      }),
    );

    await vi.waitFor(() => {
      expect(mocks.writeFileContent).toHaveBeenCalledTimes(1);
    });

    expect(events).toContain('build-completed');
    expect(planBuildStateService.isBuildActive(PLAN_FILE)).toBe(false);
  });

  it('cancelBuild emits build-cancelled and clears the build', () => {
    const events: string[] = [];
    planBuildStateService.subscribe(PLAN_FILE, (e) => events.push(e.type));

    planBuildStateService.startBuild(PLAN_FILE, ['t1']);
    planBuildStateService.cancelBuild(PLAN_FILE);

    expect(events).toEqual(['build-started', 'build-cancelled']);
    expect(planBuildStateService.isBuildActive(PLAN_FILE)).toBe(false);
  });
});

