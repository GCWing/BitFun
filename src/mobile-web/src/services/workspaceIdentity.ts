import type { RecentWorkspaceEntry, SessionInfo } from './RemoteSessionManager';

export type WorkspaceIdentity = Pick<
  RecentWorkspaceEntry,
  'path' | 'remote_connection_id' | 'remote_ssh_host'
>;

export function workspaceIdentityKey(workspace: WorkspaceIdentity): string {
  return JSON.stringify([
    workspace.remote_connection_id ?? null,
    workspace.remote_ssh_host ?? null,
    workspace.path,
  ]);
}

/** Legacy cached rows have no host provenance. Only project them when local
 * ownership is unambiguous; a live scoped listing will restore remote rows. */
export function sessionMatchesWorkspace(
  session: SessionInfo,
  workspace: WorkspaceIdentity,
  catalog: WorkspaceIdentity[] = [],
): boolean {
  if (session.workspace_identity) {
    return workspaceIdentityKey(session.workspace_identity) === workspaceIdentityKey(workspace);
  }
  return session.workspace_path === workspace.path
    && !workspace.remote_connection_id
    && !workspace.remote_ssh_host
    && !catalog.some((candidate) => candidate.path === workspace.path
      && !!(candidate.remote_connection_id || candidate.remote_ssh_host));
}

export function mergeWorkspaceSessions(
  existing: SessionInfo[],
  incoming: SessionInfo[],
  workspace: WorkspaceIdentity | undefined,
  replaceWorkspace: boolean,
): SessionInfo[] {
  const retained = !replaceWorkspace
    ? existing
    : workspace
      ? existing.filter((session) => !session.workspace_identity
        || workspaceIdentityKey(session.workspace_identity) !== workspaceIdentityKey(workspace))
      : [];
  const merged = new Map(retained.map((session) => [session.session_id, session]));
  incoming.forEach((session) => {
    const previous = merged.get(session.session_id);
    merged.set(session.session_id, {
      ...session,
      workspace_path: session.workspace_path || workspace?.path,
      workspace_identity: session.workspace_identity ?? workspace ?? previous?.workspace_identity,
    });
  });
  return [...merged.values()];
}
