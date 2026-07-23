import type { FileSystemNode } from '../types';

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').toLowerCase();
}

function findNodeInTree(nodes: FileSystemNode[], path: string): FileSystemNode | null {
  for (const node of nodes) {
    if (normalizePath(node.path) === normalizePath(path)) {
      return node;
    }
    if (node.children) {
      const found = findNodeInTree(node.children, path);
      if (found) return found;
    }
  }
  return null;
}

/**
 * Resolves the parent directory for "new file / new folder" actions
 * from the current selection (same rules as the file explorer toolbar).
 */
export function getNewItemParentPath(
  workspacePath: string | undefined,
  selectedFile: string | undefined,
  fileTree: FileSystemNode[]
): string {
  if (!workspacePath) {
    return '';
  }

  if (!selectedFile) {
    return workspacePath;
  }

  const selectedNode = findNodeInTree(fileTree, selectedFile);

  if (!selectedNode) {
    return workspacePath;
  }

  if (selectedNode.isDirectory) {
    return selectedNode.path;
  }

  const isWindows = selectedNode.path.includes('\\');
  const separator = isWindows ? '\\' : '/';

  const lastSeparatorIndex = selectedNode.path.lastIndexOf(separator);

  if (lastSeparatorIndex === -1) {
    return workspacePath;
  }

  const parentPath = selectedNode.path.substring(0, lastSeparatorIndex);

  return parentPath || workspacePath;
}
