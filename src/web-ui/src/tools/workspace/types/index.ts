/**
 * Workspace management feature types.
 */

// Workspace models
export interface Workspace {
  id: string;
  name: string;
  rootPath: string;
  lastOpened: Date;
  settings?: WorkspaceSettings;
  metadata?: WorkspaceMetadata;
}

export interface WorkspaceSettings {
  excludePatterns: string[];
  includePatterns: string[];
  watchIgnore: string[];
  maxFileSize: number;
  encoding: string;
  lineEnding: 'auto' | 'lf' | 'crlf';
  trimTrailingWhitespace: boolean;
  insertFinalNewline: boolean;
  autoSave: boolean;
  autoSaveDelay: number;
  fileAssociations: Record<string, string>;
  searchExclude: string[];
  filesExclude: string[];
}

export interface WorkspaceMetadata {
  description?: string;
  version?: string;
  author?: string;
  createdAt: Date;
  lastModified: Date;
  fileCount?: number;
  totalSize?: number;
  languages?: string[];
  gitRepository?: GitRepositoryInfo;
}

export interface GitRepositoryInfo {
  url: string;
  branch: string;
  commit: string;
  isDirty: boolean;
  hasUnpushedCommits: boolean;
}

// File system models
export interface FileSystemItem {
  id: string;
  name: string;
  path: string;
  type: 'file' | 'directory';
  size?: number;
  lastModified: Date;
  isHidden: boolean;
  children?: FileSystemItem[];
  extension?: string;
  language?: string;
  encoding?: string;
  permissions?: FilePermissions;
}

export interface FilePermissions {
  readable: boolean;
  writable: boolean;
  executable: boolean;
}

// Workspace state
export interface WorkspaceState {
  currentWorkspace: Workspace | null;
  recentWorkspaces: Workspace[];
  fileTree: FileSystemItem[];
  selectedItems: string[];
  expandedItems: string[];
  loading: boolean;
  error: string | null;
  searchQuery: string;
  searchResults: FileSystemItem[];
  isSearching: boolean;
}

// Workspace events
export type WorkspaceEvent = 
  | { type: 'workspace:opened'; payload: Workspace }
  | { type: 'workspace:closed'; payload: { workspaceId: string } }
  | { type: 'workspace:changed'; payload: Workspace }
  | { type: 'file:created'; payload: FileSystemItem }
  | { type: 'file:deleted'; payload: { path: string } }
  | { type: 'file:renamed'; payload: { oldPath: string; newPath: string } }
  | { type: 'file:modified'; payload: FileSystemItem }
  | { type: 'directory:created'; payload: FileSystemItem }
  | { type: 'directory:deleted'; payload: { path: string } }
  | { type: 'search:started'; payload: { query: string } }
  | { type: 'search:completed'; payload: { query: string; results: FileSystemItem[] } }
  | { type: 'error:occurred'; payload: { error: string } };

// File operations and search options
export interface FileOperationOptions {
  overwrite?: boolean;
  recursive?: boolean;
  preserveTimestamps?: boolean;
  encoding?: string;
}

export interface WorkspaceSearchOptions {
  includePatterns?: string[];
  excludePatterns?: string[];
  caseSensitive?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
  includeHidden?: boolean;
  maxResults?: number;
  maxDepth?: number;
}

export interface FileWatchOptions {
  recursive?: boolean;
  ignorePatterns?: string[];
  debounceDelay?: number;
}

// Workspace manager interface
export interface IWorkspaceManager {
  // Workspace operations
  openWorkspace(path: string): Promise<Workspace>;
  closeWorkspace(): Promise<void>;
  getCurrentWorkspace(): Workspace | null;
  getRecentWorkspaces(): Workspace[];
  
  // File system operations
  getFileTree(path?: string): Promise<FileSystemItem[]>;
  readFile(path: string): Promise<string>;
  writeFile(path: string, content: string, options?: FileOperationOptions): Promise<void>;
  createFile(path: string, content?: string): Promise<FileSystemItem>;
  deleteFile(path: string): Promise<void>;
  renameFile(oldPath: string, newPath: string): Promise<void>;
  copyFile(sourcePath: string, destPath: string, options?: FileOperationOptions): Promise<void>;
  
