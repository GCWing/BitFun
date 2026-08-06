import { describe, expect, it } from 'vitest';
import type {
  IssueFixAutonomousPollResponse,
  IssueFixAutonomousStatusResponse,
} from '@/infrastructure/api';
import {
  emptyRunState,
  mergeLightState,
  pruneSelection,
  rowLocked,
  rowState,
  runProgress,
  selectAllState,
  setAllSelected,
  toggleSelection,
  userTodoDisplayText,
  userTodoPresentation,
} from './issueFixRunState';

const ISSUES = ['1849', '1580', '1920'];

function control(
  patch: Partial<IssueFixAutonomousStatusResponse> = {},
): IssueFixAutonomousStatusResponse {
  return {
    goalId: 'bitfun-goal',
    agentId: 'bitfun-cron',
    kernelState: 'runnable',
    shouldRun: true,
    actionRequired: false,
    selectedTodoId: 'todo_1849',
    issues: [
      {
        issueRef: '1849',
        issueUrl: 'https://github.com/owner/repo/issues/1849',
        todoId: 'todo_1849',
        status: 'open',
        selected: true,
      },
      {
        issueRef: '1580',
        issueUrl: 'https://github.com/owner/repo/issues/1580',
        todoId: 'todo_1580',
        status: 'open',
        selected: false,
      },
    ],
    hostLoop: {
      enabled: true,
      jobId: 'cron-1',
      sessionId: 'session-1',
      activeTurnId: 'turn-1',
    },
    ...patch,
  };
}

describe('LoopX row projection', () => {
  it('shows the Kernel-selected issue as fixing and the second issue as queued', () => {
    const state = emptyRunState();
    expect(rowState(state, control(), '1849')).toBe('fixing');
    expect(rowState(state, control(), '1580')).toBe('queued');
  });

  it('survives a UI remount because persisted todos are the source', () => {
    expect(rowState(emptyRunState(), control(), '1580')).toBe('queued');
  });

  it('projects an operator gate as blocked without skipping the queued issue', () => {
    const gated = control({ kernelState: 'operator_gate', shouldRun: false, actionRequired: true });
    expect(rowState(emptyRunState(), gated, '1849')).toBe('blocked');
    expect(rowState(emptyRunState(), gated, '1580')).toBe('queued');
  });

  it('locks every persisted todo against local removal', () => {
    const state = emptyRunState();
    expect(rowLocked(control(), '1849')).toBe(true);
    expect(toggleSelection(state, control(), '1849')).toBe(state);
  });
});

describe('local selection', () => {
  it('selects only issues not already persisted in LoopX', () => {
    const selected = setAllSelected(emptyRunState(), control(), ISSUES, true);
    expect([...selected.selectedIssueIds]).toEqual(['1920']);
    expect(selectAllState(selected, control(), ISSUES)).toBe('all');
  });

  it('counts Kernel and local states together', () => {
    const selected = toggleSelection(emptyRunState(), control(), '1920');
    expect(runProgress(selected, control(), ISSUES)).toEqual({
      total: 3,
      done: 0,
      blocked: 0,
      queued: 2,
      fixing: 1,
    });
  });

  it('drops selected ids for issues that vanished from a refreshed list', () => {
    const selected = toggleSelection(emptyRunState(), null, '1920');
    expect(pruneSelection(selected, ['1849', '1580']).selectedIssueIds.size).toBe(0);
    // Unchanged selections keep their identity so React state does not churn.
    expect(pruneSelection(selected, ISSUES)).toBe(selected);
  });
});

