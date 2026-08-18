/**
 * The usage report, held for as long as it is on screen and no longer.
 *
 * It used to be a `DialogTurn` appended to `session.dialogTurns` and persisted
 * to the backend as a `local_command` turn. A report about a session is not an
 * event in it: the turn carried no user message and no model round, and the
 * backend already answered "is this part of the conversation" with no in six
 * places — `is_model_visible` and `is_transcript_visible` are both false for
 * `LocalCommand`, and the usage report excludes its own turns from its own
 * scope.
 *
 * The front end could not keep the same story straight. The turn grew
 * `dialogTurns` and moved `at(-1)`, which is exactly what follow-output reads
 * as a Turn arriving, so a report pinned itself to the top of the viewport. And
 * the filter meant to keep it out of the Turn rail required
 * `usageReportProvisional`, a flag that `commitLocalUsageReportTurn` deleted on
 * successful persistence — so it worked until the moment persistence made it
 * matter, and a `/usage` report then took a numbered Turn slot.
 *
 * Reports written by earlier versions are still on disk and still render, in
 * `UserMessageItem`; only the writer is gone.
 *
 * Module state rather than React state because the caller is a service several
 * layers below any component — the same shape as `chatPopupState.ts`.
 */

import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';

export interface SessionUsageModalState {
  open: boolean;
  sessionId: string;
  workspacePath?: string;
  /** The request is in flight; the card draws its own waiting state. */
  isLoading: boolean;
  report?: SessionUsageReport;
  /** The rendered report, for copying and for the fallback view. */
  markdown: string;
}

const CLOSED: SessionUsageModalState = {
  open: false,
  sessionId: '',
  isLoading: false,
  markdown: '',
};

let state: SessionUsageModalState = CLOSED;
const listeners = new Set<() => void>();

function setState(next: SessionUsageModalState) {
  state = next;
  listeners.forEach(listener => listener());
}

/*
 * Identity is stable while nothing changes, which `useSyncExternalStore`
 * requires of a snapshot — it re-reads on every render and treats a new object
 * as a change.
 */
export function getSessionUsageModalState(): SessionUsageModalState {
  return state;
}

export function subscribeSessionUsageModal(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** Open on the waiting state, before the report exists. */
export function openSessionUsageModal(params: {
  sessionId: string;
  workspacePath?: string;
}): void {
  setState({
    open: true,
    sessionId: params.sessionId,
    workspacePath: params.workspacePath,
    isLoading: true,
    markdown: '',
  });
}

/**
 * Show the report that arrived.
 *
 * Ignored if the modal has been closed in the meantime: the reader dismissing
 * the wait is them saying they no longer want it, and a request already in
 * flight cannot be recalled.
 */
export function showSessionUsageModalReport(params: {
  sessionId: string;
  report: SessionUsageReport;
  markdown: string;
}): void {
  if (!state.open || state.sessionId !== params.sessionId) return;
  setState({
    ...state,
    isLoading: false,
    report: params.report,
    markdown: params.markdown,
  });
}

export function closeSessionUsageModal(): void {
  if (!state.open) return;
  setState(CLOSED);
}
