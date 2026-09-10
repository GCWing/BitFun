import { repositoryPathKey } from '@/shared/utils/pathUtils';
import type { FlatFileNode } from '../types';

/** Plan ancestor expansion using the target filesystem's path semantics. */
export function planFileTreeReveal(root: string, target: string, remote = false): {
  pathsToExpand: string[]; targetPath: string;
} | null {
  if (!root || !target) return null;
  const windows = !remote && (/^[a-z]:[/\\]/i.test(root) || root.startsWith('\\\\'));
  const normalize = (path: string) => (windows ? path.replace(/\\/g, '/') : path).replace(/\/+$/, '') || '/';
  const normalizedRoot = normalize(root);
  const normalizedTarget = normalize(target);
  const compare = (path: string) => windows ? path.toLowerCase() : path;
  const prefix = normalizedRoot === '/' ? '/' : `${normalizedRoot}/`;
  if (compare(normalizedTarget) !== compare(normalizedRoot)
    && !compare(normalizedTarget).startsWith(compare(prefix))) return null;

  const parts = normalizedTarget.slice(normalizedRoot === '/' ? 1 : normalizedRoot.length).split('/').filter(Boolean);
  // Paths from a host are absolute. Do not let an unnormalized path escape the tree.
  if (parts.some(part => part === '.' || part === '..')) return null;
  const native = (path: string) => {
    const absolute = windows && /^[a-z]:$/i.test(path) ? `${path}/` : path;
    return windows && root.includes('\\') ? absolute.replace(/\//g, '\\') : absolute;
  };
  let currentPath = normalizedRoot;
  const pathsToExpand = parts.map(part => {
    currentPath = `${currentPath === '/' ? '' : currentPath}/${part}`;
    return native(currentPath);
  });
  return { pathsToExpand, targetPath: native(currentPath) };
}

/** Address the data row, including compressed folders, rather than rendered DOM. */
export function findFileTreeRevealIndex(nodes: FlatFileNode[], target: string): number {
  const key = repositoryPathKey(target);
  return nodes.findIndex(node => repositoryPathKey(node.path) === key
    || node.originalNode?.originalNodes?.some(original => repositoryPathKey(original.path) === key));
}
