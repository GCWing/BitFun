import {
  expandedFoldersAddEquivalent,
  expandedFoldersDeleteEquivalent,
} from '@/shared/utils/pathUtils';
import type { FileSystemNode, FileSystemOptions } from '@/tools/file-system/types';
import type { ExplorerControllerConfig, ExplorerNodeRecord, ExplorerSnapshot } from '../types/explorer';

const DEFAULT_OPTIONS: FileSystemOptions = {
  enablePathCompression: true,
  showHiddenFiles: false,
  sortBy: 'name',
  sortOrder: 'asc',
  maxDepth: undefined,
  excludePatterns: [],
};

function cloneOptions(options: FileSystemOptions): FileSystemOptions {
  return {
    ...DEFAULT_OPTIONS,
    ...options,
    excludePatterns: [...(options.excludePatterns ?? [])],
  };
}

function createDirectoryRecord(
  path: string,
  name: string,
  parentId: string | null,
  isRoot: boolean
): ExplorerNodeRecord {
  return {
    id: path,
    path,
    name,
    parentId,
    kind: 'directory',
    childIds: [],
    childrenState: 'unresolved',
    stale: false,
    isRoot,
  };
}

function createNodeRecord(
  node: FileSystemNode,
  parentId: string | null,
  isRoot: boolean,
  existing?: ExplorerNodeRecord
): ExplorerNodeRecord {
  return {
    id: node.path,
    path: node.path,
    name: node.name,
    parentId,
    kind: node.isDirectory ? 'directory' : 'file',
    size: node.size,
    extension: node.extension,
    lastModified: node.lastModified,
    childIds: node.children?.map((child) => child.path) ?? (existing?.childIds ?? []),
    childrenState: node.isDirectory
      ? (node.children ? 'resolved' : existing?.childrenState ?? 'unresolved')
      : 'resolved',
    stale: node.isDirectory ? (existing?.stale ?? false) : false,
    errorMessage: existing?.errorMessage,
    isRoot,
  };
}

export class ExplorerModel {
  private rootPath?: string;
  private readonly roots: string[] = [];
  private readonly nodes = new Map<string, ExplorerNodeRecord>();
  private readonly expandedFolders = new Set<string>();
  private readonly loadingPaths = new Set<string>();
  private selectedFile?: string;
  private loading = false;
  private error?: string;
  private options: FileSystemOptions = cloneOptions(DEFAULT_OPTIONS);

  configure(config: ExplorerControllerConfig): void {
    this.options = cloneOptions(config);
  }

  reset(rootPath?: string): void {
    this.rootPath = rootPath;
    this.roots.length = 0;
    this.nodes.clear();
    this.expandedFolders.clear();
    this.loadingPaths.clear();
    this.selectedFile = undefined;
    this.loading = false;
    this.error = undefined;

    if (rootPath) {
      this.bootstrapRoot(rootPath);
    }
  }

  setLoading(loading: boolean): void {
    this.loading = loading;
  }

  setError(error?: string): void {
    this.error = error;
    if (error) {
      this.loading = false;
    }
  }

  clearTransientErrors(): void {
    this.error = undefined;
  }

  select(filePath?: string): void {
    this.selectedFile = filePath;
  }

  expand(path: string, expanded = true): void {
    if (expanded) {
      this.replaceExpandedFolders(expandedFoldersAddEquivalent(this.expandedFolders, path));
      return;
    }

    this.replaceExpandedFolders(expandedFoldersDeleteEquivalent(this.expandedFolders, path));
  }

  ensureRoot(rootPath: string): void {
    if (this.rootPath !== rootPath) {
      this.reset(rootPath);
      return;
    }

    if (!this.nodes.has(rootPath)) {
      this.bootstrapRoot(rootPath);
    }
  }

  setDirectoryRefreshing(path: string, refreshing: boolean): void {
    const node = this.nodes.get(path);
    if (!node || node.kind !== 'directory') {
      return;
    }

    if (refreshing) {
      this.loadingPaths.add(path);
      node.childrenState = 'refreshing';
      node.stale = false;
      node.errorMessage = undefined;
      return;
    }

    this.loadingPaths.delete(path);
    if (node.childrenState === 'refreshing') {
      node.childrenState = 'resolved';
    }
  }

