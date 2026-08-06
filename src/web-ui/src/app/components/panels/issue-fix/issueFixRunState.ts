import type {
  IssueFixAutonomousPollResponse,
  IssueFixAutonomousStatusResponse,
  IssueFixUserTodo,
} from '@/infrastructure/api';

export type IssueFixRowState = 'idle' | 'queued' | 'fixing' | 'done' | 'blocked';

/**
 * Display text for a pending user todo. The primary URL is already surfaced
 * as a jump icon, so repeating it inline only costs density; drop it together
 * with any now-empty "()" shell around it.
 */
export function userTodoDisplayText(todo: IssueFixUserTodo): string {
  if (!todo.link) return todo.text;
  return todo.text
    .split(todo.link)
    .join('')
    .replace(/\(\s*\)/g, '')
    .replace(/\s{2,}/g, ' ')
    .trim();
}

export interface IssueFixUserTodoPresentation {
  action: string;
  context: string | null;
  /** Structured reading of the action for localized rendering; null when the
   * free-form text did not match any known shape. */
  kind:
    | { type: 'mergePr'; pr: string; issue: string | null }
    | { type: 'closeIssue'; issue: string; pr: string | null }
    | { type: 'postComment'; issue: string }
    | null;
}

const MAX_USER_TODO_ACTION_CHARS = 72;
const MAX_USER_TODO_CONTEXT_CHARS = 96;

function truncateUserTodoPart(value: string, maxChars: number): string {
  if (value.length <= maxChars) return value;
  return `${value.slice(0, Math.max(0, maxChars - 1)).trimEnd()}\u2026`;
}

function capitalizeAscii(value: string): string {
  return value ? `${value[0].toUpperCase()}${value.slice(1)}` : value;
}

/**
 * Recognize the three action shapes the heartbeat contract produces, so the
 * UI can phrase them in the user's language instead of echoing agent prose.
 */
