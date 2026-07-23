import { useEffect, useState } from 'react';
import { workspaceManager } from '../services/business/workspaceManager';
import { isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';

function readWorkspaceFields() {
  const ws = workspaceManager.getState().currentWorkspace;
  return {
    workspace: ws as WorkspaceInfo | null,
    workspacePath: ws?.rootPath ?? '',
    hasWorkspace: !!ws,
    isRemoteWorkspace: isRemoteWorkspace(ws),
  };
}

/**
 * Active workspace path and presence, synced from {@link workspaceManager}.
 * Use where React context may not match the app root provider (e.g. duplicate
 * context module in a lazy chunk); behavior aligns with WorkspaceProvider state.
 */
export function useWorkspaceManagerSync(): {
  workspace: WorkspaceInfo | null;
  workspacePath: string;
  hasWorkspace: boolean;
  isRemoteWorkspace: boolean;
} {
  const [fields, setFields] = useState(readWorkspaceFields);

  useEffect(() => {
    return workspaceManager.addEventListener(() => {
      setFields(readWorkspaceFields());
    });
  }, []);

  return fields;
}
