 



export interface OpenWorkspaceRequest {
  path: string;
}

export interface WorkspaceInfo {
  id: string;
  name: string;
  rootPath: string;
  workspaceType: string;
  workspaceKind: string;
  assistantId?: string | null;
  languages: string[];
  openedAt: string;
  lastAccessed: string;
  description?: string | null;
  tags: string[];
  statistics?: {
    totalFiles: number;
    totalLines: number;
    totalSize: number;
    filesByLanguage: Record<string, number>;
    filesByExtension: Record<string, number>;
    lastUpdated: string;
  } | null;
  identity?: {
    name?: string | null;
    creature?: string | null;
    vibe?: string | null;
    emoji?: string | null;
  } | null;
  connectionId?: string;
  connectionName?: string;
}

export interface FileOperationRequest {
  path: string;
}

export interface WriteFileRequest {
  path: string;
  content: string;
}



export interface GetConfigRequest {
  path?: string;
}

export interface SetConfigRequest {
  path: string;
  value: any;
}

export interface ResetConfigRequest {
  path?: string;
}

export interface ImportConfigRequest {
  configData: any;
}



export interface GetModelInfoRequest {
  modelId: string;
}

export interface TestConnectionRequest {
  config: any;
}

export interface SendMessageRequest {
  message: string;
  context?: any;
}

export interface FixMermaidCodeRequest {
  sourceCode: string;
  errorMessage: string;
}



export interface GetToolInfoRequest {
  toolName: string;
}

export interface ExecuteToolRequest {
  toolName: string;
  parameters: any;
  workspacePath?: string;
}

export interface ValidateToolInputRequest {
  toolName: string;
  input: any;
  workspacePath?: string;
}



export interface AnalyzeProjectRequest {
  path: string;
  options?: any;
}

export interface SearchCodeRequest {
  query: string;
  options?: any;
}



export interface OpenExternalRequest {
  url: string;
}

export interface ShowInFolderRequest {
  path: string;
}

export interface SetClipboardRequest {
  text: string;
}



export interface ComputeDiffRequest {
  oldContent: string;
  newContent: string;
  options?: any;
}

export interface ApplyPatchRequest {
  content: string;
  patch: string;
}



export interface SearchFilesRequest {
  rootPath: string;
  pattern: string;
  searchContent?: boolean;
  searchId?: string;
  caseSensitive?: boolean;
  useRegex?: boolean;
  wholeWord?: boolean;
  maxResults?: number;
  includeDirectories?: boolean;
}

export interface SearchFilenamesRequest {
  rootPath: string;
  pattern: string;
  searchId?: string;
  caseSensitive?: boolean;
  useRegex?: boolean;
  wholeWord?: boolean;
  maxResults?: number;
  includeDirectories?: boolean;
}

export interface SearchFileContentsRequest {
  rootPath: string;
  pattern: string;
  searchId?: string;
  caseSensitive?: boolean;
  useRegex?: boolean;
  wholeWord?: boolean;
  maxResults?: number;
}

export interface CancelSearchRequest {
  searchId: string;
}

export interface SearchRepoIndexRequest {
  rootPath: string;
}

export type SearchMatchType = 'fileName' | 'content';

export interface FileSearchResult {
  path: string;
  name: string;
  isDirectory: boolean;
  matchType: SearchMatchType;
  lineNumber?: number;
  matchedContent?: string;
  previewBefore?: string;
  previewInside?: string;
  previewAfter?: string;
}

export interface FileSearchResponse {
  results: FileSearchResult[];
  limit: number;
  truncated: boolean;
  searchMetadata?: SearchMetadata;
}

export interface FileSearchResultGroup {
  path: string;
  name: string;
  isDirectory: boolean;
  fileNameMatch?: FileSearchResult;
  contentMatches: FileSearchResult[];
}

export type FileSearchStreamKind = 'filenames' | 'content';

