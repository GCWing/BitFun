import type { Session } from '../types/flow-chat';

type SessionWorkspaceBinding = Pick<
  Session,
  'workspacePath' | 'projectWorkspacePath' | 'config'
>;

/** Concrete root in which terminal, Git, and file tools execute. */
export function sessionExecutionWorkspacePath(
  session: SessionWorkspaceBinding,
): string | undefined {
  return session.workspacePath || session.config.workspacePath;
}

/** Main-project root that owns session persistence and orchestration state. */
export function sessionProjectWorkspacePath(
  session: SessionWorkspaceBinding,
): string | undefined {
  return (
    session.projectWorkspacePath
    || session.config.projectWorkspacePath
    || sessionExecutionWorkspacePath(session)
  );
}

export function requireSessionProjectWorkspacePath(
  session: SessionWorkspaceBinding,
  sessionId: string,
): string {
  const path = sessionProjectWorkspacePath(session);
  if (!path) {
    throw new Error(`Workspace path not found for session ${sessionId}`);
  }
  return path;
}
