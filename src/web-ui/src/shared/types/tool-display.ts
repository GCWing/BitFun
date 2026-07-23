/**
 * Tool Display System Types
 */

export interface ToolExecutionInfo {
  id: string;
  toolName: string;
  userFacingName?: string;
  input: Record<string, any>;
  startTime: number;
  status: 'queued' | 'executing' | 'completed' | 'error' | 'cancelled';
  costUSD?: number;
  durationMs?: number;
  error?: string;
  aiIntent?: string; 
  progressMessage?: string; 
  progressPercentage?: number; 
}

export interface ToolResult {
  toolUseId: string;
  toolName: string;
  data: any;
  error?: string;
  isError: boolean;
  timestamp: number;
}

export interface ToolDisplayMessage {
  id: string;
  type: 'tool_use' | 'tool_result';
  toolExecution?: ToolExecutionInfo;
  toolResult?: ToolResult;
  timestamp: number;
}

export interface ToolDisplayOptions {
  verbose: boolean;
  showCost?: boolean;
  showDuration?: boolean;
  maxOutputLines?: number;
  truncateAfter?: number;
}

export interface ToolDisplayComponentProps {
  toolExecution?: ToolExecutionInfo;
  toolResult?: ToolResult;
  options?: ToolDisplayOptions;
  width?: number | string;
}

export interface BashToolResult {
  stdout: string;
  stdoutLines: number;
  stderr: string;
  stderrLines: number;
  interrupted: boolean;
  exitCode?: number;
}

export interface FileToolResult {
  content?: string;
  filePath: string;
  operation: 'read' | 'write' | 'edit' | 'multi-edit';
  changes?: Array<{
    type: 'add' | 'remove' | 'modify';
    line: number;
    content: string;
  }>;
  success: boolean;
}

export interface SearchToolResult {
  pattern: string;
  results: Array<{
    file: string;
    line: number;
    content: string;
    context?: string[];
  }>;
  totalMatches: number;
  filesSearched: number;
}

export interface WebToolResult {
  url?: string;
  query?: string;
  content: string;
  title?: string;
  results?: Array<{
    title: string;
    url: string;
    snippet: string;
  }>;
}


export interface MemoryToolResult {
  operation: 'read' | 'write';
  key?: string;
  value?: any;
  success: boolean;
  memories?: Array<{
    key: string;
    value: any;
    timestamp: number;
  }>;
}

export interface AdvancedToolResult {
  type: 'task' | 'think' | 'todo' | 'expert' | 'architect';
  content: string;
  agentType?: string;
  todos?: Array<{
    id: string;
    content: string;
    status: 'pending' | 'completed' | 'cancelled';
  }>;
  thinking?: string;
  suggestions?: string[];
}
