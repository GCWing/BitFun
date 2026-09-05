import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';

const BROWSER_DROP_REGISTRY_KEY = '__OPENBITFUN_BROWSER_DROP_FILES__';

type BrowserDropRegistryHost = typeof globalThis & {
  [BROWSER_DROP_REGISTRY_KEY]?: Map<string, readonly File[]>;
};

export type BrowserDroppedFilePathResolver = (
  token: string,
  fileCount: number,
) => Promise<string[]>;

function createDropToken(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  return `drop-${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}
/**
 * Keeps browser File wrappers alive just long enough for the Windows host to
 * resolve their original paths through WebView2. File contents are never read
 * or copied, and the transient registry entry is removed on every exit path.
 */
export async function resolveBrowserDroppedFilePaths(
  files: readonly File[],
  resolvePaths: BrowserDroppedFilePathResolver = (token, fileCount) => (
    workspaceAPI.resolveBrowserDroppedFilePaths(token, fileCount)
  ),
): Promise<string[]> {
  if (files.length === 0) return [];

  const host = globalThis as BrowserDropRegistryHost;
  const registry = host[BROWSER_DROP_REGISTRY_KEY] ?? new Map<string, readonly File[]>();
  host[BROWSER_DROP_REGISTRY_KEY] = registry;

  let token = createDropToken();
  while (registry.has(token)) token = createDropToken();
  registry.set(token, files);

  try {
    return await resolvePaths(token, files.length);
  } finally {
    registry.delete(token);
    if (registry.size === 0) delete host[BROWSER_DROP_REGISTRY_KEY];
  }
}
