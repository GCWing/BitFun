import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import { dirnameAbsolutePath, expandedFoldersContains, pathsEquivalentFs } from '@/shared/utils/pathUtils';
import type { FileSystemChangeEvent, FileSystemNode } from '@/tools/file-system/types';
import { ExplorerModel } from '../model/ExplorerModel';
import { projectExplorerSnapshot } from '../projection/ExplorerViewProjector';
import { tauriExplorerFileSystemProvider } from '../provider/TauriExplorerFileSystemProvider';
import type { ExplorerControllerConfig, ExplorerFileSystemProvider, ExplorerSnapshot } from '../types/explorer';

const log = createLogger('ExplorerController');

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

  return sortedNodes.map((node) => ({
    ...node,
    children: node.children ? sortNodes(node.children, sortBy, sortOrder) : undefined,
  }));
}

function comparePathDepth(left: string, right: string): number {
  const leftDepth = left.split(/[/\\]/).filter(Boolean).length;
  const rightDepth = right.split(/[/\\]/).filter(Boolean).length;
  return leftDepth - rightDepth;
}

export class ExplorerController {
  private readonly provider: ExplorerFileSystemProvider;
  private readonly model = new ExplorerModel();
  private readonly listeners = new Set<() => void>();
  private cachedSnapshot?: ExplorerSnapshot;
  private config: ExplorerControllerConfig = {
    autoLoad: true,
    enableAutoWatch: true,
    enablePathCompression: true,
    showHiddenFiles: false,
    sortBy: 'name',
    sortOrder: 'asc',
    excludePatterns: [],
  };
  private lastAppliedConfig: ExplorerControllerConfig | null = null;
  private unwatch?: () => void;
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

