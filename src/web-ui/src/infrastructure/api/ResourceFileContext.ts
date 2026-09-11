import { createContext, useContext } from 'react';

/** Resource-owned I/O for nested renderers, including Markdown embedded assets. */
export interface ResourceFileAccess {
  scope: { surfaceId: string; remoteConnectionId?: string; workspacePath?: string; workspaceId?: string };
  files: { readFileContent: (path: string, encoding?: string) => Promise<string> };
}
export const ResourceFileContext = createContext<ResourceFileAccess | null>(null);
export const useResourceFileAccess = () => useContext(ResourceFileContext);
