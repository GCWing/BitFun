import { isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';
import type { GitBasicInfoOptions } from '@/tools/git/hooks/useGitState';

export function getWorkspaceGitBasicInfoOptions(
  workspace: WorkspaceInfo,
  isActive: boolean
): GitBasicInfoOptions | undefined {
  if (isRemoteWorkspace(workspace)) {
    return undefined;
  }

  return {
    isActive,
    refreshOnMount: isActive,
    refreshOnActive: true,
    participateInWindowFocusRefresh: false,
  };
}
