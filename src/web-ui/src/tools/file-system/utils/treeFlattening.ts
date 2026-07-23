import { FileSystemNode, FlatFileNode } from '../types';
import { expandedFoldersContains } from '@/shared/utils/pathUtils';

function nodeToFlatNode(
  node: FileSystemNode,
  parentPath: string | null,
  depth: number,
  childrenLoaded: boolean
): FlatFileNode {
  return {
    path: node.path,
    name: node.name,
    parentPath,
    isDirectory: node.isDirectory,
    depth,
    childrenLoaded,
    isLoading: false,
    size: node.size,
    extension: node.extension,
    lastModified: node.lastModified,
    isCompressed: node.isCompressed,
    originalNode: node,
  };
}

export function flattenFileTree(
  nodes: FileSystemNode[],
  expandedFolders: Set<string>,
  loadingPaths: Set<string> = new Set(),
  parentPath: string | null = null,
  depth: number = 0
): FlatFileNode[] {
  const result: FlatFileNode[] = [];

  for (const node of nodes) {
    const isExpanded = expandedFoldersContains(expandedFolders, node.path);
    const hasChildren = node.children && node.children.length > 0;
    const childrenLoaded = node.isDirectory ? (node.children !== undefined) : true;

    result.push({
      ...nodeToFlatNode(node, parentPath, depth, childrenLoaded),
      isLoading: loadingPaths.has(node.path),
    });

    if (node.isDirectory && isExpanded && hasChildren) {
      const childNodes = flattenFileTree(
        node.children!,
        expandedFolders,
        loadingPaths,
        node.path,
        depth + 1
      );
      result.push(...childNodes);
    }

  }

  return result;
}

export function countVisibleNodes(
  nodes: FileSystemNode[],
  expandedFolders: Set<string>
): number {
  let count = 0;

  for (const node of nodes) {
    count++;
    if (node.isDirectory && expandedFoldersContains(expandedFolders, node.path) && node.children) {
      count += countVisibleNodes(node.children, expandedFolders);
    }
  }

  return count;
}

export function findNodeIndex(flatNodes: FlatFileNode[], path: string): number {
  return flatNodes.findIndex(node => node.path === path);
}

export function getAncestorPaths(path: string, workspacePath?: string): string[] {
  const ancestors: string[] = [];
  
  const normalizedPath = path.replace(/\\/g, '/');
  const normalizedWorkspace = workspacePath?.replace(/\\/g, '/') || '';
  
  let currentPath = normalizedPath;
  
  while (currentPath && currentPath !== normalizedWorkspace) {
    const lastSlash = currentPath.lastIndexOf('/');
    if (lastSlash <= 0) break;
    
    currentPath = currentPath.substring(0, lastSlash);
    if (currentPath && currentPath !== normalizedWorkspace) {
      ancestors.push(currentPath);
    }
  }
  
  return ancestors.reverse();
}

/**
 * Update a node's children in the tree and return a new structure.
 */
export function updateNodeChildren(
  nodes: FileSystemNode[],
  targetPath: string,
  children: FileSystemNode[]
): FileSystemNode[] {
  return nodes.map(node => {
    if (node.path === targetPath) {
      return {
        ...node,
        children: children,
      };
    }
    
    if (node.children && node.path !== targetPath) {
      const updatedChildren = updateNodeChildren(node.children, targetPath, children);
      if (updatedChildren !== node.children) {
        return {
          ...node,
          children: updatedChildren,
        };
      }
    }
    
    return node;
  });
}

export function markNodeLoading(
  flatNodes: FlatFileNode[],
  path: string,
  isLoading: boolean
): FlatFileNode[] {
  return flatNodes.map(node => {
    if (node.path === path) {
      return { ...node, isLoading };
    }
    return node;
  });
}
