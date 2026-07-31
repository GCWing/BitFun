import { describe, expect, it } from 'vitest';
import {
  emptyRunState,
  isBlockedOnHuman,
  nextIssueToRun,
  permitsPullRequest,
  recordOutcome,
  requiresHuman,
  rowLocked,
  rowState,
  rowStatusKey,
  runProgress,
  selectAllState,
  setAllSelected,
  toggleSelection,
  type IssueFixRunState,
} from './issueFixRunState';

const ISSUES = ['1677', '1849', '1805', '1920'];

function withSelection(issueIds: string[]): IssueFixRunState {
  return { ...emptyRunState(), selectedIssueIds: new Set(issueIds) };
}

describe('requiresHuman', () => {
  it('is true only for a user gate', () => {
    expect(requiresHuman('user_gate')).toBe(true);
    for (const step of ['runnable_successor', 'monitor_continuation', 'no_followup'] as const) {
      expect(requiresHuman(step)).toBe(false);
    }
    expect(requiresHuman(undefined)).toBe(false);
  });
});

describe('permitsPullRequest', () => {
  it('is true only for the fix route', () => {
    expect(permitsPullRequest('fix_pr')).toBe(true);
    expect(permitsPullRequest('comment_only')).toBe(false);
    expect(permitsPullRequest('triage_only')).toBe(false);
    expect(permitsPullRequest(undefined)).toBe(false);
  });
});

describe('rowState', () => {
  it('reports idle for an unselected issue', () => {
    expect(rowState(emptyRunState(), '1849')).toBe('idle');
  });

  it('reports queued once selected', () => {
    expect(rowState(withSelection(['1849']), '1849')).toBe('queued');
  });

  it('reports fixing for the active issue', () => {
    const state = { ...withSelection(['1849']), activeIssueId: '1849' };
    expect(rowState(state, '1849')).toBe('fixing');
  });

  it('reports done once a decision arrives', () => {
    const state = recordOutcome(withSelection(['1849']), {
      issueId: '1849',
      route: 'fix_pr',
      nextStep: 'monitor_continuation',
    });
    expect(rowState(state, '1849')).toBe('done');
  });

  it('reports blocked rather than done for a user gate', () => {
    // The whole point of the gate: showing this as "done" would hide the one
    // case that needs a person.
    const state = recordOutcome(withSelection(['1920']), {
      issueId: '1920',
      route: 'fix_pr',
      nextStep: 'user_gate',
    });
    expect(rowState(state, '1920')).toBe('blocked');
  });

  it('reports blocked for an errored issue even without a decision', () => {
    const state = recordOutcome(withSelection(['1805']), {
      issueId: '1805',
      error: 'loopx exited with status 1',
    });
    expect(rowState(state, '1805')).toBe('blocked');
  });

  it('lets an error outrank a stale decision', () => {
    const state = recordOutcome(withSelection(['1805']), {
      issueId: '1805',
      nextStep: 'no_followup',
      error: 'validation command failed',
    });
    expect(rowState(state, '1805')).toBe('blocked');
  });

  it('lets a gate outrank the active marker', () => {
    const gated = recordOutcome(withSelection(['1920']), {
      issueId: '1920',
      nextStep: 'user_gate',
    });
    const state = { ...gated, activeIssueId: '1920' };
    expect(rowState(state, '1920')).toBe('blocked');
  });

  it('treats a triage decision as done, not blocked', () => {
    // LoopX declining to open a PR is a resolved outcome, not a gate.
    const state = recordOutcome(withSelection(['1687']), {
      issueId: '1687',
      route: 'triage_only',
      nextStep: 'no_followup',
    });
    expect(rowState(state, '1687')).toBe('done');
  });
});

describe('rowStatusKey', () => {
  it('distinguishes a pull request from a resolution without one', () => {
    const withPr = recordOutcome(emptyRunState(), {
      issueId: '1849',
      route: 'fix_pr',
      nextStep: 'monitor_continuation',
      pullRequestUrl: 'https://github.com/example/repo/pull/1',
    });
    expect(rowStatusKey(withPr, '1849')).toBe('pullRequestOpened');

    const withoutPr = recordOutcome(emptyRunState(), {
      issueId: '1687',
      route: 'triage_only',
      nextStep: 'no_followup',
    });
    expect(rowStatusKey(withoutPr, '1687')).toBe('resolvedWithoutPullRequest');
  });

  it('reports a gate and a failure separately', () => {
    const gated = recordOutcome(emptyRunState(), {
      issueId: '1920',
      nextStep: 'user_gate',
    });
    expect(rowStatusKey(gated, '1920')).toBe('awaitingDecision');

    const failed = recordOutcome(emptyRunState(), { issueId: '1805', error: 'boom' });
    expect(rowStatusKey(failed, '1805')).toBe('stopped');
  });

  it('returns null when there is nothing to explain', () => {
    expect(rowStatusKey(emptyRunState(), '1849')).toBeNull();
  });
});