export interface FileSearchStreamStartResponse {
  searchId: string;
  limit: number;
}

export interface FileSearchProgressEvent {
  searchId: string;
  searchKind: FileSearchStreamKind;
  results: FileSearchResultGroup[];
}

export interface FileSearchCompleteEvent {
  searchId: string;
  searchKind: FileSearchStreamKind;
  limit: number;
  truncated: boolean;
  totalResults: number;
  searchMetadata?: SearchMetadata;
}

export interface FileSearchErrorEvent {
  searchId: string;
  searchKind: FileSearchStreamKind;
  error: string;
}

export type SearchBackendKind =
  | 'indexed'
  | 'indexed_workspace'
  | 'text_fallback'
  | 'scan_fallback';

export interface SearchMetadata {
  backend: SearchBackendKind | string;
  repoPhase: WorkspaceSearchRepoPhase | string;
  rebuildRecommended: boolean;
  candidateDocs: number;
  matchedLines: number;
  matchedOccurrences: number;
}

export type WorkspaceSearchRepoPhase =
  | 'preparing'
  | 'needs_index'
  | 'building'
  | 'ready'
  | 'stale'
  | 'refreshing'
  | 'limited';

export type WorkspaceSearchTaskKind =
  | 'build'
  | 'rebuild'
  | 'refresh';

export type WorkspaceSearchTaskState =
  | 'queued'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type WorkspaceSearchTaskPhase =
  | 'discovering'
  | 'processing'
  | 'persisting'
  | 'finalizing'
  | 'refreshing';

export interface WorkspaceSearchDirtyFiles {
  modified: number;
  deleted: number;
  new: number;
}

export interface WorkspaceSearchRepoStatus {
  repoId: string;
  repoPath: string;
  storageRoot: string;
  baseSnapshotRoot: string;
  workspaceOverlayRoot: string;
  phase: WorkspaceSearchRepoPhase;
  snapshotKey?: string | null;
  lastProbeUnixSecs?: number | null;
  lastRebuildUnixSecs?: number | null;
  dirtyFiles: WorkspaceSearchDirtyFiles;
  rebuildRecommended: boolean;
  activeTaskId?: string | null;
  probeHealthy: boolean;
  lastError?: string | null;
  overlay?: WorkspaceSearchOverlayStatus | null;
}

export interface WorkspaceSearchOverlayStatus {
  committedSeqNo: number;
  lastSeqNo: number;
  uncommittedOps: number;
  pendingDocs: number;
  activeSegments: number;
  activeDeleteSegments: number;
  mergeRequested: boolean;
  mergeRunning: boolean;
  mergeAttempts: number;
  mergeCompleted: number;
  mergeFailed: number;
  lastMergeError?: string | null;
}

export interface WorkspaceSearchTaskStatus {
  taskId: string;
  workspaceId: string;
  kind: WorkspaceSearchTaskKind;
  state: WorkspaceSearchTaskState;
  phase?: WorkspaceSearchTaskPhase | null;
  message: string;
  processed: number;
  total?: number | null;
  startedUnixSecs: number;
  updatedUnixSecs: number;
  finishedUnixSecs?: number | null;
  cancellable: boolean;
  error?: string | null;
}

export interface WorkspaceSearchIndexStatus {
  repoStatus: WorkspaceSearchRepoStatus;
  activeTask?: WorkspaceSearchTaskStatus | null;
}

export interface WorkspaceSearchIndexTaskHandle {
  task: WorkspaceSearchTaskStatus;
  repoStatus: WorkspaceSearchRepoStatus;
}

export interface ExplorerNodeDto {
  path: string;
  name: string;
  isDirectory: boolean;
  size?: number | null;
  extension?: string | null;
  lastModified?: number | null;
  children?: ExplorerNodeDto[];
}

export interface ExplorerChildrenPageDto {
  children: ExplorerNodeDto[];
  total: number;
  hasMore: boolean;
  offset: number;
  limit: number;
}
