import type {
  DispatchWorkspaceProbe,
} from './types';

export const DISPATCH_PROTOCOL_VERSION = 2;

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
  probedWorkspacePath: string | undefined = workspace?.path,
): boolean {
  const normalizedPath = workspacePath.trim();
  return (
    normalizedPath.length > 0 &&
    normalizedPath === probedWorkspacePath?.trim() &&
    workspace?.exists === true &&
    workspace?.isDirectory === true &&
    !!workspace?.path.trim()
  );
}
