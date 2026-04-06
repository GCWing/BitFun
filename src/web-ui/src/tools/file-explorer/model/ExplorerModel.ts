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

function createNodeRecord(
  node: FileSystemNode,
  parentId: string | null,
  isRoot: boolean,
  childIds: string[],
  childrenState: ExplorerNodeRecord['childrenState']
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
    childIds,
    childrenState,
    hasMoreChildren: node.hasMoreChildren ?? false,
    totalChildren: node.totalChildren ?? childIds.length,
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
  private silentRefreshing = false;
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
    this.silentRefreshing = false;
    this.error = undefined;
  }

  setRootPath(rootPath?: string): void {
    this.rootPath = rootPath;
  }

  setLoading(loading: boolean, silentRefreshing = false): void {
    this.loading = loading;
    this.silentRefreshing = silentRefreshing;
    if (!loading && !silentRefreshing) {
      this.loadingPaths.clear();
    }
  }

  setError(error?: string): void {
    this.error = error;
    if (error) {
      this.loading = false;
      this.silentRefreshing = false;
    }
  }

  clearTransientErrors(): void {
    this.error = undefined;
  }

  replaceTree(rootNodes: FileSystemNode[]): void {
    this.nodes.clear();
    this.roots.length = 0;

    for (const rootNode of rootNodes) {
      this.insertSubtree(rootNode, null, true);
      this.roots.push(rootNode.path);
      this.expandedFolders.add(rootNode.path);
    }

    this.loading = false;
    this.silentRefreshing = false;
    this.error = undefined;
  }

  replaceRootChildren(
    rootPath: string,
    children: FileSystemNode[],
    totalChildren = children.length,
    hasMoreChildren = false
  ): void {
    const rootNode: FileSystemNode = {
      path: rootPath,
      name: rootPath.split(/[/\\]/).filter(Boolean).pop() || rootPath,
      isDirectory: true,
      children,
      totalChildren,
      hasMoreChildren,
      loadedChildrenCount: children.length,
    };

    this.replaceTree([rootNode]);
  }

  setDirectoryLoading(path: string, loading: boolean): void {
    if (loading) {
      this.loadingPaths.add(path);
      this.expandedFolders.add(path);
      const existing = this.nodes.get(path);
      if (existing && existing.kind === 'directory') {
        existing.childrenState = 'loading';
      }
      return;
    }

    this.loadingPaths.delete(path);
    const existing = this.nodes.get(path);
    if (existing && existing.kind === 'directory' && existing.childrenState === 'loading') {
      existing.childrenState = 'resolved';
    }
  }

  upsertChildren(
    parentPath: string,
    children: FileSystemNode[],
    options: {
      append?: boolean;
      totalChildren?: number;
      hasMoreChildren?: boolean;
    } = {}
  ): void {
    const parent = this.nodes.get(parentPath);
    if (!parent || parent.kind !== 'directory') {
      return;
    }

    const append = options.append ?? false;
    const previousChildIds = append ? new Set<string>() : new Set(parent.childIds);
    const nextChildIds: string[] = append ? [...parent.childIds] : [];
    const existingChildIds = new Set(nextChildIds);

    for (const child of children) {
      const existing = this.nodes.get(child.path);
      const nextRecord = createNodeRecord(
        child,
        parentPath,
        false,
        existing?.kind === 'directory' ? existing.childIds : [],
        child.isDirectory
          ? existing?.childrenState ?? (child.children ? 'resolved' : 'unresolved')
          : 'resolved'
      );

      if (child.children) {
        nextRecord.childrenState = 'resolved';
      }

      this.nodes.set(child.path, nextRecord);
      if (!existingChildIds.has(child.path)) {
        nextChildIds.push(child.path);
        existingChildIds.add(child.path);
      }
      previousChildIds.delete(child.path);

      if (child.children) {
        this.upsertChildren(child.path, child.children, {
          append: false,
          totalChildren: child.totalChildren,
          hasMoreChildren: child.hasMoreChildren,
        });
      }
    }

    for (const removedChildId of previousChildIds) {
      this.removeSubtree(removedChildId);
      this.expandedFolders.delete(removedChildId);
      this.loadingPaths.delete(removedChildId);
    }

    parent.childIds = nextChildIds;
    parent.childrenState = 'resolved';
    parent.totalChildren = options.totalChildren ?? nextChildIds.length;
    parent.hasMoreChildren = options.hasMoreChildren ?? false;
    parent.errorMessage = undefined;
    this.loadingPaths.delete(parentPath);
  }

  markDirectoryError(path: string, message: string): void {
    const node = this.nodes.get(path);
    if (!node || node.kind !== 'directory') {
      return;
    }

    node.childrenState = 'error';
    node.errorMessage = message;
    this.loadingPaths.delete(path);
  }

  expand(path: string, expanded = true): void {
    if (expanded) {
      this.expandedFolders.add(path);
      return;
    }

    this.expandedFolders.delete(path);
  }

  select(filePath?: string): void {
    this.selectedFile = filePath;
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
      silentRefreshing: this.silentRefreshing,
      error: this.error,
      loadingPaths: new Set(this.loadingPaths),
      options: cloneOptions(this.options),
    };
  }

  private insertSubtree(node: FileSystemNode, parentId: string | null, isRoot: boolean): string {
    const childIds = (node.children ?? []).map(child => child.path);
    const record = createNodeRecord(
      node,
      parentId,
      isRoot,
      childIds,
      node.isDirectory ? (node.children ? 'resolved' : 'unresolved') : 'resolved'
    );
    this.nodes.set(node.path, record);

    for (const child of node.children ?? []) {
      this.insertSubtree(child, node.path, false);
    }

    return node.path;
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

  private projectTree(): FileSystemNode[] {
    return this.roots
      .map(rootId => this.projectNode(rootId))
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
      hasMoreChildren: record.kind === 'directory' ? record.hasMoreChildren : undefined,
      totalChildren: record.kind === 'directory' ? record.totalChildren : undefined,
      loadedChildrenCount: record.kind === 'directory' ? record.childIds.length : undefined,
    };

    if (record.kind === 'directory' && record.childIds.length > 0) {
      node.children = record.childIds
        .map(childId => this.projectNode(childId))
        .filter((child): child is FileSystemNode => child !== undefined);
    }

    return node;
  }
}