function classifyUserTodoAction(
  action: string,
  context: string | null,
): IssueFixUserTodoPresentation['kind'] {
  const scan = `${action} ${context ?? ''}`;
  const pr = /(?:\bPR\s*#|\bpull\/)(\d+)/i.exec(scan)?.[1] ?? null;
  // "Issue #N", "fixes #N", or any bare "#N" that is not the PR number.
  const issue =
    /\b(?:issue|fix(?:es)?|resolv(?:es)?|clos(?:es|ing)?)\s*#?(\d+)/i.exec(scan)?.[1] ??
    [...scan.matchAll(/#(\d+)/g)].map((m) => m[1]).find((n) => n !== pr) ??
    null;
  if (/\bmerge\b/i.test(action) && pr) {
    return { type: 'mergePr', pr, issue: issue !== pr ? issue : null };
  }
  if (/\bclos(?:e|ing)\b/i.test(action) && issue) {
    return { type: 'closeIssue', issue, pr };
  }
  if (/\b(?:comment|response|diagnos)/i.test(action) && issue) {
    return { type: 'postComment', issue };
  }
  return null;
}

/**
 * Project legacy free-form todo text into the notification center's two-line
 * layout. New heartbeats already write `action - state/reason`; the fallback
 * also keeps older parenthesized and colon-delimited todos readable.
 */
export function userTodoPresentation(todo: IssueFixUserTodo): IssueFixUserTodoPresentation {
  let text = userTodoDisplayText(todo)
    .replace(/^\s*\[[^\]]+\]\s*/, '')
    .replace(/^\s*authorize\s+/i, '')
    .replace(/\b[\w.-]+\/[\w.-]+\s+issue\s+#(\d+)/gi, 'Issue #$1')
    .replace(/\b[\w.-]+\/[\w.-]+#(\d+)/g, 'Issue #$1')
    .replace(/\s+/g, ' ')
    .trim();

  let trailing: string | null = null;
  const trailingContext = text.match(/\s+\(([^()]*)\)\s*[.!?]?$/);
  if (trailingContext?.index != null && trailingContext[1].trim()) {
    trailing = trailingContext[1].trim();
    text = text.slice(0, trailingContext.index).trim();
  } else {
    const divider = text.match(/\s+(?:\u2014|\u2013|\u00b7)\s+|:\s+/);
    if (divider?.index != null) {
      const detailStart = divider.index + divider[0].length;
      trailing = text.slice(detailStart).trim() || null;
      text = text.slice(0, divider.index).trim();
    }
  }

  const action = truncateUserTodoPart(capitalizeAscii(text), MAX_USER_TODO_ACTION_CHARS);
  const context = trailing
    ? truncateUserTodoPart(capitalizeAscii(trailing), MAX_USER_TODO_CONTEXT_CHARS)
    : null;
  return { action, context, kind: classifyUserTodoAction(text, trailing) };
}

export interface IssueFixSelectionState {
  selectedIssueIds: Set<string>;
}

export function emptyRunState(): IssueFixSelectionState {
  return { selectedIssueIds: new Set() };
}

/** Drop selected ids that no longer exist in the refreshed issue list. */
export function pruneSelection(
  selection: IssueFixSelectionState,
  issueIds: string[],
): IssueFixSelectionState {
  const known = new Set(issueIds);
  const selectedIssueIds = new Set(
    [...selection.selectedIssueIds].filter((issueId) => known.has(issueId)),
  );
  return selectedIssueIds.size === selection.selectedIssueIds.size
    ? selection
    : { selectedIssueIds };
}

/**
 * Fold a cheap poll (LoopX todo list + host loop, no quota packet) into the
 * last full projection. Quota-derived fields (kernelState, shouldRun,
 * recommendedAction, selectedTodoId) keep their previous values; the selected
 * flag is re-derived so rows do not flicker between polls.
 */
export function mergeLightState(
  control: IssueFixAutonomousStatusResponse,
  poll: IssueFixAutonomousPollResponse,
): IssueFixAutonomousStatusResponse {
  return {
    ...control,
    goalId: poll.goalId,
    agentId: poll.agentId,
    actionRequired: poll.actionRequired,
    gatePrompt: poll.userQuestion?.prompt ?? null,
    userQuestion: poll.userQuestion ?? null,
    issues: poll.issues.map((issue) => ({
      ...issue,
      selected: issue.todoId === control.selectedTodoId,
    })),
    userTodos: poll.userTodos ?? [],
    hostLoop: poll.hostLoop,
  };
}

function kernelTodo(
  control: IssueFixAutonomousStatusResponse | null,
  issueId: string,
) {
  return control?.issues.find((todo) => todo.issueRef === issueId);
}

/** Project a row from LoopX Kernel state plus unsaved checkbox selection. */
export function rowState(
  selection: IssueFixSelectionState,
  control: IssueFixAutonomousStatusResponse | null,
  issueId: string,
): IssueFixRowState {
  const todo = kernelTodo(control, issueId);
  if (!todo) {
    return selection.selectedIssueIds.has(issueId) ? 'queued' : 'idle';
  }
  if (todo.status === 'done') {
    return 'done';
  }
  if (todo.status === 'blocked') {
    return 'blocked';
  }
  if (todo.selected) {
    if (control?.actionRequired || control?.kernelState === 'operator_gate') {
      return 'blocked';
    }
    if (control?.hostLoop.enabled && (control.shouldRun || control.hostLoop.activeTurnId)) {
      return 'fixing';
    }
  }
  return 'queued';
}

/** Persisted LoopX rows cannot be removed from the queue by a checkbox. */
export function rowLocked(
  control: IssueFixAutonomousStatusResponse | null,
  issueId: string,
): boolean {
  return Boolean(kernelTodo(control, issueId));
}

export function rowStatusKey(state: IssueFixRowState): string | null {
  switch (state) {
    case 'queued':
      return 'queued';
    case 'fixing':
      return 'fixing';
    case 'done':
      return 'resolvedWithoutPullRequest';
    case 'blocked':
      return 'awaitingDecision';
    default:
      return null;
  }
}

export function toggleSelection(
  selection: IssueFixSelectionState,
  control: IssueFixAutonomousStatusResponse | null,
  issueId: string,
): IssueFixSelectionState {
  if (rowLocked(control, issueId)) {
    return selection;
  }
  const selectedIssueIds = new Set(selection.selectedIssueIds);
  if (selectedIssueIds.has(issueId)) {
    selectedIssueIds.delete(issueId);
  } else {
    selectedIssueIds.add(issueId);
  }
  return { selectedIssueIds };
}

export function setAllSelected(
  selection: IssueFixSelectionState,
  control: IssueFixAutonomousStatusResponse | null,
  issueIds: string[],
  selected: boolean,
): IssueFixSelectionState {
  const selectedIssueIds = new Set(selection.selectedIssueIds);
  for (const issueId of issueIds) {
    if (rowLocked(control, issueId)) {
      continue;
    }
    if (selected) {
      selectedIssueIds.add(issueId);
    } else {
      selectedIssueIds.delete(issueId);
    }
  }
  return { selectedIssueIds };
}

export function selectAllState(
  selection: IssueFixSelectionState,
  control: IssueFixAutonomousStatusResponse | null,
  issueIds: string[],
): 'none' | 'some' | 'all' {
  const selectable = issueIds.filter((issueId) => !rowLocked(control, issueId));
  if (selectable.length === 0) {
    return 'none';
  }
  const selected = selectable.filter((issueId) => selection.selectedIssueIds.has(issueId));
  if (selected.length === 0) {
    return 'none';
  }
  return selected.length === selectable.length ? 'all' : 'some';
}

export function runProgress(
  selection: IssueFixSelectionState,
  control: IssueFixAutonomousStatusResponse | null,
  issueIds: string[],
): { total: number; done: number; blocked: number; queued: number; fixing: number } {
  let done = 0;
  let blocked = 0;
  let queued = 0;
  let fixing = 0;
  for (const issueId of issueIds) {
    switch (rowState(selection, control, issueId)) {
      case 'done':
        done += 1;
        break;
      case 'blocked':
        blocked += 1;
        break;
      case 'queued':
        queued += 1;
        break;
      case 'fixing':
        fixing += 1;
        break;
      default:
        break;
    }
  }
  return { total: issueIds.length, done, blocked, queued, fixing };
}
