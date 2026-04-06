import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import { dirnameAbsolutePath, expandedFoldersContains, pathsEquivalentFs } from '@/shared/utils/pathUtils';
import type { FileSystemChangeEvent, FileSystemNode } from '@/tools/file-system/types';
import { ExplorerModel } from '../model/ExplorerModel';
import { projectExplorerSnapshot } from '../projection/ExplorerViewProjector';
import { tauriExplorerFileSystemProvider } from '../provider/TauriExplorerFileSystemProvider';
import type { ExplorerControllerConfig, ExplorerFileSystemProvider, ExplorerSnapshot } from '../types/explorer';

const log = createLogger('ExplorerController');
const DEFAULT_POLL_INTERVAL_MS = 1000;
const DIRECTORY_PAGE_SIZE = 200;

function cloneConfig(config: ExplorerControllerConfig): ExplorerControllerConfig {
  return {
    ...config,
    excludePatterns: [...(config.excludePatterns ?? [])],
  };
}

function sameStringArray(left: string[] = [], right: string[] = []): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}

function didReloadRelevantOptionsChange(
  previous: ExplorerControllerConfig | null,
  current: ExplorerControllerConfig
): boolean {
  if (!previous) {
    return false;
  }

  return (
    previous.showHiddenFiles !== current.showHiddenFiles ||
    previous.sortBy !== current.sortBy ||
    previous.sortOrder !== current.sortOrder ||
    previous.maxDepth !== current.maxDepth ||
    !sameStringArray(previous.excludePatterns, current.excludePatterns)
  );
}

