import { useContextStore } from '@/shared/stores/contextStore';
import type { DirectoryContext, FileContext } from '@/shared/types/context';

export interface FileMentionTarget {
  path: string;
  name: string;
  isDirectory: boolean;
}

function newContextId(prefix: string): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }

  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

function getRelativePath(fullPath: string, workspacePath?: string): string | undefined {
  if (!workspacePath) {
    return undefined;
  }

  const normalizedWorkspace = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
  const normalizedPath = fullPath.replace(/\\/g, '/');

  if (!normalizedPath.toLowerCase().startsWith(normalizedWorkspace.toLowerCase())) {
    return undefined;
  }

  return normalizedPath.slice(normalizedWorkspace.length).replace(/^\//, '');
}

export function createFileMentionContext(
  target: FileMentionTarget,
  workspacePath?: string,
): FileContext | DirectoryContext {
  const timestamp = Date.now();

  if (target.isDirectory) {
    return {
      id: newContextId('dir'),
      type: 'directory',
      directoryPath: target.path,
      directoryName: target.name,
      recursive: true,
      timestamp,
    };
  }

  return {
    id: newContextId('file'),
    type: 'file',
    filePath: target.path,
    fileName: target.name,
    relativePath: getRelativePath(target.path, workspacePath),
    timestamp,
  };
}

export function addFileMentionToChat(
  target: FileMentionTarget,
  workspacePath?: string,
): FileContext | DirectoryContext {
  const context = createFileMentionContext(target, workspacePath);
  useContextStore.getState().addContext(context);
  window.dispatchEvent(new CustomEvent('insert-context-tag', { detail: { context } }));
  return context;
}
