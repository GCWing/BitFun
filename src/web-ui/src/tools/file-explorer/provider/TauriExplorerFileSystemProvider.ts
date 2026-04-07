import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { workspaceAPI } from '@/infrastructure/api';
import type { ExplorerNodeDto } from '@/infrastructure/api/service-api/tauri-commands';
import { createLogger } from '@/shared/utils/logger';
import type { FileSystemChangeEvent, FileSystemNode, FileSystemOptions } from '@/tools/file-system/types';
import type { ExplorerChildrenRequest, ExplorerFileSystemProvider } from '../types/explorer';

const log = createLogger('TauriExplorerProvider');

interface FileWatchEvent {
  path: string;
  kind: string;
  timestamp: number;
  from?: string;
}

function transformRawNode(rawNode: ExplorerNodeDto): FileSystemNode {
  const node: FileSystemNode = {
    path: rawNode.path,
    name: rawNode.name,
    isDirectory: rawNode.isDirectory,
    size: rawNode.size ?? undefined,
    extension: rawNode.extension ?? undefined,
    lastModified: rawNode.lastModified ? new Date(rawNode.lastModified) : undefined,
  };

  if (Array.isArray(rawNode.children)) {
    node.children = rawNode.children.map((child) => transformRawNode(child));
  }

  return node;
}

function sortNodes(
  nodes: FileSystemNode[],
  sortBy: FileSystemOptions['sortBy'] = 'name',
  sortOrder: FileSystemOptions['sortOrder'] = 'asc'
): FileSystemNode[] {
  const sortedNodes = [...nodes].sort((left, right) => {
    if (left.isDirectory && !right.isDirectory) return -1;
    if (!left.isDirectory && right.isDirectory) return 1;

    let comparison = 0;

    switch (sortBy) {
      case 'size':
        comparison = (left.size || 0) - (right.size || 0);
        break;
      case 'lastModified':
        comparison = (left.lastModified?.getTime() || 0) - (right.lastModified?.getTime() || 0);
        break;
      case 'type':
        comparison = (left.extension || '').localeCompare(right.extension || '');
        break;
      case 'name':
      default:
        comparison = left.name.localeCompare(right.name, 'zh-CN', { numeric: true });
        break;
    }

    return sortOrder === 'desc' ? -comparison : comparison;
  });

  return sortedNodes.map((node) => ({
    ...node,
    children: node.children ? sortNodes(node.children, sortBy, sortOrder) : undefined,
  }));
}

function normalizeForCompare(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

interface BackendWatchRef {
  count: number;
  rootPath: string;
  started: boolean;
}

const backendWatchRefs = new Map<string, BackendWatchRef>();

function toBackendWatchKey(path: string): string {
  const normalized = normalizeForCompare(path);
  const isWindowsLike = /^[a-zA-Z]:/.test(normalized) || normalized.startsWith('//');
  return isWindowsLike ? normalized.toLowerCase() : normalized;
}

function retainBackendWatch(rootPath: string): string {
  const key = toBackendWatchKey(rootPath);
  const existing = backendWatchRefs.get(key);
  if (existing) {
    existing.count += 1;
    return key;
  }

  backendWatchRefs.set(key, {
    count: 1,
    rootPath,
    started: false,
  });

  void workspaceAPI
    .startFileWatch(rootPath, true)
    .then(() => {
      const current = backendWatchRefs.get(key);
      if (!current) {
        void workspaceAPI.stopFileWatch(rootPath).catch(() => {});
        return;
      }

      current.started = true;
    })
    .catch((error) => {
      log.warn('Failed to register backend file watch', { rootPath, error });
    });

  return key;
}

function releaseBackendWatch(key: string): void {
  const existing = backendWatchRefs.get(key);
  if (!existing) {
    return;
  }

  existing.count -= 1;
  if (existing.count > 0) {
    return;
  }

  backendWatchRefs.delete(key);
  if (!existing.started) {
    return;
  }

  void workspaceAPI.stopFileWatch(existing.rootPath).catch((error) => {
    log.warn('Failed to unregister backend file watch', { rootPath: existing.rootPath, error });
  });
}

function mapEventKind(kind: string): FileSystemChangeEvent['type'] {
  switch (kind) {
    case 'create':
      return 'created';
    case 'modify':
      return 'modified';
    case 'remove':
      return 'deleted';
    case 'rename':
      return 'renamed';
    default:
      return 'modified';
  }
}

export class TauriExplorerFileSystemProvider implements ExplorerFileSystemProvider {
  async getChildren(request: ExplorerChildrenRequest): Promise<FileSystemNode[]> {
    const rawChildren = await workspaceAPI.explorerGetChildren(request.path);
    return sortNodes(
      rawChildren.map((node) => transformRawNode(node)),
      request.options?.sortBy,
      request.options?.sortOrder
    );
  }

  watch(rootPath: string, callback: (event: FileSystemChangeEvent) => void): () => void {
    let unlisten: UnlistenFn | null = null;
    let active = true;
    const normalizedRoot = normalizeForCompare(rootPath);
    const backendWatchKey = retainBackendWatch(rootPath);

    const start = async () => {
      try {
        unlisten = await listen<FileWatchEvent[]>('file-system-changed', (event) => {
          if (!active) {
            return;
          }

          const isUnderRoot = (targetPath: string) =>
            targetPath === normalizedRoot || targetPath.startsWith(`${normalizedRoot}/`);

          for (const fileEvent of event.payload) {
            const normalizedPath = normalizeForCompare(fileEvent.path);
            const normalizedFrom = fileEvent.from ? normalizeForCompare(fileEvent.from) : '';
            const relevant =
              isUnderRoot(normalizedPath) ||
              (fileEvent.kind === 'rename' && normalizedFrom !== '' && isUnderRoot(normalizedFrom));

            if (!relevant) {
              continue;
            }

            callback({
              type: mapEventKind(fileEvent.kind),
              path: fileEvent.path,
              oldPath: fileEvent.from,
              timestamp: new Date(fileEvent.timestamp * 1000),
            });
          }
        });
      } catch (error) {
        log.error('Failed to start explorer file watcher', { rootPath, error });
      }
    };

    void start();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
      releaseBackendWatch(backendWatchKey);
    };
  }
}

export const tauriExplorerFileSystemProvider = new TauriExplorerFileSystemProvider();
