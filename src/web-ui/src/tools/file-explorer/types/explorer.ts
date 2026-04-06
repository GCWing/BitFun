import type { FileSystemChangeEvent, FileSystemNode, FileSystemOptions } from '@/tools/file-system/types';

export type ExplorerNodeId = string;

export type ExplorerNodeKind = 'file' | 'directory';

export type ExplorerChildrenState = 'unresolved' | 'loading' | 'resolved' | 'error';

export interface ExplorerNodeRecord {
  id: ExplorerNodeId;
  path: string;
  name: string;
  parentId: ExplorerNodeId | null;
  kind: ExplorerNodeKind;
  size?: number;
  extension?: string;
  lastModified?: Date;
  childIds: ExplorerNodeId[];
  childrenState: ExplorerChildrenState;
  hasMoreChildren: boolean;
  totalChildren: number;
  errorMessage?: string;
  isRoot: boolean;
}

export interface ExplorerSnapshot {
  rootPath?: string;
  fileTree: FileSystemNode[];
  selectedFile?: string;
  expandedFolders: Set<string>;
  loading: boolean;
  silentRefreshing: boolean;
  error?: string;
  loadingPaths: Set<string>;
  options: FileSystemOptions;
}

export interface ExplorerControllerConfig extends FileSystemOptions {
  rootPath?: string;
  autoLoad?: boolean;
  enableAutoWatch?: boolean;
  enableLazyLoad?: boolean;
  pollingIntervalMs?: number;
}

export interface ExplorerChildrenRequest {
  path: string;
  offset?: number;
  limit?: number;
  options?: FileSystemOptions;
}

export interface ExplorerChildrenPage {
  children: FileSystemNode[];
  total: number;
  hasMore: boolean;
  offset: number;
  limit: number;
}

export interface ExplorerFileSystemProvider {
  getFileTree(rootPath: string, options?: FileSystemOptions): Promise<FileSystemNode[]>;
  getChildren(request: ExplorerChildrenRequest): Promise<FileSystemNode[]>;
  getChildrenPage(request: ExplorerChildrenRequest): Promise<ExplorerChildrenPage>;
  watch(rootPath: string, callback: (event: FileSystemChangeEvent) => void): () => void;
}