  // Directory operations
  createDirectory(path: string): Promise<FileSystemItem>;
  deleteDirectory(path: string, recursive?: boolean): Promise<void>;
  listDirectory(path: string): Promise<FileSystemItem[]>;
  
  // Search
  searchFiles(query: string, options?: WorkspaceSearchOptions): Promise<FileSystemItem[]>;
  searchInFiles(query: string, options?: WorkspaceSearchOptions): Promise<SearchInFilesResult[]>;
  
  // Watchers
  watchFile(path: string, callback: (event: FileWatchEvent) => void, options?: FileWatchOptions): () => void;
  watchDirectory(path: string, callback: (event: FileWatchEvent) => void, options?: FileWatchOptions): () => void;
  
  // Settings
  getWorkspaceSettings(): WorkspaceSettings;
  updateWorkspaceSettings(settings: Partial<WorkspaceSettings>): Promise<void>;
  
  // Events
  addEventListener(listener: (event: WorkspaceEvent) => void): () => void;
}

// Watch events
export interface FileWatchEvent {
  type: 'created' | 'modified' | 'deleted' | 'renamed';
  path: string;
  newPath?: string; // Used for rename events.
  timestamp: Date;
}

// Search results
export interface SearchInFilesResult {
  file: FileSystemItem;
  matches: SearchMatch[];
  totalMatches: number;
}

export interface SearchMatch {
  line: number;
  column: number;
  length: number;
  text: string;
  context: string;
}

// UI contracts
export interface WorkspaceExplorerProps {
  onFileSelect?: (file: FileSystemItem) => void;
  onDirectorySelect?: (directory: FileSystemItem) => void;
  onFileOpen?: (file: FileSystemItem) => void;
  onFileContextMenu?: (file: FileSystemItem, event: React.MouseEvent) => void;
  showHidden?: boolean;
  sortBy?: 'name' | 'type' | 'size' | 'modified';
  sortOrder?: 'asc' | 'desc';
  className?: string;
}

export interface FileTreeProps {
  items: FileSystemItem[];
  selectedItems: string[];
  expandedItems: string[];
  onItemSelect?: (item: FileSystemItem) => void;
  onItemExpand?: (item: FileSystemItem, expanded: boolean) => void;
  onItemContextMenu?: (item: FileSystemItem, event: React.MouseEvent) => void;
  renderItem?: (item: FileSystemItem) => React.ReactNode;
  className?: string;
}

export interface UseWorkspaceReturn {
  // State
  currentWorkspace: Workspace | null;
  recentWorkspaces: Workspace[];
  fileTree: FileSystemItem[];
  selectedItems: string[];
  loading: boolean;
  error: string | null;
  searchResults: FileSystemItem[];
  isSearching: boolean;
  
  // Actions
  openWorkspace: (path: string) => Promise<Workspace>;
  closeWorkspace: () => Promise<void>;
  refreshFileTree: () => Promise<void>;
  selectItem: (itemId: string, multi?: boolean) => void;
  expandItem: (itemId: string, expanded: boolean) => void;
  createFile: (path: string, content?: string) => Promise<FileSystemItem>;
  createDirectory: (path: string) => Promise<FileSystemItem>;
  deleteItem: (path: string) => Promise<void>;
  renameItem: (oldPath: string, newPath: string) => Promise<void>;
  searchFiles: (query: string, options?: WorkspaceSearchOptions) => Promise<FileSystemItem[]>;
  clearError: () => void;
}

// Context menu
export interface FileContextMenuItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  shortcut?: string;
  action: (item: FileSystemItem) => void;
  separator?: boolean;
  disabled?: boolean;
  submenu?: FileContextMenuItem[];
}

// Workspace config
export interface WorkspaceConfig {
  maxFileSize: number;
  maxSearchResults: number;
  watcherIgnorePatterns: string[];
  autoRefreshInterval: number;
  enableFileWatcher: boolean;
  showHiddenFiles: boolean;
  sortFilesBy: 'name' | 'type' | 'size' | 'modified';
  sortOrder: 'asc' | 'desc';
}

// Workspace stats
export interface WorkspaceStats {
  totalFiles: number;
  totalDirectories: number;
  totalSize: number;
  filesByType: Record<string, number>;
  largestFiles: Array<{ path: string; size: number }>;
  recentlyModified: Array<{ path: string; modified: Date }>;
}