function sortNodes(
  nodes: FileSystemNode[],
  sortBy: 'name' | 'size' | 'lastModified' | 'type' = 'name',
  sortOrder: 'asc' | 'desc' = 'asc'
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

export class ExplorerController {
  private readonly provider: ExplorerFileSystemProvider;
  private readonly model = new ExplorerModel();
  private readonly listeners = new Set<() => void>();
  private cachedSnapshot?: ExplorerSnapshot;
  private config: ExplorerControllerConfig = {
    autoLoad: true,
    enableAutoWatch: true,
    enableLazyLoad: true,
    pollingIntervalMs: DEFAULT_POLL_INTERVAL_MS,
    enablePathCompression: true,
    showHiddenFiles: false,
    sortBy: 'name',
    sortOrder: 'asc',
    excludePatterns: [],
  };
  private lastAppliedConfig: ExplorerControllerConfig | null = null;
  private unwatch?: () => void;
  private pollId?: number;
  private pendingRefreshTimer?: ReturnType<typeof setTimeout>;
  private pendingRefreshPaths = new Set<string>();
  private generation = 0;
  private disposed = false;

  constructor(provider: ExplorerFileSystemProvider = tauriExplorerFileSystemProvider) {
    this.provider = provider;
    this.model.configure(this.config);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  getSnapshot(): ExplorerSnapshot {
    if (!this.cachedSnapshot) {
      this.cachedSnapshot = projectExplorerSnapshot(this.model.getSnapshot());
    }
    return this.cachedSnapshot;
  }

  async configure(config: ExplorerControllerConfig): Promise<void> {
    const nextConfig = cloneConfig({
      ...this.config,
      ...config,
    });
    const rootChanged = this.config.rootPath !== nextConfig.rootPath;
    const optionsChanged = didReloadRelevantOptionsChange(this.lastAppliedConfig, nextConfig);
    this.config = nextConfig;
    this.model.configure(nextConfig);

    if (rootChanged) {
      this.resetForRoot(nextConfig.rootPath);
      this.lastAppliedConfig = cloneConfig(nextConfig);
      this.emit();
      if (nextConfig.autoLoad && nextConfig.rootPath) {
        if (nextConfig.enableLazyLoad) {
          await this.loadRootLazy(nextConfig.rootPath, false);
        } else {
          await this.loadRootTree(nextConfig.rootPath, false);
        }
      }
      return;
    }

    if (nextConfig.rootPath && optionsChanged && this.model.getSnapshot().fileTree.length > 0) {
      this.lastAppliedConfig = cloneConfig(nextConfig);
      if (nextConfig.enableLazyLoad) {
        await this.loadRootLazy(nextConfig.rootPath, true);
      } else {
        await this.loadRootTree(nextConfig.rootPath, true);
      }
      return;
    }

    this.lastAppliedConfig = cloneConfig(nextConfig);
    this.syncWatchers();
  }

  async loadFileTree(path?: string, silent = false): Promise<void> {
    const targetPath = path ?? this.config.rootPath;
    if (!targetPath) {
      return;
    }
    await this.loadRootTree(targetPath, silent);
  }

  async loadFileTreeLazy(path?: string, silent = false): Promise<void> {
    const targetPath = path ?? this.config.rootPath;
    if (!targetPath) {
      return;
    }
    await this.loadRootLazy(targetPath, silent);
  }

  selectFile(filePath: string): void {
    this.model.select(filePath);
    this.emit();
  }

  replaceTree(fileTree: FileSystemNode[]): void {
    this.model.replaceTree(fileTree);
    this.emit();
  }

  expandFolder(folderPath: string, expanded?: boolean): void {
    const currentExpanded = expandedFoldersContains(this.model.getExpandedFolders(), folderPath);
    const nextExpanded = expanded ?? !currentExpanded;

    if (nextExpanded && this.config.enableLazyLoad) {
      const node = this.model.getNode(folderPath);
      const needsLazyLoad =
        node?.kind === 'directory' &&
        node.childrenState !== 'resolved' &&
        node.childrenState !== 'loading';

      if (needsLazyLoad) {
        void this.expandFolderLazy(folderPath);
        return;
      }
    }

    this.model.expand(folderPath, nextExpanded);
    this.emit();
  }

  async expandFolderLazy(folderPath: string): Promise<void> {
    const currentExpanded = expandedFoldersContains(this.model.getExpandedFolders(), folderPath);
    if (currentExpanded) {
      this.model.expand(folderPath, false);
      this.emit();
      return;
    }

    this.model.expand(folderPath, true);
    const node = this.model.getNode(folderPath);
    const alreadyResolved = node?.childrenState === 'resolved';
    this.emit();

    if (alreadyResolved) {
      return;
    }

    this.model.setDirectoryLoading(folderPath, true);
    this.emit();

    try {
      const page = await this.loadDirectoryChildren(folderPath);
      this.model.upsertChildren(folderPath, page.children, {
        append: false,
        totalChildren: page.total,
        hasMoreChildren: page.hasMore,
      });
      this.emit();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.model.markDirectoryError(folderPath, message);
      this.emit();
      log.error('Failed to expand directory lazily', { folderPath, error });
    }
  }

  async loadMoreFolder(folderPath: string): Promise<void> {
    const node = this.model.getNode(folderPath);
    if (!node || node.kind !== 'directory' || !node.hasMoreChildren) {
      return;
    }

    if (this.model.getSnapshot().loadingPaths.has(folderPath)) {
      return;
    }

    this.model.setDirectoryLoading(folderPath, true);
    this.emit();

    try {
      const page = await this.loadDirectoryChildrenPage(folderPath, node.childIds.length);
      this.model.upsertChildren(folderPath, page.children, {
        append: true,
        totalChildren: page.total,
        hasMoreChildren: page.hasMore,
      });
      this.emit();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.model.markDirectoryError(folderPath, message);
      this.emit();
      log.error('Failed to load more explorer children', { folderPath, error });
    }
  }

  dispose(): void {
    this.disposed = true;
    this.stopWatchers();
    if (this.pendingRefreshTimer) {
      clearTimeout(this.pendingRefreshTimer);
      this.pendingRefreshTimer = undefined;
    }
    this.listeners.clear();
  }

  private async loadRootTree(rootPath: string, silent: boolean): Promise<void> {
    const generation = ++this.generation;
    this.model.setRootPath(rootPath);
    this.model.clearTransientErrors();
    this.model.setLoading(true, silent);
    this.emit();

    try {
      const tree = await this.provider.getFileTree(rootPath, this.config);
      if (!this.isGenerationCurrent(generation, rootPath)) {
        return;
      }

      this.model.replaceTree(tree);
      this.syncWatchers();
      this.emit();

      if (silent) {
        globalEventBus.emit('file-tree:silent-refresh-completed', {
          path: rootPath,
          fileTree: tree,
        });
      }
    } catch (error) {
      if (!this.isGenerationCurrent(generation, rootPath)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      this.model.setError(message);
      this.emit();
      log.error('Failed to load explorer tree', { rootPath, error });
    }
  }

  private async loadRootLazy(rootPath: string, silent: boolean): Promise<void> {
    const generation = ++this.generation;
    this.model.setRootPath(rootPath);
    this.model.clearTransientErrors();
    this.model.setLoading(true, silent);
    this.emit();

    try {
      const page = await this.loadDirectoryChildren(rootPath);
      if (!this.isGenerationCurrent(generation, rootPath)) {
        return;
      }

      this.model.replaceRootChildren(
        rootPath,
        page.children,
        page.total,
        page.hasMore
      );
      this.syncWatchers();
      this.emit();

      if (silent) {
        globalEventBus.emit('file-tree:silent-refresh-completed', {
          path: rootPath,
          fileTree: this.model.getSnapshot().fileTree,
        });
      }
    } catch (error) {
      if (!this.isGenerationCurrent(generation, rootPath)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      this.model.setError(message);
      this.emit();
      log.error('Failed to load root children lazily', { rootPath, error });
    }
  }

  private async refreshDirectory(path: string): Promise<void> {
    try {
      const node = this.model.getNode(path);
      const targetCount =
        node?.kind === 'directory' && node.childIds.length > 0
          ? node.childIds.length
          : DIRECTORY_PAGE_SIZE;
      const page = await this.loadDirectoryChildren(path, targetCount);

      this.model.upsertChildren(path, page.children, {
        append: false,
        totalChildren: page.total,
        hasMoreChildren: page.hasMore,
      });
      this.emit();
    } catch (error) {
      log.warn('Failed to refresh explorer directory', { path, error });
    }
  }

  private async loadDirectoryChildren(
    path: string,
    targetCount: number = DIRECTORY_PAGE_SIZE
  ): Promise<{
    children: FileSystemNode[];
    total: number;
    hasMore: boolean;
  }> {
    const allChildren: FileSystemNode[] = [];
    let offset = 0;
    let total = 0;

    while (allChildren.length < targetCount) {
      const page = await this.loadDirectoryChildrenPage(path, offset);
      allChildren.push(...page.children);
      total = page.total;

      if (!page.hasMore || page.children.length === 0) {
        break;
      }

      offset = page.offset + page.limit;
    }

    return {
      children: sortNodes(
        allChildren.slice(0, targetCount),
        this.config.sortBy ?? 'name',
        this.config.sortOrder ?? 'asc'
      ),
      total,
      hasMore: total > Math.min(allChildren.length, targetCount),
    };
  }

  private async loadDirectoryChildrenPage(
    path: string,
    offset: number
  ): Promise<{
    children: FileSystemNode[];
    total: number;
    hasMore: boolean;
    offset: number;
    limit: number;
  }> {
    return this.provider.getChildrenPage({
      path,
      offset,
      limit: DIRECTORY_PAGE_SIZE,
      options: this.config,
    });
  }

  private handleFileChange(event: FileSystemChangeEvent): void {
    const parentPath = dirnameAbsolutePath(event.path);

    this.pendingRefreshPaths.add(event.path);
    if (parentPath) {
      this.pendingRefreshPaths.add(parentPath);
    }
    if (event.oldPath) {
      const oldParent = dirnameAbsolutePath(event.oldPath);
      this.pendingRefreshPaths.add(event.oldPath);
      if (oldParent) {
        this.pendingRefreshPaths.add(oldParent);
      }
    }

    if (
      event.type === 'modified' ||
      event.type === 'created' ||
      event.type === 'renamed'
    ) {
      globalEventBus.emit('editor:file-changed', { filePath: event.path });
    }

    if (this.pendingRefreshTimer) {
      clearTimeout(this.pendingRefreshTimer);
    }

    this.pendingRefreshTimer = setTimeout(() => {
      const rootPath = this.config.rootPath;
      if (!rootPath) {
        return;
      }

      const refreshTargets = Array.from(this.pendingRefreshPaths);
      this.pendingRefreshPaths.clear();

      if (!this.config.enableLazyLoad) {
        void this.loadRootTree(rootPath, true);
        return;
      }

      const expandedFolders = this.model.getExpandedFolders();
      const directoriesToRefresh = new Set<string>();

      for (const target of refreshTargets) {
        const node = this.model.getNode(target);
        if (node?.kind === 'directory') {
          directoriesToRefresh.add(target);
        }

        const dirPath = dirnameAbsolutePath(target);
        if (dirPath && (dirPath === rootPath || expandedFoldersContains(expandedFolders, dirPath))) {
          directoriesToRefresh.add(dirPath);
        }
      }

      if (directoriesToRefresh.size === 0) {
        directoriesToRefresh.add(rootPath);
      }

      for (const directory of directoriesToRefresh) {
        void this.refreshDirectory(directory);
      }
    }, 200);
  }

  private syncWatchers(): void {
    this.stopWatchers();

    const rootPath = this.config.rootPath;
    if (!rootPath) {
      return;
    }

    if (this.config.enableAutoWatch) {
      this.unwatch = this.provider.watch(rootPath, (event) => this.handleFileChange(event));
    }

    const pollingIntervalMs = this.config.pollingIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;
    this.pollId = window.setInterval(() => {
      const currentRoot = this.config.rootPath;
      if (!currentRoot) {
        return;
      }

      if (!this.config.enableLazyLoad) {
        void this.loadRootTree(currentRoot, true);
        return;
      }

      void this.refreshDirectory(currentRoot);
      for (const expandedPath of this.model.getExpandedFolders()) {
        if (pathsEquivalentFs(expandedPath, currentRoot)) {
          continue;
        }
        void this.refreshDirectory(expandedPath);
      }
    }, pollingIntervalMs);
  }

  private stopWatchers(): void {
    this.unwatch?.();
    this.unwatch = undefined;

    if (this.pollId !== undefined) {
      window.clearInterval(this.pollId);
      this.pollId = undefined;
    }
  }

  private resetForRoot(rootPath?: string): void {
    this.stopWatchers();
    this.model.reset(rootPath);
  }

  private emit(): void {
    if (this.disposed) {
      return;
    }

    this.cachedSnapshot = undefined;

    for (const listener of this.listeners) {
      listener();
    }
  }

  private isGenerationCurrent(generation: number, rootPath: string): boolean {
    return generation === this.generation && this.config.rootPath === rootPath;
  }
}
