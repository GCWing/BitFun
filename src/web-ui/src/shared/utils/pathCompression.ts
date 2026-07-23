 

import { FileTreeNode } from '../types/project-view';
import { i18nService } from '@/infrastructure/i18n';

export interface CompressedNode extends Omit<FileTreeNode, 'children'> {
  children?: CompressedNode[];
  isCompressed?: boolean;
  compressedPath?: string; 
  originalNodes?: FileTreeNode[]; 
}

 
function canCompress(node: FileTreeNode): boolean {
  return (
    node.isDirectory &&
    !!node.children &&
    node.children.length === 1 &&
    node.children[0].isDirectory
  );
}

 
function compressNodePath(node: FileTreeNode): CompressedNode {
  if (!canCompress(node)) {
    
    const compressedNode: CompressedNode = {
      ...node,
      children: node.children?.map(child => compressNodePath(child))
    };
    return compressedNode;
  }

  
  const pathSegments: string[] = [node.name];
  const originalNodes: FileTreeNode[] = [node];
  let currentNode = node;

  
  while (canCompress(currentNode)) {
    const childNode = currentNode.children![0];
    pathSegments.push(childNode.name);
    originalNodes.push(childNode);
    currentNode = childNode;
  }

  
  const compressedNode: CompressedNode = {
    ...node, 
    name: pathSegments.join('/'), 
    path: currentNode.path, 
    isCompressed: true,
    compressedPath: pathSegments.join('/'),
    originalNodes,
    
    children: currentNode.children?.map(child => compressNodePath(child))
  };

  return compressedNode;
}

 
export function compressFileTree(fileTree: FileTreeNode[]): CompressedNode[] {
  return fileTree.map(node => compressNodePath(node));
}

 
export function lazyCompressFileTree(fileTree: FileTreeNode[], expandedFolders: Set<string>): CompressedNode[] {
  return fileTree.map(node => lazyCompressNodePath(node, expandedFolders));
}

 
function lazyCompressNodePath(node: FileTreeNode, expandedFolders: Set<string>): CompressedNode {
  
  if (!expandedFolders.has(node.path)) {
    return {
      ...node,
      children: node.children?.map(child => ({
        ...child,
        children: child.children 
      }))
    };
  }

  
  if (!node.children || node.children.length === 0) {
    return { ...node };
  }

  
  const compressedChildren = node.children.map(child => {
    if (child.isDirectory && canCompress(child)) {
      
      return compressNodePath(child);
    } else {
      
      return lazyCompressNodePath(child, expandedFolders);
    }
  });

  return {
    ...node,
    children: compressedChildren
  };
}

 
export function expandCompressedNode(compressedNode: CompressedNode): FileTreeNode[] {
  if (!compressedNode.isCompressed || !compressedNode.originalNodes) {
    
    return [{
      ...compressedNode,
      children: compressedNode.children?.map(child => expandCompressedNode(child)).flat()
    }];
  }

  
  const nodes = [...compressedNode.originalNodes];
  
  
  for (let i = nodes.length - 1; i >= 0; i--) {
    const currentNode = nodes[i];
    
    if (i === nodes.length - 1) {
      
      currentNode.children = compressedNode.children?.map(child => expandCompressedNode(child)).flat();
    } else {
      
      currentNode.children = [nodes[i + 1]];
    }
  }

  return [nodes[0]];
}

 
export function shouldCompressPaths(options?: {
  enabled?: boolean;
  minDepth?: number;
  maxCompressedSegments?: number;
}): boolean {
  const defaultOptions = {
    enabled: true,
    minDepth: 2, 
    maxCompressedSegments: 5 
  };
  
  const finalOptions = { ...defaultOptions, ...options };
  return finalOptions.enabled ?? true;
}

 
export function getCompressionTooltip(compressedNode: CompressedNode): string {
  if (!compressedNode.isCompressed || !compressedNode.originalNodes) {
    return compressedNode.path;
  }
  
  const segments = compressedNode.originalNodes.map(node => node.name);
  return i18nService.t('common:file.compressedPathTooltip', {
    compressed: segments.join(' > '),
    full: compressedNode.path
  });
}
