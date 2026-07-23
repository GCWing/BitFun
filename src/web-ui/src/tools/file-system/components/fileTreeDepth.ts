export function getPathDepth(nodePath: string, workspacePath?: string): number {
  if (!workspacePath) {
    const normalized = nodePath.replace(/\\/g, '/');
    return normalized.split('/').filter(Boolean).length - 1;
  }

  const normalizedNode = nodePath.replace(/\\/g, '/').toLowerCase();
  const normalizedWorkspace = workspacePath.replace(/\\/g, '/').toLowerCase();

  let relativePath = normalizedNode;
  if (normalizedNode.startsWith(normalizedWorkspace)) {
    relativePath = normalizedNode.slice(normalizedWorkspace.length);
  }

  const segments = relativePath.replace(/^\//, '').split('/').filter(Boolean);
  return segments.length;
}
