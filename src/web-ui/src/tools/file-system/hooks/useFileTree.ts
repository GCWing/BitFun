import { useState, useCallback } from 'react';
import { FileSystemNode } from '../types';
import { lazyCompressFileTree, shouldCompressPaths } from '../utils/pathCompression';

export interface UseFileTreeOptions {
  enablePathCompression?: boolean;
  initialExpandedFolders?: string[];
}

export interface UseFileTreeReturn {
  expandedFolders: Set<string>;
  toggleFolder: (path: string) => void;
  expandFolder: (path: string) => void;
  collapseFolder: (path: string) => void;
  expandAll: (nodes: FileSystemNode[]) => void;
  collapseAll: () => void;
  processNodes: (nodes: FileSystemNode[]) => FileSystemNode[];
}

export function useFileTree(options: UseFileTreeOptions = {}): UseFileTreeReturn {
  const {
    enablePathCompression = true,
    initialExpandedFolders = []
  } = options;

  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set(initialExpandedFolders)
  );

  const toggleFolder = useCallback((path: string) => {
    setExpandedFolders(prev => {
      const newSet = new Set(prev);
      if (newSet.has(path)) {
        newSet.delete(path);
      } else {
        newSet.add(path);
      }
      return newSet;
    });
  }, []);

  const expandFolder = useCallback((path: string) => {
    setExpandedFolders(prev => new Set(prev).add(path));
  }, []);

  const collapseFolder = useCallback((path: string) => {
    setExpandedFolders(prev => {
      const newSet = new Set(prev);
      newSet.delete(path);
      return newSet;
    });
  }, []);

  const expandAll = useCallback((nodes: FileSystemNode[]) => {
    const allFolderPaths = new Set<string>();
    
    const collectFolderPaths = (nodeList: FileSystemNode[]) => {
      for (const node of nodeList) {
        if (node.isDirectory) {
          allFolderPaths.add(node.path);
          if (node.children) {
            collectFolderPaths(node.children);
          }
        }
      }
    };
    
    collectFolderPaths(nodes);
    setExpandedFolders(allFolderPaths);
  }, []);

  const collapseAll = useCallback(() => {
    setExpandedFolders(new Set());
  }, []);

  const processNodes = useCallback((nodes: FileSystemNode[]) => {
    if (!enablePathCompression || !shouldCompressPaths()) {
      return nodes;
    }
    return lazyCompressFileTree(nodes, expandedFolders);
  }, [enablePathCompression, expandedFolders]);

  return {
    expandedFolders,
    toggleFolder,
    expandFolder,
    collapseFolder,
    expandAll,
    collapseAll,
    processNodes
  };
}