import type { Session } from '../types/flow-chat';

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
  ].join('|');
}
