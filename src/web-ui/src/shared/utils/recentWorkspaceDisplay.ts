import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';

/** Remote POSIX path without leading slash (e.g. `/mnt/vdb/lfs` → `mnt/vdb/lfs`) for list readability. */
function compactRemotePathForRecentList(rootPath: string, displayName: string): string {
  let s = rootPath.replace(/\\/g, '/');
  while (s.includes('//')) {
    s = s.replace('//', '/');
  }
  if (s === '/' || s.trim() === '') {
    return displayName.trim() || '/';
  }
  const trimmed = s.replace(/\/+$/, '');
  const body = trimmed.startsWith('/') ? trimmed.slice(1) : trimmed;
  return body.trim() || displayName;
}

export type RecentWorkspaceLineParts = {
  /** SSH host (or connection display name) for remote; omit for local. */
  hostPrefix: string | null;
  folderLabel: string;
  /** Tooltip: `host:path` for remote, path for local. */
  tooltip: string;
};

/**
 * Labels for recent-workspace rows: remote shows host (VS Code–style disambiguation).
 */
export function getRecentWorkspaceLineParts(workspace: WorkspaceInfo): RecentWorkspaceLineParts {
  if (workspace.workspaceKind !== WorkspaceKind.Remote) {
    return {
      hostPrefix: null,
      folderLabel: workspace.name,
      tooltip: workspace.rootPath,
    };
  }
  const host =
    workspace.sshHost?.trim() ||
    workspace.connectionName?.trim() ||
    null;
  const tooltipBase = host ?? workspace.connectionId?.trim() ?? '';
  const tooltip =
    tooltipBase.length > 0 ? `${tooltipBase}:${workspace.rootPath}` : workspace.rootPath;
  return {
    hostPrefix: host,
    folderLabel: compactRemotePathForRecentList(workspace.rootPath, workspace.name),
    tooltip,
  };
}
