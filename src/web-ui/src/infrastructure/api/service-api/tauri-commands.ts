 



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
}

export type SearchMatchType = 'fileName' | 'content';

export interface FileSearchResult {
  path: string;
  name: string;
  isDirectory: boolean;
  matchType: SearchMatchType;
  lineNumber?: number;
  matchedContent?: string;
}
