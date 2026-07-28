import type { Session } from '../types/flow-chat';
import { sessionProjectWorkspacePath } from './sessionWorkspace';

type SessionWorktreeFacts = Pick<
  Session,
  'dialogTurns' | 'totalTurnCount' | 'workspaceId' | 'workspacePath' | 'projectWorkspacePath' | 'config'
>;

export function isSessionWorktreeBindingLocked(
  session: Pick<SessionWorktreeFacts, 'dialogTurns' | 'totalTurnCount'>,
  isProcessing: boolean,
): boolean {
  return session.dialogTurns.length > 0
    || (session.totalTurnCount ?? 0) > 0
    || isProcessing;
}

export function isSessionWorktreeMaterialized(
  session: Pick<SessionWorktreeFacts, 'config'>,
): boolean {
  return !!session.config.executionTarget?.worktreeId;
}

/** Checkbox state, including an unmaterialized choice made before first send. */
export function isSessionWorktreeIsolationEnabled(
  session: Pick<SessionWorktreeFacts, 'config'>,
): boolean {
  return session.config.worktreeIsolationRequested
    ?? isSessionWorktreeMaterialized(session);
}

export interface SessionWorktreeMaterializationPlan {
  enabled: boolean;
  projectWorkspacePath: string;
}

/**
 * Resolve the one transition that must run after a prompt is submitted and
 * before its backend turn starts. `undefined` means the checkbox has not
 * requested a change, or the session is already materialized as requested.
 */
export function sessionWorktreeMaterializationPlan(
  session: SessionWorktreeFacts,
): SessionWorktreeMaterializationPlan | undefined {
  const requested = session.config.worktreeIsolationRequested;
  if (
    requested === undefined
    || requested === isSessionWorktreeMaterialized(session)
  ) {
    return undefined;
  }

  const projectWorkspacePath = sessionProjectWorkspacePath(session)?.trim();
  if (!projectWorkspacePath) {
    throw new Error('Project workspace path is required to prepare worktree isolation');
  }
  return { enabled: requested, projectWorkspacePath };
}

/**
 * Fields read by the composer that can change after a historical-session
 * hydrate or a worktree transition. Including them in the store selector keeps
 * the toggle state and project locator from using a stale session snapshot.
 */
export function sessionWorktreeBindingSubscriptionKey(session: SessionWorktreeFacts): string {
  return [
    session.dialogTurns.length,
    session.totalTurnCount ?? '',
    session.workspaceId ?? '',
    session.workspacePath ?? '',
    session.projectWorkspacePath ?? '',
    session.config.projectWorkspacePath ?? '',
    session.config.executionTarget?.kind ?? '',
    session.config.executionTarget?.worktreeId ?? '',
    session.config.executionTarget?.rootPath ?? '',
    session.config.worktreeIsolationRequested ?? '',
  ].join('|');
}