  markDirectoryStale(path: string): void {
    const node = this.nodes.get(path);
    if (!node || node.kind !== 'directory') {
      return;
    }

    node.stale = true;
    if (node.childrenState === 'resolved') {
      node.errorMessage = undefined;
    }
  }

  markVisibleSubtreeStale(rootPath: string): void {
    this.markDirectoryStale(rootPath);

    for (const path of this.expandedFolders) {
      if (path === rootPath) {
        continue;
      }
      const node = this.nodes.get(path);
      if (node?.kind === 'directory') {
        node.stale = true;
      }
    }
  }

  upsertChildren(parentPath: string, children: FileSystemNode[]): void {
    const parent = this.nodes.get(parentPath);
    if (!parent || parent.kind !== 'directory') {
      return;
    }

    const previousChildIds = new Set(parent.childIds);
    const nextChildIds: string[] = [];

    for (const child of children) {
      const existing = this.nodes.get(child.path);
      const nextRecord = createNodeRecord(child, parentPath, false, existing);
      this.nodes.set(child.path, nextRecord);
      nextChildIds.push(child.path);
      previousChildIds.delete(child.path);

      if (child.children) {
        this.upsertChildren(child.path, child.children);
      }
    }

    for (const removedChildId of previousChildIds) {
      this.removeSubtree(removedChildId);
      this.replaceExpandedFolders(expandedFoldersDeleteEquivalent(this.expandedFolders, removedChildId));
      this.loadingPaths.delete(removedChildId);
    }

    parent.childIds = nextChildIds;
    parent.childrenState = 'resolved';
    parent.stale = false;
    parent.errorMessage = undefined;
    this.loadingPaths.delete(parentPath);
  }

  markDirectoryError(path: string, message: string): void {
    const node = this.nodes.get(path);
    if (!node || node.kind !== 'directory') {
      return;
    }

    node.childrenState = 'error';
    node.stale = true;
    node.errorMessage = message;
    this.loadingPaths.delete(path);
  }

  getNode(path: string): ExplorerNodeRecord | undefined {
    return this.nodes.get(path);
  }

  getExpandedFolders(): Set<string> {
    return new Set(this.expandedFolders);
  }

  getSnapshot(): ExplorerSnapshot {
    return {
      rootPath: this.rootPath,
      fileTree: this.projectTree(),
      selectedFile: this.selectedFile,
      expandedFolders: new Set(this.expandedFolders),
      loading: this.loading,
      error: this.error,
      loadingPaths: new Set(this.loadingPaths),
      options: cloneOptions(this.options),
    };
  }

  private bootstrapRoot(rootPath: string): void {
    const rootName = rootPath.split(/[/\\]/).filter(Boolean).pop() || rootPath;
    const rootRecord = createDirectoryRecord(rootPath, rootName, null, true);
    this.nodes.set(rootPath, rootRecord);
    this.roots.length = 0;
    this.roots.push(rootPath);
    this.replaceExpandedFolders(expandedFoldersAddEquivalent(this.expandedFolders, rootPath));
  }

  private removeSubtree(nodeId: string): void {
    const node = this.nodes.get(nodeId);
    if (!node) {
      return;
    }

    for (const childId of node.childIds) {
      this.removeSubtree(childId);
    }

    this.nodes.delete(nodeId);
  }

  private replaceExpandedFolders(next: Set<string>): void {
    this.expandedFolders.clear();
    next.forEach((value) => this.expandedFolders.add(value));
  }

  private projectTree(): FileSystemNode[] {
    return this.roots
      .map((rootId) => this.projectNode(rootId))
      .filter((node): node is FileSystemNode => node !== undefined);
  }

  private projectNode(nodeId: string): FileSystemNode | undefined {
    const record = this.nodes.get(nodeId);
    if (!record) {
      return undefined;
    }

    const node: FileSystemNode = {
      path: record.path,
      name: record.name,
      isDirectory: record.kind === 'directory',
      size: record.size,
      extension: record.extension,
      lastModified: record.lastModified,
    };

    if (record.kind === 'directory' && record.childIds.length > 0) {
      node.children = record.childIds
        .map((childId) => this.projectNode(childId))
        .filter((child): child is FileSystemNode => child !== undefined);
    }

    return node;
  }
}
