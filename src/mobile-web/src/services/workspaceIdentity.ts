import type { AssistantEntry, RecentWorkspaceEntry, SessionInfo } from './RemoteSessionManager';

export interface WorkspaceCatalog {
  workspaces: RecentWorkspaceEntry[];
  /** Absent opened_workspaces on the wire means a legacy host, not an empty catalog. */
  source: 'opened' | 'recent';
}

export function projectWorkspaceCatalog(
  response: { workspaces: RecentWorkspaceEntry[]; opened_workspaces?: RecentWorkspaceEntry[] | null },
  assistants: AssistantEntry[] = [],
): WorkspaceCatalog {
  const source = Array.isArray(response.opened_workspaces) ? 'opened' : 'recent';
  const rows = source === 'opened' ? response.opened_workspaces! : [
    ...assistants.map((assistant): RecentWorkspaceEntry => ({
      path: assistant.path, name: assistant.name, last_opened: '', workspace_kind: 'assistant',
    })),
    ...response.workspaces,
  ];
  const seen = new Set<string>();
  return {
    source,
    workspaces: rows.filter((workspace) => {
      if (!workspace.path) return false;
      const key = workspaceIdentityKey(workspace);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    }),
  };
}

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
