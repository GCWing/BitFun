import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { workspaceAPI } from '@/infrastructure/api';
import type { ExplorerChildrenPageDto, ExplorerNodeDto } from '@/infrastructure/api/service-api/tauri-commands';
import { createLogger } from '@/shared/utils/logger';
import type { FileSystemChangeEvent, FileSystemNode, FileSystemOptions } from '@/tools/file-system/types';
import type { ExplorerChildrenPage, ExplorerChildrenRequest, ExplorerFileSystemProvider } from '../types/explorer';

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

function transformRawTree(rawNodes: ExplorerNodeDto[]): FileSystemNode[] {
  return rawNodes.map(node => transformRawNode(node));
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

  return sortedNodes.map(node => ({
    ...node,
    children: node.children ? sortNodes(node.children, sortBy, sortOrder) : undefined,
  }));
}

function normalizeForCompare(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
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
  async getFileTree(rootPath: string, options: FileSystemOptions = {}): Promise<FileSystemNode[]> {
    const rawTree = await workspaceAPI.explorerGetFileTree(rootPath, options.maxDepth);
    return sortNodes(transformRawTree(rawTree), options.sortBy, options.sortOrder);
  }

  async getChildren(request: ExplorerChildrenRequest): Promise<FileSystemNode[]> {
    const rawChildren = await workspaceAPI.explorerGetChildren(request.path);
    return sortNodes(
      rawChildren.map((node) => transformRawNode(node)),
      request.options?.sortBy,
      request.options?.sortOrder
    );
  }

  async getChildrenPage(request: ExplorerChildrenRequest): Promise<ExplorerChildrenPage> {
    const offset = request.offset ?? 0;
    const limit = request.limit ?? 100;
    const result: ExplorerChildrenPageDto = await workspaceAPI.explorerGetChildrenPaginated(
      request.path,
      offset,
      limit
    );
    return {
      children: sortNodes(
        result.children.map((node) => transformRawNode(node)),
        request.options?.sortBy,
        request.options?.sortOrder
      ),
      total: result.total,
      hasMore: result.hasMore,
      offset: result.offset,
      limit: result.limit,
    };
  }

  watch(rootPath: string, callback: (event: FileSystemChangeEvent) => void): () => void {
    let unlisten: UnlistenFn | null = null;
    let active = true;
    const normalizedRoot = normalizeForCompare(rootPath);

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
    };
  }
}

export const tauriExplorerFileSystemProvider = new TauriExplorerFileSystemProvider();