describe('light poll merge', () => {
  function poll(
    patch: Partial<IssueFixAutonomousPollResponse> = {},
  ): IssueFixAutonomousPollResponse {
    const base = control();
    return {
      goalId: base.goalId,
      agentId: base.agentId,
      actionRequired: false,
      issues: base.issues.map((issue) => ({ ...issue, selected: false })),
      userQuestion: null,
      hostLoop: { ...base.hostLoop, activeTurnId: null },
      ...patch,
    };
  }

  it('keeps quota-derived fields and re-derives the selected flag', () => {
    const merged = mergeLightState(control(), poll());
    expect(merged.kernelState).toBe('runnable');
    expect(merged.shouldRun).toBe(true);
    expect(merged.issues.find((issue) => issue.issueRef === '1849')?.selected).toBe(true);
    expect(merged.hostLoop.activeTurnId).toBeNull();
  });

  it('carries user todos from the poll into the merged projection', () => {
    const todo = {
      todoId: 'todo_review',
      taskClass: 'user_action',
      text: 'Review and merge PR #2054',
      link: 'https://github.com/owner/repo/pull/2054',
    };
    const withTodos = mergeLightState(control(), poll({ userTodos: [todo] }));
    expect(withTodos.userTodos).toEqual([todo]);
    // A follow-up poll without the todo clears the block.
    expect(mergeLightState(withTodos, poll()).userTodos).toEqual([]);
  });

  it('drops the linked URL from the display text since the jump icon carries it', () => {
    expect(
      userTodoDisplayText({
        todoId: 't',
        taskClass: 'user_action',
        text: 'Merge PR #2038 (https://github.com/o/r/pull/2038) — fixes #1980 · CI green',
        link: 'https://github.com/o/r/pull/2038',
      }),
    ).toBe('Merge PR #2038 — fixes #1980 · CI green');
    expect(
      userTodoDisplayText({
        todoId: 't',
        taskClass: 'user_gate',
        text: 'Authorize closing issue #2016',
        link: null,
      }),
    ).toBe('Authorize closing issue #2016');
  });

  it('projects legacy user actions as a short action plus current context', () => {
    expect(
      userTodoPresentation({
        todoId: 'comment-2032',
        taskClass: 'user_action',
        text: '[P0] Authorize posting maintainer diagnosis comment for GCWing/BitFun#2032 (comment_only route: diagnosis drafted)',
        link: null,
      }),
    ).toEqual({
      action: 'Posting maintainer diagnosis comment for Issue #2032',
      context: 'Comment_only route: diagnosis drafted',
      kind: { type: 'postComment', issue: '2032' },
    });

    expect(
      userTodoPresentation({
        todoId: 'merge-2038',
        taskClass: 'user_action',
        text: 'Merge PR #2038 (https://github.com/o/r/pull/2038) \u2014 fixes #1980 \u00b7 CI green, validated',
        link: 'https://github.com/o/r/pull/2038',
      }),
    ).toEqual({
      action: 'Merge PR #2038',
      context: 'Fixes #1980 \u00b7 CI green, validated',
      kind: { type: 'mergePr', pr: '2038', issue: '1980' },
    });
  });

  it('bounds the action line but preserves the full context of an old drafted-response todo', () => {
    const presentation = userTodoPresentation({
      todoId: 'comment-1290',
      taskClass: 'user_action',
      text: '[P0] Authorize posting response comment for GCWing/BitFun#1290 (discussion question: user asks about a chat group; the drafted response includes every support channel and a long explanation)',
      link: null,
    });

    expect(presentation.action.length).toBeLessThanOrEqual(72);
    // The context is intentionally NOT truncated: it is the user's only view
    // of the current state/reason, and the center wraps long text instead.
    expect(presentation.context).toBe(
      'Discussion question: user asks about a chat group; the drafted response includes every support channel and a long explanation',
    );
  });

  it('surfaces a gate discovered by the poll and clears an answered one', () => {
    const question = { todoId: 'gate_1849', prompt: 'Publish the validated PR?' };
    const withGate = mergeLightState(control(), poll({ actionRequired: true, userQuestion: question }));
    expect(withGate.actionRequired).toBe(true);
    expect(withGate.userQuestion).toEqual(question);
    expect(withGate.gatePrompt).toBe(question.prompt);

    const answered = mergeLightState(withGate, poll());
    expect(answered.actionRequired).toBe(false);
    expect(answered.userQuestion).toBeNull();
    expect(answered.gatePrompt).toBeNull();
  });
});
