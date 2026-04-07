import type { FileSystemChangeEvent, FileSystemNode, FileSystemOptions } from '@/tools/file-system/types';

export type ExplorerNodeId = string;

export type ExplorerNodeKind = 'file' | 'directory';

export type ExplorerChildrenState = 'unresolved' | 'refreshing' | 'resolved' | 'error';

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
  stale: boolean;
  errorMessage?: string;
  isRoot: boolean;
}

export interface ExplorerSnapshot {
  rootPath?: string;
  fileTree: FileSystemNode[];
  selectedFile?: string;
  expandedFolders: Set<string>;
  loading: boolean;
  error?: string;
  loadingPaths: Set<string>;
  options: FileSystemOptions;
}

export interface ExplorerControllerConfig extends FileSystemOptions {
  rootPath?: string;
  autoLoad?: boolean;
  enableAutoWatch?: boolean;
}

export interface ExplorerChildrenRequest {
  path: string;
  options?: FileSystemOptions;
}

export interface ExplorerFileSystemProvider {
  getChildren(request: ExplorerChildrenRequest): Promise<FileSystemNode[]>;
  watch(rootPath: string, callback: (event: FileSystemChangeEvent) => void): () => void;
}
