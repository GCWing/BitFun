import { useEffect, useCallback, useRef, useState } from 'react';
import { FileSystemNode } from '../types';
import { gitStateManager } from '@/tools/git/state/GitStateManager';
import { GitState } from '@/tools/git/state/types';
import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useFileTreeGitSync');

export interface UseFileTreeGitSyncProps {
  workspacePath?: string;
  fileTree: FileSystemNode[];
  onTreeUpdate: (tree: FileSystemNode[]) => void;
  autoRefresh?: boolean;
  debounceDelay?: number;
}

/**
 * Parse Git status from backend response.
 * Backend format: { status: 'M', index_status: null, workdir_status: 'M' }
 */
function parseGitStatusFromBackend(
  status: string,
  indexStatus: string | null | undefined,
  workdirStatus: string | null | undefined
): 'untracked' | 'modified' | 'added' | 'deleted' | 'renamed' | 'conflicted' | 'staged' | undefined {
  if (status === '?' || status === '??') return 'untracked';
  
  if (status === 'U' || (indexStatus && indexStatus.includes('U')) || (workdirStatus && workdirStatus.includes('U'))) {
    return 'conflicted';
  }
  
  if (indexStatus && indexStatus !== ' ') {
    if (indexStatus === 'A') return 'added';
    if (indexStatus === 'D') return 'deleted';
    if (indexStatus === 'R') return 'renamed';
    if (indexStatus === 'M') return 'staged';
    return 'staged';
  }
  
  if (workdirStatus && workdirStatus !== ' ') {
    if (workdirStatus === 'M') return 'modified';
    if (workdirStatus === 'D') return 'deleted';
    return 'modified';
  }
  
  if (status === 'M') return 'modified';
  if (status === 'A') return 'added';
  if (status === 'D') return 'deleted';
  if (status === 'R') return 'renamed';
  
  return undefined;
}

function collectChildrenGitStatuses(node: FileSystemNode): Set<string> {
  const statuses = new Set<string>();
  
  if (!node.children || node.children.length === 0) {
    return statuses;
  }
  
  for (const child of node.children) {
    if (child.gitStatus) {
      statuses.add(child.gitStatus);
    }
    
    if (child.childrenGitStatuses) {
      child.childrenGitStatuses.forEach(status => statuses.add(status));
    }
  }
  
  return statuses;
}

function buildGitStatusMap(
  gitState: GitState | null
): Map<string, { status: string; gitStatus: ReturnType<typeof parseGitStatusFromBackend> }> {
  const gitStatusMap = new Map<string, { status: string; gitStatus: ReturnType<typeof parseGitStatusFromBackend> }>();
  
  if (!gitState || !gitState.isRepository) {
    return gitStatusMap;
  }
  
  gitState.staged?.forEach(file => {
    const status = parseGitStatusFromBackend(file.status, file.index_status, file.workdir_status);
    if (status) {
      gitStatusMap.set(file.path.replace(/\\/g, '/'), { status: file.status, gitStatus: status });
    }
  });
  
  gitState.unstaged?.forEach(file => {
    const status = parseGitStatusFromBackend(file.status, file.index_status, file.workdir_status);
    const normalizedPath = file.path.replace(/\\/g, '/');
    if (status && !gitStatusMap.has(normalizedPath)) {
      gitStatusMap.set(normalizedPath, { status: file.status, gitStatus: status });
    }
  });
  
  gitState.untracked?.forEach(filePath => {
    const normalizedPath = filePath.replace(/\\/g, '/');
    if (!gitStatusMap.has(normalizedPath)) {
      gitStatusMap.set(normalizedPath, { status: '??', gitStatus: 'untracked' });
    }
  });
  
  return gitStatusMap;
}

function getNodeStatusInfo(
  nodePath: string,
  workspacePath: string | undefined,
  gitStatusMap: Map<string, { status: string; gitStatus: ReturnType<typeof parseGitStatusFromBackend> }>
): { status: string; gitStatus: ReturnType<typeof parseGitStatusFromBackend> } | undefined {
  const normalizedNodePath = nodePath.replace(/\\/g, '/');

  const absoluteMatch = gitStatusMap.get(normalizedNodePath);
  if (absoluteMatch) return absoluteMatch;

  if (!workspacePath) return undefined;

  const normalizedWorkspacePath = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!normalizedNodePath.startsWith(`${normalizedWorkspacePath}/`)) {
    return undefined;
  }

  const relativePath = normalizedNodePath.slice(normalizedWorkspacePath.length + 1);
  return gitStatusMap.get(relativePath);
}

/**
 * Update file tree nodes with Git status. Stores complete Git info regardless of expansion state.
 */
function updateNodeGitStatus(
  nodes: FileSystemNode[],
  gitStatusMap: Map<string, { status: string; gitStatus: ReturnType<typeof parseGitStatusFromBackend> }>,
  workspacePath?: string
): FileSystemNode[] {
  return nodes.map(node => {
    let updatedNode = { ...node };
    const statusInfo = getNodeStatusInfo(node.path, workspacePath, gitStatusMap);
    const matched = !!statusInfo;
    
    if (statusInfo) {
      updatedNode = {
        ...updatedNode,
        gitStatus: statusInfo.gitStatus,
        gitStatusText: statusInfo.status
      };
    }
    
    if (node.children && node.children.length > 0) {
      updatedNode.children = updateNodeGitStatus(node.children, gitStatusMap, workspacePath);
      
      const childStatuses = collectChildrenGitStatuses(updatedNode);
      
      if (node.isDirectory) {
        updatedNode = {
          ...updatedNode,
          hasChildrenGitChanges: childStatuses.size > 0,
          childrenGitStatuses: childStatuses as any
        };
      }
    }
    
    if (!matched && !node.isDirectory && node.gitStatus) {
      updatedNode = {
        ...updatedNode,
        gitStatus: undefined,
        gitStatusText: undefined
      };
    }
    
    return updatedNode;
  });
}

