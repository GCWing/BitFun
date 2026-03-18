 
/**
 * Tool execution event stream types.
 *
 * These payloads typically come from backend events and keep snake_case fields
 * to match the wire format.
 */
export type ToolExecutionStatus = 
  | 'pending'     
  | 'receiving'   
  | 'starting'    
  | 'running'     
  | 'completed'   
  | 'failed'      
  | 'cancelled'   


export interface BaseToolEvent {
  task_id: string;
  tool_use_id: string;
  tool_name: string;
  timestamp: number;
  model_round_id?: string;
  dialog_turn_id?: string;
}


export interface ToolStartEvent extends BaseToolEvent {
  type: 'tool_start';
  input: any;
  ai_intent?: string;  
  requires_confirmation?: boolean;
}


export interface ToolCompleteEvent extends BaseToolEvent {
  type: 'tool_complete';
  result: any;
  result_for_assistant?: string;
  duration_ms: number;
  success: boolean;
  error?: string;
}


export interface ToolProgressEvent extends BaseToolEvent {
  type: 'tool_progress';
  progress: number;  // 0-1
  message?: string;
}


export type ToolEvent = ToolStartEvent | ToolCompleteEvent | ToolProgressEvent;


export interface ToolExecutionState {
  toolUseId: string;
  toolName: string;
  status: ToolExecutionStatus;
  input: any;
  result?: any;
  error?: string;
  startTime: number;
  endTime?: number;
  duration?: number;
  aiIntent?: string;
  requiresConfirmation?: boolean;
  userConfirmed?: boolean;
}
