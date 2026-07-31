/**
 * Row state for the issue-fix panel.
 *
 * Kept as pure functions so the mapping from LoopX's decisions onto what a user
 * sees is testable without rendering. The mapping matters: a `user_gate` shown as
 * "done" would hide the one case that needs a person.
 */

/** What LoopX says should happen next for an issue. Mirrors the Rust `NextStep`. */
export type IssueFixNextStep =
  | 'runnable_successor'
  | 'monitor_continuation'
  | 'user_gate'
  | 'no_followup';

/** Which resolution LoopX selected. Mirrors the Rust `FixRoute`. */
export type IssueFixRoute = 'fix_pr' | 'comment_only' | 'triage_only';

/** What a row shows. */
export type IssueFixRowState =
  /** Selected, not started. */
  | 'queued'
  /** Being worked on now. */
  | 'fixing'
  /** Finished, whatever the outcome. */
  | 'done'
  /** Stopped, waiting on a person. */
  | 'blocked'
  /** Not selected. */
  | 'idle';

export interface IssueFixRunEntry {
  issueId: string;
  route?: IssueFixRoute;
  nextStep?: IssueFixNextStep;
  /** LoopX's reason codes, shown verbatim rather than reinterpreted. */
  reasonCodes?: string[];
  pullRequestUrl?: string | null;
  /** Set when the run failed for a reason outside LoopX's decisions. */
  error?: string | null;
}

export interface IssueFixRunState {
  /** Issues the user selected. */
  selectedIssueIds: Set<string>;
  /** The issue currently being worked, if any. */
  activeIssueId?: string | null;
  /** Per-issue results, keyed by issue id. */
  entries: Record<string, IssueFixRunEntry>;
}

export function emptyRunState(): IssueFixRunState {
  return { selectedIssueIds: new Set(), activeIssueId: null, entries: {} };
}

/**
 * Whether this step means a person has to act before anything else happens.
 *
 * LoopX raises `user_gate` for semantic ambiguity and for missing write
 * authority. Crossing it automatically would defeat the gate, so the UI must
 * make it visually distinct from a completed row.
 */
export function requiresHuman(step: IssueFixNextStep | undefined): boolean {
  return step === 'user_gate';
}

/** Whether a route can lead to a pull request at all. */
export function permitsPullRequest(route: IssueFixRoute | undefined): boolean {
  return route === 'fix_pr';
}

/**
 * Resolve one row's state.
 *
 * Order matters. A blocked entry outranks "active" because a gated issue is not
 * progressing even while it is the current one, and an errored entry outranks a
 * decision because the decision may be stale.
 */
export function rowState(state: IssueFixRunState, issueId: string): IssueFixRowState {
  const entry = state.entries[issueId];
  if (entry?.error) {
    return 'blocked';
  }
  if (requiresHuman(entry?.nextStep)) {
    return 'blocked';
  }
  if (entry?.nextStep) {
    return 'done';
  }
  if (state.activeIssueId === issueId) {
    return 'fixing';
  }
  if (state.selectedIssueIds.has(issueId)) {
    return 'queued';
  }
  return 'idle';
}

/** Whether a row's checkbox should be locked. */
export function rowLocked(state: IssueFixRunState, issueId: string): boolean {
  const row = rowState(state, issueId);
  return row === 'fixing' || row === 'done' || row === 'blocked';
}

/**
 * A short i18n key suffix describing why a row is in its state.
 *
 * Returns null when there is nothing to explain, so a caller can omit the label
 * rather than render an empty one.
 */
export function rowStatusKey(state: IssueFixRunState, issueId: string): string | null {
  const entry = state.entries[issueId];
  if (entry?.error) {
    return 'stopped';
  }
  if (requiresHuman(entry?.nextStep)) {
    return 'awaitingDecision';
  }
  switch (rowState(state, issueId)) {
    case 'fixing':
      return 'fixing';
    case 'done':
      return entry?.pullRequestUrl ? 'pullRequestOpened' : 'resolvedWithoutPullRequest';
    case 'queued':
      return 'queued';
    default:
      return null;
  }
}

/** Toggle one issue's selection, leaving locked rows alone. */
export function toggleSelection(state: IssueFixRunState, issueId: string): IssueFixRunState {
  if (rowLocked(state, issueId)) {
    return state;
  }
  const selectedIssueIds = new Set(state.selectedIssueIds);
  if (selectedIssueIds.has(issueId)) {
    selectedIssueIds.delete(issueId);
  } else {
    selectedIssueIds.add(issueId);
  }
  return { ...state, selectedIssueIds };
}

/** Select or clear every selectable issue. */
export function setAllSelected(
  state: IssueFixRunState,
  issueIds: string[],
  selected: boolean,
): IssueFixRunState {
  const selectedIssueIds = new Set(state.selectedIssueIds);
  for (const issueId of issueIds) {
    if (rowLocked(state, issueId)) {
      continue;
    }
    if (selected) {
      selectedIssueIds.add(issueId);
    } else {
      selectedIssueIds.delete(issueId);
    }
  }
  return { ...state, selectedIssueIds };
}

/** Tri-state for the select-all control. */
export function selectAllState(
  state: IssueFixRunState,
  issueIds: string[],
): 'none' | 'some' | 'all' {
  const selectable = issueIds.filter((issueId) => !rowLocked(state, issueId));
  if (selectable.length === 0) {
    return 'none';
  }
  const selected = selectable.filter((issueId) => state.selectedIssueIds.has(issueId));
  if (selected.length === 0) {
    return 'none';
  }
  return selected.length === selectable.length ? 'all' : 'some';
}

/** Record one issue's outcome. */
export function recordOutcome(
  state: IssueFixRunState,
  entry: IssueFixRunEntry,
): IssueFixRunState {
  const entries = { ...state.entries, [entry.issueId]: entry };
  // Clear the active marker when the issue that finished was the active one, so
  // a completed row does not keep rendering as in-progress.
  const activeIssueId = state.activeIssueId === entry.issueId ? null : state.activeIssueId;
  return { ...state, entries, activeIssueId };
}

/**
 * The next issue to work, in the order given.
 *
 * Returns null when a gate is open: a blocked issue must be resolved by a person
 * before the run continues, so advancing past it would skip the gate.
 */
export function nextIssueToRun(state: IssueFixRunState, issueIds: string[]): string | null {
  for (const issueId of issueIds) {
    const row = rowState(state, issueId);
    if (row === 'blocked') {
      return null;
    }
    if (row === 'queued') {
      return issueId;
    }
  }
  return null;
}

/** Whether the run has stopped because something needs a person. */
export function isBlockedOnHuman(state: IssueFixRunState, issueIds: string[]): boolean {
  return issueIds.some((issueId) => rowState(state, issueId) === 'blocked');
}

/** Counts for the panel's summary line. */
export function runProgress(
  state: IssueFixRunState,
  issueIds: string[],
): { total: number; done: number; blocked: number; queued: number } {
  let done = 0;
  let blocked = 0;
  let queued = 0;
  for (const issueId of issueIds) {
    switch (rowState(state, issueId)) {
      case 'done':
        done += 1;
        break;
      case 'blocked':
        blocked += 1;
        break;
      case 'queued':
      case 'fixing':
        queued += 1;
        break;
      default:
        break;
    }
  }
  return { total: issueIds.length, done, blocked, queued };
}