export function useFileTreeGitSync({
  workspacePath,
  fileTree,
  onTreeUpdate,
  autoRefresh = true,
  debounceDelay = 300
}: UseFileTreeGitSyncProps) {
  const treeRef = useRef(fileTree);
  const onTreeUpdateRef = useRef(onTreeUpdate);
  const debounceTimerRef = useRef<NodeJS.Timeout | undefined>(undefined);
  const updateInProgressRef = useRef(false);
  
  const [gitState, setGitState] = useState<GitState | null>(() => 
    workspacePath ? gitStateManager.getState(workspacePath) : null
  );
  const gitStateRef = useRef(gitState);
  
  useEffect(() => {
    treeRef.current = fileTree;
  }, [fileTree]);
  
  useEffect(() => {
    onTreeUpdateRef.current = onTreeUpdate;
  }, [onTreeUpdate]);
  
  useEffect(() => {
    gitStateRef.current = gitState;
  }, [gitState]);
  
  const applyGitStatusToTreeRef = useRef<typeof applyGitStatusToTree | null>(null);
  
  const applyGitStatusToTree = useCallback((
    targetTree: FileSystemNode[],
    state: GitState | null,
    immediate: boolean = false
  ) => {
    const doApply = () => {
      if (updateInProgressRef.current) return;
      if (!targetTree || targetTree.length === 0) return;
      
      updateInProgressRef.current = true;
      
      try {
        const gitStatusMap = buildGitStatusMap(state);
        const updatedTree = updateNodeGitStatus(targetTree, gitStatusMap, workspacePath);
        
        onTreeUpdateRef.current(updatedTree);
        
        log.debug('Git status applied to file tree', {
          totalFiles: gitStatusMap.size,
          staged: state?.staged?.length || 0,
          unstaged: state?.unstaged?.length || 0,
          untracked: state?.untracked?.length || 0
        });
      } finally {
        updateInProgressRef.current = false;
      }
    };
    
    if (immediate) {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      doApply();
    } else {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(doApply, debounceDelay);
    }
  }, [debounceDelay, workspacePath]);
  
  const applyGitStatusImmediate = useCallback(() => {
    applyGitStatusToTree(treeRef.current, gitStateRef.current, true);
  }, [applyGitStatusToTree]);
  
  const refreshGitStatus = useCallback(async () => {
    if (!workspacePath) return;
    
    await gitStateManager.refresh(workspacePath, {
      layers: ['basic', 'status'],
      reason: 'manual',
      force: true,
    });
  }, [workspacePath]);
  
  useEffect(() => {
    applyGitStatusToTreeRef.current = applyGitStatusToTree;
  }, [applyGitStatusToTree]);

  const prevTreeLengthRef = useRef(fileTree.length);
  useEffect(() => {
    const prevLength = prevTreeLengthRef.current;
    const currentLength = fileTree.length;
    prevTreeLengthRef.current = currentLength;
    
    if (prevLength === 0 && currentLength > 0) {
      const currentGitState = gitStateRef.current;
      if (currentGitState?.isRepository) {
        applyGitStatusToTreeRef.current?.(fileTree, currentGitState, true);
      }
    }
  }, [fileTree]);

  useEffect(() => {
    if (!workspacePath) return;
    
    const normalizedPath = workspacePath.replace(/\\/g, '/');
    
    const unsubscribe = gitStateManager.subscribe(
      normalizedPath,
      (newState, _prevState, changedLayers) => {
        if (changedLayers.includes('status') || changedLayers.includes('basic')) {
          setGitState(newState);
          
          if (!newState.isRefreshing) {
            applyGitStatusToTreeRef.current(treeRef.current, newState, false);
          }
        }
      },
      {
        layers: ['basic', 'status'],
        immediate: true,
      }
    );
    
    return unsubscribe;
  }, [workspacePath]);
  
  useEffect(() => {
    if (!workspacePath || !autoRefresh) return;
    
    const handleFileTreeRefresh = () => {
      refreshGitStatus();
    };
    
    const handleSilentRefreshCompleted = (event: { path: string; fileTree: FileSystemNode[] }) => {
      const currentGitState = gitStateRef.current;
      if (currentGitState?.isRepository) {
        applyGitStatusToTreeRef.current(event.fileTree, currentGitState, true);
      }
      
      refreshGitStatus();
    };
    
    globalEventBus.on('file-tree:refresh', handleFileTreeRefresh);
    globalEventBus.on('file-tree:silent-refresh-completed', handleSilentRefreshCompleted);
    
    return () => {
      globalEventBus.off('file-tree:refresh', handleFileTreeRefresh);
      globalEventBus.off('file-tree:silent-refresh-completed', handleSilentRefreshCompleted);
    };
  }, [workspacePath, autoRefresh, refreshGitStatus]);
  
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, []);
  
  return {
    refresh: refreshGitStatus,
    applyImmediate: applyGitStatusImmediate,
    gitStatus: gitState,
    loading: gitState?.isRefreshing ?? false
  };
}
