/**
 * Agent/tool API DTOs (frontend).
 *
 * These types mirror backend request/response payloads and intentionally use
 * snake_case fields to match the wire format.
 */
export interface AgentExecutionRequest {
  agent_type: string;
  prompt: string;
  description?: string;
  model_name?: string;
  workspace_path?: string;
  context?: Record<string, string>;
  safe_mode?: boolean;
  verbose?: boolean;
}

export interface AgentExecutionResponse {
  id: string;
  status: string;
  result?: any;
  error?: string;
  progress?: string;
  agent_type: string;
  duration_ms?: number;
  tool_uses?: number;
}

export interface AgentInfo {
  agent_type: string;
  when_to_use: string;
  tools: string;
  system_prompt?: string;
  location: string;
  color?: string;
  model_name?: string;
}

export interface ToolInfo {
  name: string;
  description: string;
  input_schema: any;
  is_readonly: boolean;
  is_concurrency_safe: boolean;
  needs_permissions: boolean;
  dynamic_info?: DynamicToolInfo;
}

export interface DynamicToolInfo {
  providerId: string;
  providerKind?: string;
  mcp?: DynamicMcpToolInfo;
}

export interface DynamicMcpToolInfo {
  serverId: string;
  serverName: string;
  toolName: string;
}

export interface ToolExecutionRequest {
  tool_name: string;
  input: any;
  context?: Record<string, string>;
  safe_mode?: boolean;
}

export interface ToolExecutionResponse {
  tool_name: string;
  success: boolean;
  result?: any;
  error?: string;
  validation_error?: string;
  duration_ms: number;
}

export interface ToolValidationRequest {
  tool_name: string;
  input: any;
}

export interface ToolValidationResponse {
  tool_name: string;
  valid: boolean;
  message?: string;
  error_code?: number;
  meta?: any;
}


export interface AgentTaskUpdateEvent {
  type: 'task_started' | 'task_progress' | 'task_completed' | 'task_error';
  task_id: string;
  data?: any;
  error?: string;
  progress?: string;
  agent_type?: string;
  description?: string;
}





export interface DialogTurnStartEvent {
  task_id: string;
  dialog_turn_id: string;
  user_message: string;
  timestamp: number;
}

export interface DialogTurnCompleteEvent {
  task_id: string;
  dialog_turn_id: string;
  status: 'completed' | 'error';
  total_model_rounds: number;
  timestamp: number;
}


export interface ModelRoundStartEvent {
  task_id: string;
  dialog_turn_id: string;
  model_round_id: string;
  model_round_index: number;
  timestamp: number;
}

export interface ModelRoundContentEvent {
  task_id: string;
  dialog_turn_id: string;
  model_round_id: string;
  content_id: string;
  content_type: 'text' | 'tool_call' | 'tool_result' | 'thinking';
  content: string;
  metadata?: {
    tool_name?: string;
    tool_use_id?: string;
    tool_input?: any;
    is_streaming?: boolean;
    chunk_index?: number;
    total_chunks?: number;
  };
  timestamp: number;
}

export interface ModelRoundEndEvent {
  task_id: string;
  dialog_turn_id: string;
  model_round_id: string;
  round_status: 'completed' | 'pending_confirmation' | 'error';
  timestamp: number;
}


export interface TaskCompleteEvent {
  task_id: string;
  status: 'completed' | 'error';
  total_dialog_turns: number;
  result?: any;
  timestamp: number;
}

export interface TaskErrorEvent {
  task_id: string;
  dialog_turn_id?: string;
  model_round_id?: string;
  error: string;
  timestamp: number;
}


 
export interface StreamChunkEvent {
  task_id: string;
  type: 'text' | 'tool_result' | 'thinking';
  content: string;
  model_round_id?: string;    
  dialog_turn_id?: string;    
  chunk_index?: number;
  total_chunks?: number;
  timestamp: number;
}

 
export interface StreamToolUseEvent {
  task_id: string;
  tool_use_id: string;
  tool_name: string;
  input: any;
  model_round_id?: string;    
  dialog_turn_id?: string;    
  timestamp: number;
}

 
export interface StreamToolResultEvent {
  task_id: string;
  type: 'tool_result';
  content: string;
  timestamp: number;
}

 
export interface StreamStartEvent {
  task_id: string;
  agent_type: string;
  prompt: string;
}

 
export interface StreamCompleteEvent {
  task_id: string;
  status: 'completed';
  result?: any;
}

 
export interface StreamErrorEvent {
  task_id: string;
  error: string;
  timestamp?: number;
}

export interface StreamProgressEvent {
  task_id: string;
  type: 'progress';
  timestamp: number;
}


export interface ToolCallConfirmationEvent {
  request: {
    call_id: string;
    name: string;
    args: Record<string, any>;
    is_client_initiated: boolean;
    prompt_id: string;
  };
  confirmation_type: string; // 'edit' | 'execute' | 'confirm'
  message?: string;
  file_diff?: string;
  file_name?: string;
  original_content?: string;
  new_content?: string;
}
