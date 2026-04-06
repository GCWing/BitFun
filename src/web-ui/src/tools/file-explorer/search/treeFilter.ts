import type { FileSystemNode } from '@/tools/file-system/types';

export function filterTreeBySearch(
  nodes: FileSystemNode[],
  query: string
): FileSystemNode[] {
  const result: FileSystemNode[] = [];

  for (const node of nodes) {
    if (node.name.toLowerCase().includes(query)) {
      result.push(node);
      continue;
    }

    if (!node.isDirectory || !node.children) {
      continue;
    }

    const filteredChildren = filterTreeBySearch(node.children, query);
    if (filteredChildren.length > 0) {
      result.push({
        ...node,
        children: filteredChildren,
      });
    }
  }

  return result;
}

export function filterTreeByPredicate(
  nodes: FileSystemNode[],
  predicate: (node: FileSystemNode) => boolean
): FileSystemNode[] {
  const result: FileSystemNode[] = [];

  for (const node of nodes) {
    const filteredChildren =
      node.isDirectory && node.children
        ? filterTreeByPredicate(node.children, predicate)
        : [];

    if (predicate(node)) {
      result.push(
        filteredChildren.length > 0
          ? {
              ...node,
              children: filteredChildren,
            }
          : node
      );
      continue;
    }

    if (filteredChildren.length > 0) {
      result.push({
        ...node,
        children: filteredChildren,
      });
    }
  }

  return result;
}
