/**
 * Simplified type definitions with essential base types only.
 */

// Tool calls (aligned with backend).
export interface ToolCall {
  id: string;
  name: string;
  input: any;
  result?: any;
}

// Agent execution requests (aligned with backend).
export interface AgentExecutionRequest {
  agent_type: string;
  prompt: string;
  description?: string;
  model_name?: string;
  workspace_path?: string;
  context?: Record<string, any>;
  safe_mode?: boolean;
  verbose?: boolean;
}

// Agent results (aligned with backend).
export interface AgentResult {
  status: 'success' | 'error' | 'partial';
  content: string;
  error?: string;
  metadata?: Record<string, any>;
  tool_calls?: ToolCall[];
}