      if (nextConfig.rootPath) {
        this.syncWatchers();
        if (nextConfig.autoLoad) {
          await this.initializeRoot(nextConfig.rootPath);
        }
      }
      return;
    }

    if (nextConfig.rootPath && optionsChanged && this.model.getSnapshot().fileTree.length > 0) {
      this.lastAppliedConfig = cloneConfig(nextConfig);
      await this.refreshExplorer(true);
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

    if (!pathsEquivalentFs(targetPath, this.config.rootPath ?? '')) {
      await this.configure({ ...this.config, rootPath: targetPath });
      return;
    }

    const root = this.model.getNode(targetPath);
    if (!root || root.childrenState === 'unresolved') {
      await this.initializeRoot(targetPath);
      return;
    }

    await this.refreshExplorer(true, silent);
  }

  selectFile(filePath: string): void {
    this.model.select(filePath);
    this.emit();
  }

  expandFolder(folderPath: string, expanded?: boolean): void {
    const currentExpanded = expandedFoldersContains(this.model.getExpandedFolders(), folderPath);
    const nextExpanded = expanded ?? !currentExpanded;

    this.model.expand(folderPath, nextExpanded);
    this.emit();

    if (!nextExpanded) {
      return;
    }

    const node = this.model.getNode(folderPath);
    const needsResolve =
      node?.kind === 'directory' &&
      (node.childrenState === 'unresolved' || node.childrenState === 'error' || node.stale);

    if (needsResolve) {
      void this.resolveDirectory(folderPath, true);
    }
  }

  async expandFolderLazy(folderPath: string): Promise<void> {
    const currentExpanded = expandedFoldersContains(this.model.getExpandedFolders(), folderPath);
    if (currentExpanded) {
      this.model.expand(folderPath, false);
      this.emit();
      return;
    }

    this.model.expand(folderPath, true);
    this.emit();
    await this.resolveDirectory(folderPath, true);
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

  private async initializeRoot(rootPath: string): Promise<void> {
    if (!this.config.rootPath || !pathsEquivalentFs(rootPath, this.config.rootPath)) {
      return;
    }

    const expectedGeneration = this.generation;
    this.model.ensureRoot(rootPath);
    this.model.clearTransientErrors();
    this.model.setLoading(true);
    this.emit();

    try {
      await this.resolveDirectory(rootPath, true, expectedGeneration, rootPath);
      if (!this.isGenerationCurrent(expectedGeneration, rootPath)) {
        return;
      }
      this.model.setLoading(false);
      this.emit();
    } catch (error) {
      if (!this.isGenerationCurrent(expectedGeneration, rootPath)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      this.model.setError(message);
      this.emit();
      log.error('Failed to initialize explorer root', { rootPath, error });
    }
  }

  private async refreshExplorer(includeExpandedSubtree: boolean, silent = true): Promise<void> {
    const rootPath = this.config.rootPath;
    if (!rootPath) {
      return;
    }

    const expectedGeneration = this.generation;
    this.model.clearTransientErrors();
    this.model.markVisibleSubtreeStale(rootPath);
    this.emit();

    const directoriesToRefresh = this.getDirectoriesToRefresh(rootPath, includeExpandedSubtree);
    for (const directory of directoriesToRefresh) {
      if (!this.isGenerationCurrent(expectedGeneration, rootPath)) {
        return;
      }
      await this.resolveDirectory(directory, true, expectedGeneration, rootPath);
    }

    if (silent) {
      globalEventBus.emit('file-tree:silent-refresh-completed', {
        path: rootPath,
        fileTree: this.model.getSnapshot().fileTree,
      });
    }
  }

  private getDirectoriesToRefresh(rootPath: string, includeExpandedSubtree: boolean): string[] {
    if (!includeExpandedSubtree) {
      return [rootPath];
    }

    const directories = new Set<string>([rootPath]);
    for (const expandedPath of this.model.getExpandedFolders()) {
      if (pathsEquivalentFs(expandedPath, rootPath)) {
        continue;
      }

      const node = this.model.getNode(expandedPath);
      if (node?.kind === 'directory') {
        directories.add(expandedPath);
      }
    }

    return Array.from(directories).sort(comparePathDepth);
  }

  private async resolveDirectory(
    path: string,
    forceRefresh = false,
    expectedGeneration = this.generation,
    expectedRootPath = this.config.rootPath ?? ''
  ): Promise<void> {
    const node = this.model.getNode(path);
    if (!node || node.kind !== 'directory') {
      return;
    }

    if (node.childrenState === 'refreshing') {
      return;
    }

    const shouldResolve =
      forceRefresh ||
      node.childrenState === 'unresolved' ||
      node.childrenState === 'error' ||
      node.stale;

    if (!shouldResolve) {
      return;
    }

    this.model.setDirectoryRefreshing(path, true);
    this.emit();

    try {
      const children = await this.provider.getChildren({
        path,
        options: this.config,
      });

      if (!this.isGenerationCurrent(expectedGeneration, expectedRootPath)) {
        return;
      }

      this.model.upsertChildren(
        path,
        sortNodes(children, this.config.sortBy ?? 'name', this.config.sortOrder ?? 'asc')
      );
      this.emit();
    } catch (error) {
      if (!this.isGenerationCurrent(expectedGeneration, expectedRootPath)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      this.model.markDirectoryError(path, message);
      if (pathsEquivalentFs(path, this.config.rootPath ?? '')) {
        this.model.setError(message);
      }
      this.emit();
      log.error('Failed to resolve explorer directory', { path, error });
      throw error;
    }
  }

  private handleFileChange(event: FileSystemChangeEvent): void {
    const parentPath = dirnameAbsolutePath(event.path);
    const changedDirectory = this.model.getNode(event.path);

    if (parentPath) {
      this.pendingRefreshPaths.add(parentPath);
      this.model.markDirectoryStale(parentPath);
    }

    if (changedDirectory?.kind === 'directory') {
      this.pendingRefreshPaths.add(event.path);
      this.model.markDirectoryStale(event.path);
    }

    if (event.oldPath) {
      const oldParent = dirnameAbsolutePath(event.oldPath);
      if (oldParent) {
        this.pendingRefreshPaths.add(oldParent);
        this.model.markDirectoryStale(oldParent);
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
      void this.flushPendingRefreshes();
    }, 200);

    this.emit();
  }

  private async flushPendingRefreshes(): Promise<void> {
    const rootPath = this.config.rootPath;
    if (!rootPath) {
      return;
    }

    const refreshTargets = Array.from(this.pendingRefreshPaths);
    this.pendingRefreshPaths.clear();

    if (refreshTargets.length === 0) {
      return;
    }

    const directoriesToRefresh = new Set<string>();
    const expandedFolders = this.model.getExpandedFolders();

    for (const directory of refreshTargets) {
      const isRootEquivalent = pathsEquivalentFs(directory, rootPath);
      const isExpanded = expandedFoldersContains(expandedFolders, directory);
      if (isRootEquivalent || isExpanded) {
        directoriesToRefresh.add(directory);
      }
    }

    if (directoriesToRefresh.size === 0) {
      return;
    }

    for (const directory of Array.from(directoriesToRefresh).sort(comparePathDepth)) {
      await this.resolveDirectory(directory, true);
    }
  }

  private syncWatchers(): void {
    this.stopWatchers();

    const rootPath = this.config.rootPath;
    if (!rootPath || !this.config.enableAutoWatch) {
      return;
    }

    this.unwatch = this.provider.watch(rootPath, (event) => this.handleFileChange(event));
  }

  private stopWatchers(): void {
    this.unwatch?.();
    this.unwatch = undefined;
  }

  private resetForRoot(rootPath?: string): void {
    this.stopWatchers();
    this.generation += 1;
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
    return generation === this.generation && pathsEquivalentFs(this.config.rootPath ?? '', rootPath);
  }
}
