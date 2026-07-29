import type { DispatchWorkspaceProbe } from './types';

export const DISPATCH_PROTOCOL_VERSION = 1;

export const BASE_DISPATCH_CAPABILITIES = [
  'persistent_jobs',
  'cursor_events',
  'detached_worker',
  'frontend_event_projection',
  'workspace_serialization',
] as const;

export function isDispatchWorkspaceReady(
  workspacePath: string,
  workspace: DispatchWorkspaceProbe | undefined,
): boolean {
  const normalizedPath = workspacePath.trim();
  return (
    normalizedPath.length > 0 &&
    workspace?.path === normalizedPath &&
    workspace.exists === true &&
    workspace.isDirectory === true
  );
}