describe('selection', () => {
  it('toggles an issue on and off', () => {
    let state = toggleSelection(emptyRunState(), '1849');
    expect(state.selectedIssueIds.has('1849')).toBe(true);
    state = toggleSelection(state, '1849');
    expect(state.selectedIssueIds.has('1849')).toBe(false);
  });

  it('refuses to toggle a locked row', () => {
    const done = recordOutcome(withSelection(['1849']), {
      issueId: '1849',
      nextStep: 'no_followup',
    });
    expect(rowLocked(done, '1849')).toBe(true);
    expect(toggleSelection(done, '1849')).toBe(done);
  });

  it('selects and clears every selectable issue', () => {
    const all = setAllSelected(emptyRunState(), ISSUES, true);
    expect(all.selectedIssueIds.size).toBe(ISSUES.length);
    const none = setAllSelected(all, ISSUES, false);
    expect(none.selectedIssueIds.size).toBe(0);
  });

  it('leaves locked rows out of a select-all', () => {
    const done = recordOutcome(emptyRunState(), {
      issueId: '1677',
      nextStep: 'no_followup',
    });
    const all = setAllSelected(done, ISSUES, true);
    expect(all.selectedIssueIds.has('1677')).toBe(false);
    expect(all.selectedIssueIds.size).toBe(ISSUES.length - 1);
  });

  it('reports the tri-state for select-all', () => {
    expect(selectAllState(emptyRunState(), ISSUES)).toBe('none');
    expect(selectAllState(withSelection(['1849']), ISSUES)).toBe('some');
    expect(selectAllState(withSelection(ISSUES), ISSUES)).toBe('all');
  });

  it('reports none when nothing is selectable', () => {
    let state = emptyRunState();
    for (const issueId of ISSUES) {
      state = recordOutcome(state, { issueId, nextStep: 'no_followup' });
    }
    expect(selectAllState(state, ISSUES)).toBe('none');
  });
});

describe('recordOutcome', () => {
  it('clears the active marker for the issue that finished', () => {
    const running = { ...withSelection(['1849']), activeIssueId: '1849' };
    const state = recordOutcome(running, { issueId: '1849', nextStep: 'no_followup' });
    expect(state.activeIssueId).toBeNull();
  });

  it('leaves another issue active', () => {
    const running = { ...withSelection(['1849', '1805']), activeIssueId: '1805' };
    const state = recordOutcome(running, { issueId: '1849', nextStep: 'no_followup' });
    expect(state.activeIssueId).toBe('1805');
  });
});

describe('nextIssueToRun', () => {
  it('returns the first queued issue in order', () => {
    expect(nextIssueToRun(withSelection(ISSUES), ISSUES)).toBe('1677');
  });

  it('skips issues that already finished', () => {
    const state = recordOutcome(withSelection(ISSUES), {
      issueId: '1677',
      nextStep: 'no_followup',
    });
    expect(nextIssueToRun(state, ISSUES)).toBe('1849');
  });

  it('stops at an open gate rather than skipping past it', () => {
    // Advancing here would cross the gate LoopX raised, which is exactly what
    // the gate exists to prevent.
    const state = recordOutcome(withSelection(ISSUES), {
      issueId: '1677',
      nextStep: 'user_gate',
    });
    expect(nextIssueToRun(state, ISSUES)).toBeNull();
    expect(isBlockedOnHuman(state, ISSUES)).toBe(true);
  });

  it('stops at a failure too', () => {
    const state = recordOutcome(withSelection(ISSUES), {
      issueId: '1849',
      error: 'loopx rejected the request',
    });
    // 1677 is still queued and comes first, so it runs before the failure.
    expect(nextIssueToRun(state, ISSUES)).toBe('1677');
    const afterFirst = recordOutcome(state, { issueId: '1677', nextStep: 'no_followup' });
    expect(nextIssueToRun(afterFirst, ISSUES)).toBeNull();
  });

  it('returns null when nothing is queued', () => {
    expect(nextIssueToRun(emptyRunState(), ISSUES)).toBeNull();
  });
});

describe('runProgress', () => {
  it('counts each state', () => {
    let state = withSelection(ISSUES);
    state = recordOutcome(state, { issueId: '1677', nextStep: 'no_followup' });
    state = recordOutcome(state, { issueId: '1849', nextStep: 'monitor_continuation' });
    state = recordOutcome(state, { issueId: '1920', nextStep: 'user_gate' });

    expect(runProgress(state, ISSUES)).toEqual({
      total: 4,
      done: 2,
      blocked: 1,
      queued: 1,
    });
  });

  it('counts nothing for an empty run', () => {
    expect(runProgress(emptyRunState(), ISSUES)).toEqual({
      total: 4,
      done: 0,
      blocked: 0,
      queued: 0,
    });
  });
});
