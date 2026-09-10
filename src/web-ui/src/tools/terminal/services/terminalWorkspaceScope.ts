import type { SessionResponse } from '../types/session';

export interface TerminalWorkspaceScope {
  rootPath: string;
  connectionId?: string | null;
  isRemote: boolean;
}

/** Target path semantics, independent of the controller's operating system. */
export function normalizeTerminalPath(path: string, remote = false): string {
  const windows = !remote && (/^[a-z]:[/\\]/i.test(path) || path.startsWith('\\\\'));
  const normalized = windows ? path.replace(/\\/g, '/').toLowerCase() : path;
  return normalized.replace(/\/+$/, '') || '/';
}

export function isTerminalPathInside(path: string, root: string, remote = false): boolean {
  if (!path || !root) return false;
  const candidate = normalizeTerminalPath(path, remote);
  const parent = normalizeTerminalPath(root, remote);
  return candidate === parent || candidate.startsWith(parent === '/' ? '/' : `${parent}/`);
}

export function terminalMatchesEnvironment(session: SessionResponse, scope: TerminalWorkspaceScope): boolean {
  const remote = session.shellType === 'Remote' || Boolean(session.connectionId);
  return scope.isRemote
    ? remote && Boolean(scope.connectionId) && session.connectionId === scope.connectionId
    : !remote;
}

/** Prefer the closest opened workspace, so nested projects do not share terminals. */
export function terminalBelongsToWorkspace(
  session: SessionResponse,
  scope: TerminalWorkspaceScope,
  workspaces: TerminalWorkspaceScope[] = [],
): boolean {
  if (!terminalMatchesEnvironment(session, scope)) return false;
  const origin = session.initialCwd || session.cwd;
  if (!isTerminalPathInside(origin, scope.rootPath, scope.isRemote)) return false;
  const root = normalizeTerminalPath(scope.rootPath, scope.isRemote);
  return !workspaces.some(other =>
    terminalMatchesEnvironment(session, other)
    && normalizeTerminalPath(other.rootPath, other.isRemote).length > root.length
    && isTerminalPathInside(origin, other.rootPath, other.isRemote),
  );
}

/** Legacy hosts expose only cwd. Remember the first observation for this service lifetime. */
export class TerminalOriginCache {
  private origins = new Map<string, string>();

  project(surfaceId: string, session: SessionResponse): SessionResponse {
    const key = JSON.stringify([surfaceId, session.connectionId ?? '', session.id]);
    const initialCwd = session.initialCwd || this.origins.get(key) || session.cwd;
    if (initialCwd) this.origins.set(key, initialCwd);
    return { ...session, initialCwd };
  }
}
