export const OPEN_WORKTREE_MANAGER_EVENT = 'bitfun:open-worktree-manager';
export const OPEN_WORKTREE_LAUNCHER_EVENT = 'bitfun:open-worktree-launcher';

export type WorktreeLauncherMode = 'agentic' | 'Cowork';

export function openWorktreeManager(projectWorkspacePath: string): void {
  window.dispatchEvent(new CustomEvent(OPEN_WORKTREE_MANAGER_EVENT, {
    detail: { projectWorkspacePath },
  }));
}

export function openWorktreeLauncher(
  projectWorkspacePath: string,
  mode: WorktreeLauncherMode = 'agentic',
): void {
  window.dispatchEvent(new CustomEvent(OPEN_WORKTREE_LAUNCHER_EVENT, {
    detail: { projectWorkspacePath, mode },
  }));
}
