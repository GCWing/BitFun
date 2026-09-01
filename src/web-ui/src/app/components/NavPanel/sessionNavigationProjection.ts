import type { WorkspaceInfo } from '@/shared/types';
import { isLinkedWorktreeWorkspace, isRemoteWorkspace } from '@/shared/types';
import { isSamePath } from '@/shared/utils/pathUtils';

export type SessionNavigationScope = 'all' | 'assistants' | 'projects';
export type WorkspaceBackedSessionGroupKind = 'assistant' | 'project';

/**
 * Navigation-facing projection of a session owner.
 *
 * Projects and assistants currently share the WorkspaceInfo persistence model,
 * but the sidebar treats them as peer session groups. Long-term tracking should
 * join this projection through its own durable owner identity once that contract
 * exists; it must not be inferred from a session title.
 */
export interface WorkspaceBackedSessionGroup {
  groupId: `workspace:${string}`;
  kind: WorkspaceBackedSessionGroupKind;
  workspace: WorkspaceInfo;
}

/**
 * Resolve the active sidebar group without treating a remote POSIX path as a
 * workspace identity. The same path can be open on multiple remote hosts, so
 * remote workspaces must match by their stable workspace id. Local path
 * matching remains available for a linked worktree whose canonical project is
 * the visible navigation group.
 */
export function isWorkspaceBackedSessionGroupActive(
  workspace: WorkspaceInfo,
  activeWorkspace: WorkspaceInfo | null | undefined,
): boolean {
  if (!activeWorkspace) return false;
  if (workspace.id === activeWorkspace.id) return true;
  if (isRemoteWorkspace(workspace) || isRemoteWorkspace(activeWorkspace)) return false;

  const activeProjectPath = activeWorkspace.worktree && !activeWorkspace.worktree.isMain
    ? activeWorkspace.worktree.mainRepoPath
    : activeWorkspace.rootPath;

  return Boolean(activeProjectPath && isSamePath(workspace.rootPath, activeProjectPath));
}

const isWorkspaceInScope = (
  workspace: WorkspaceInfo,
  scope: SessionNavigationScope,
): boolean => {
  if (scope === 'all') return true;
  return scope === 'assistants'
    ? workspace.workspaceKind === 'assistant'
    : workspace.workspaceKind !== 'assistant';
};

export function projectWorkspaceBackedSessionGroups(
  openedWorkspaces: readonly WorkspaceInfo[],
  scope: SessionNavigationScope,
): WorkspaceBackedSessionGroup[] {
  const scopedWorkspaces = openedWorkspaces.filter(workspace => (
    isWorkspaceInScope(workspace, scope)
  ));

  const projectRoots = scopedWorkspaces
    .filter(workspace => workspace.workspaceKind !== 'assistant')
    .filter(workspace => !isLinkedWorktreeWorkspace(workspace))
    .map(workspace => workspace.rootPath);

  return scopedWorkspaces
    .filter(workspace => (
      workspace.workspaceKind === 'assistant'
      || !isLinkedWorktreeWorkspace(workspace)
      || !projectRoots.some(projectRoot => (
        isSamePath(projectRoot, workspace.worktree?.mainRepoPath || '')
      ))
    ))
    .map(workspace => ({
      groupId: `workspace:${workspace.id}`,
      kind: workspace.workspaceKind === 'assistant' ? 'assistant' : 'project',
      workspace,
    }));
}
