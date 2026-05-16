/**
 * Agent service (frontend).
 *
 * Wraps agent/tool APIs and bridges backend streaming events into a convenient
 * client-side interface.
 */
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { listen } from '@tauri-apps/api/event';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AgentService');
const hasTauriRuntime = (): boolean =>
  typeof window !== 'undefined' &&
  ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
import type {
  AgentExecutionRequest,
  AgentExecutionResponse,
  AgentInfo,
  ToolInfo,
  ToolExecutionRequest,
  ToolExecutionResponse,
  ToolValidationRequest,
  ToolValidationResponse,
  AgentTaskUpdateEvent,
  StreamChunkEvent,
  StreamToolUseEvent,
  StreamToolResultEvent,
  StreamProgressEvent,
  StreamStartEvent,
  StreamCompleteEvent,
  StreamErrorEvent,
  ToolCallConfirmationEvent,
} from '../types/agent-api';

export class AgentService {
  private static instance: AgentService;
  private taskListeners = new Map<string, (event: AgentTaskUpdateEvent) => void>();
  private streamListeners = new Map<string, {
    onChunk?: (event: StreamChunkEvent) => void;
    onToolUse?: (event: StreamToolUseEvent) => void;
    onToolResult?: (event: StreamToolResultEvent) => void;
    onProgress?: (event: StreamProgressEvent) => void;
    onComplete?: (event: StreamCompleteEvent) => void;
    onError?: (event: StreamErrorEvent) => void;
    onModelRoundStart?: (event: any) => void;
    onToolConfirmation?: (event: ToolCallConfirmationEvent) => void;
  }>();
  private unlistenFunctions: Array<() => void> = []; 

  private constructor() {
    void this.setupEventListeners().catch(error => {
      log.warn('Failed to setup event listeners during startup', error);
    });
    
    
    if (import.meta.hot) {
      import.meta.hot.dispose(() => {
        this.cleanup();
      });
    }
  }

  static getInstance(): AgentService {
    if (!AgentService.instance) {
      AgentService.instance = new AgentService();
    }
    return AgentService.instance;
  }
  
  
  private cleanup(): void {
    this.unlistenFunctions.forEach(unlisten => {
      try {
        unlisten();
      } catch (e) {
        log.warn('Failed to cleanup listener', e);
      }
    });
    this.unlistenFunctions = [];
    this.taskListeners.clear();
    this.streamListeners.clear();
  }

  private async setupEventListeners() {
    if (!hasTauriRuntime()) {
      log.warn('Tauri runtime not available, skipping agent event listeners');
      return;
    }

    const unlisten1 = await listen<AgentTaskUpdateEvent>('agent_task_update', (event) => {
      const taskEvent = event.payload;
      const listener = this.taskListeners.get(taskEvent.task_id);
      if (listener) {
        listener(taskEvent);
      }
    });
    this.unlistenFunctions.push(unlisten1);

    
    const unlisten2 = await listen<StreamStartEvent>('agentic_stream_start', () => {
      // Stream started event handled
    });
    this.unlistenFunctions.push(unlisten2);

    
    const unlisten3 = await listen<any>('model_round_start', (event) => {
      const startEvent = event.payload;
      
      if (startEvent.task_id) {
        const listener = this.streamListeners.get(startEvent.task_id);
        if (listener && listener.onModelRoundStart) {
          listener.onModelRoundStart(startEvent);
        }
      }
    });
    this.unlistenFunctions.push(unlisten3);

    
    const unlisten4 = await listen<any>('tool_execution_event', (event) => {
      const toolEvent = event.payload;
      
      
      if (toolEvent.tool_name === 'TodoWrite' && toolEvent.type === 'tool_start') {
        
        Promise.all([
          import('@/flow_chat/services/FlowChatManager'),
          import('@/flow_chat/state-machine')
        ]).then(([{ FlowChatManager }, { stateMachineManager }]) => {
          const todos = toolEvent.input?.todos || [];
          const merge = toolEvent.input?.merge || false;
          
          
          const flowChatManager = FlowChatManager.getInstance();
          const sessionId = flowChatManager.getSessionIdByTaskId(toolEvent.task_id);
          
          if (sessionId) {
            const machine = stateMachineManager.get(sessionId);
            if (machine) {
              const context = machine.getContext();
              
              
              if (merge && context.planner) {
                
                const existingTodos = context.planner.todos;
                const todoMap = new Map(existingTodos.map(t => [t.id, t]));
                todos.forEach((todo: any) => {
                  todoMap.set(todo.id, todo);
                });
                context.planner.todos = Array.from(todoMap.values());
              } else {
                
                context.planner = {
                  todos,
                  isActive: true
                };
              }
            }
          }
        }).catch(err => {
          log.error('Failed to update state machine Planner', err);
        });
      }
      
      if (toolEvent.task_id) {
        const listener = this.streamListeners.get(toolEvent.task_id);
        
        if (listener) {
          
          if (toolEvent.type === 'tool_preparing' && listener.onToolUse) {
            listener.onToolUse({
              task_id: toolEvent.task_id,
              tool_use_id: toolEvent.tool_use_id,
              tool_name: toolEvent.tool_name,
              input: { _early_detection: true }, 
              model_round_id: toolEvent.model_round_id,
              dialog_turn_id: toolEvent.dialog_turn_id,
              timestamp: toolEvent.timestamp || Date.now(),
              ai_intent: undefined,
              requires_confirmation: false,
              _is_early_detection: true  
            } as any);
          } else if (toolEvent.type === 'tool_start' && listener.onToolUse) {
            listener.onToolUse({
              task_id: toolEvent.task_id,
              tool_use_id: toolEvent.tool_use_id,
              tool_name: toolEvent.tool_name,
              input: toolEvent.input,
              model_round_id: toolEvent.model_round_id,
              dialog_turn_id: toolEvent.dialog_turn_id,
              timestamp: toolEvent.timestamp || Date.now(),
              ai_intent: toolEvent.ai_intent,
              requires_confirmation: toolEvent.requires_confirmation
            } as any);
          } else if (toolEvent.type === 'tool_complete' && listener.onToolResult) {
            const resultEvent = {
              task_id: toolEvent.task_id,
              type: 'tool_result' as const,
              content: toolEvent.result?.content || '',
              timestamp: toolEvent.timestamp || Date.now(),
              tool: toolEvent.tool_name,
              tool_name: toolEvent.tool_name,
              tool_use_id: toolEvent.tool_use_id,
              result: {
                content: toolEvent.result?.content || '',
                data: toolEvent.result?.data, 
                type: toolEvent.success ? 'result' : 'error',
                success: toolEvent.success,
                error: toolEvent.error,
                duration_ms: toolEvent.duration_ms
              }
            };
            
            listener.onToolResult(resultEvent as any);
          }
        }
        
      }
    });
    this.unlistenFunctions.push(unlisten4);

    
    const unlisten5 = await listen<any>('model_round_content', (event) => {
      const contentEvent = event.payload;
      
      if (contentEvent.task_id) {
        const listener = this.streamListeners.get(contentEvent.task_id);
        
        if (listener) {
          
          if (contentEvent.content_type === 'text' && contentEvent.content && listener.onChunk) {
            listener.onChunk({
              task_id: contentEvent.task_id,
              type: 'text' as const,
              content: contentEvent.content,
              model_round_id: contentEvent.model_round_id, 
              dialog_turn_id: contentEvent.dialog_turn_id, 
              timestamp: Date.now()
            });
          } else if (contentEvent.content_type === 'thinking' && contentEvent.content && listener.onChunk) {
            
            listener.onChunk({
              task_id: contentEvent.task_id,
              type: 'thinking' as const,
              content: contentEvent.content,
              model_round_id: contentEvent.model_round_id,
              dialog_turn_id: contentEvent.dialog_turn_id,
              timestamp: Date.now()
            });
          }
        }
        
      }
    });
    this.unlistenFunctions.push(unlisten5);

    const unlisten6 = await listen<StreamChunkEvent>('agentic_stream_chunk', (event) => {
      const chunkEvent = event.payload;
      const listener = this.streamListeners.get(chunkEvent.task_id);
      if (listener?.onChunk) {
        listener.onChunk(chunkEvent);
      }
    });
    this.unlistenFunctions.push(unlisten6);

    const unlisten7 = await listen<StreamToolUseEvent>('agentic_stream_tool_use', (event) => {
      const toolUseEvent = event.payload;
      const listener = this.streamListeners.get(toolUseEvent.task_id);
      if (listener?.onToolUse) {
        
        listener.onToolUse(toolUseEvent);
      }
    });
    this.unlistenFunctions.push(unlisten7);

    const unlisten8 = await listen<StreamToolResultEvent>('agentic_stream_tool_result', (event) => {
      const toolResultEvent = event.payload;
      const listener = this.streamListeners.get(toolResultEvent.task_id);
      if (listener?.onToolResult) {
        listener.onToolResult(toolResultEvent);
      }
    });
    this.unlistenFunctions.push(unlisten8);

    const unlisten9 = await listen<StreamProgressEvent>('agentic_stream_progress', (event) => {
      const progressEvent = event.payload;
      const listener = this.streamListeners.get(progressEvent.task_id);
      if (listener?.onProgress) {
        listener.onProgress(progressEvent);
      }
    });
    this.unlistenFunctions.push(unlisten9);

    const unlisten10 = await listen<StreamCompleteEvent>('agentic_stream_complete', (event) => {
      const completeEvent = event.payload;
      
      
      const listener = this.streamListeners.get(completeEvent.task_id);
      if (listener?.onComplete) {
        listener.onComplete(completeEvent);
      }
      
      
      
      
      
      this.streamListeners.delete(completeEvent.task_id);
    });
    this.unlistenFunctions.push(unlisten10);

    const unlisten11 = await listen<StreamErrorEvent>('agentic_stream_error', (event) => {
      const errorEvent = event.payload;
      const listener = this.streamListeners.get(errorEvent.task_id);
      if (listener?.onError) {
        listener.onError(errorEvent);
      }
      
      this.streamListeners.delete(errorEvent.task_id);
    });
    this.unlistenFunctions.push(unlisten11);

    
    const unlisten12 = await listen<ToolCallConfirmationEvent>('backend-event-toolcallconfirmation', (event) => {
      const confirmationEvent = event.payload;
      
      
      
      for (const listener of this.streamListeners.values()) {
        if (listener?.onToolConfirmation) {
          listener.onToolConfirmation(confirmationEvent);
          break; 
        }
      }
    });
    this.unlistenFunctions.push(unlisten12);
  }

  

   
  async getAvailableAgents(): Promise<string[]> {
    
    return ['general-purpose'];
  }

   
  async getActiveAgentConfigs(): Promise<AgentInfo[]> {
    const agentTypes = await agentAPI.getAvailableTools();
    
    return agentTypes.map(type => ({
      id: type,
      name: type,
      type: type,
      description: `${type} agent`,
      version: '1.0.0',
      status: 'active' as const,
      agent_type: type,
      when_to_use: `Use ${type} agent for specialized tasks`,
      tools: 'all',
      location: 'builtin'
    }));
  }

   
  async getAgentInfo(agentType: string): Promise<AgentInfo | null> {
    return agentAPI.getAgentInfo(agentType);
  }

   
  async startAgentTaskStream(
    request: AgentExecutionRequest,
    onUpdate: (event: AgentTaskUpdateEvent) => void
  ): Promise<string> {
    
    const taskId = await this.executeAgentTaskStream(request, {});
    
    
    this.taskListeners.set(taskId, onUpdate);
    
    return taskId;
  }

   
  async executeAgentTaskStream(
    request: AgentExecutionRequest,
    callbacks: {
      onChunk?: (event: StreamChunkEvent) => void;
      onToolUse?: (event: StreamToolUseEvent) => void;
      onToolResult?: (event: StreamToolResultEvent) => void;
      onProgress?: (event: StreamProgressEvent) => void;
      onComplete?: (event: StreamCompleteEvent) => void;
      onError?: (event: StreamErrorEvent) => void;
      onModelRoundStart?: (event: any) => void;
      onToolConfirmation?: (event: ToolCallConfirmationEvent) => void;
    }
  ): Promise<string> {
    
    
    let sessionId: string;
    try {
      const workspacePath = request.workspace_path;
      if (!workspacePath) {
        throw new Error('Workspace path is required to create an agent task session');
      }

      const response = await agentAPI.createSession({
        sessionName: `task-${Date.now()}`,
        agentType: request.agent_type,
        workspacePath,
        config: {
          modelName: request.model_name,
          enableTools: true,
          safeMode: true,
        }
      });
      sessionId = response.sessionId;
    } catch (error) {
      log.error('Failed to create session', error);
      throw error;
    }
    
    
    const existingListener = this.streamListeners.get(sessionId);
    if (existingListener) {
      log.warn('Session ID already has listener, will override', { sessionId });
    }
    
    
    this.streamListeners.set(sessionId, callbacks);
    
    
    try {
      const workspacePath = request.workspace_path;
      if (!workspacePath) {
        throw new Error('Workspace path is required to start an agent task');
      }

      await agentAPI.startDialogTurn({
        sessionId,
        userInput: request.prompt,
        agentType: request.agent_type,
        workspacePath,
      });
    } catch (error) {
      log.error('Failed to send message', error);
      throw error;
    }
    
    return sessionId;
  }

   
  async cancelAgentTask(taskId: string): Promise<boolean> {
    
    await agentAPI.cancelSession(taskId);
    const result = true;
    
    
    this.taskListeners.delete(taskId);
    
    return result;
  }

   
  cleanupTaskListener(taskId: string) {
    this.taskListeners.delete(taskId);
  }

  

   
  async getAllToolsInfo(): Promise<ToolInfo[]> {
    return toolAPI.getAllToolsInfo();
  }

   
  async getReadonlyToolsInfo(): Promise<ToolInfo[]> {
    
    const allTools = await toolAPI.getAllToolsInfo();
    return allTools.filter((tool: any) => tool.is_readonly === true);
  }

   
  async getToolInfo(toolName: string): Promise<ToolInfo | null> {
    return toolAPI.getToolInfo(toolName);
  }

   
  async validateToolInput(request: ToolValidationRequest): Promise<ToolValidationResponse> {
    
    const validationRequest = {
      toolName: (request as any).tool_name || (request as any).toolName,
      input: request.input || (request as any).parameters,
      workspacePath: (request as any).workspace_path || (request as any).workspacePath,
    };
    return toolAPI.validateToolInput(validationRequest);
  }

   
  async executeTool(request: ToolExecutionRequest): Promise<ToolExecutionResponse> {
    
    const executeRequest = {
      toolName: (request as any).tool_name || (request as any).toolName,
      parameters: request.input || {},
      workspacePath: (request as any).workspace_path || (request as any).workspacePath,
    };
    return toolAPI.executeTool(executeRequest);
  }

   
  async executeTask(
    description: string,
    prompt: string,
    agentType: string = 'general-purpose',
    options: {
      modelName?: string;
      workspacePath?: string;
      context?: Record<string, string>;
      safeMode?: boolean;
      verbose?: boolean;
    } = {}
  ): Promise<AgentExecutionResponse> {
    const request: AgentExecutionRequest = {
      agent_type: agentType,
      prompt,
      description,
      model_name: options.modelName,
      workspace_path: options.workspacePath,
      context: options.context,
      safe_mode: options.safeMode,
      verbose: options.verbose,
    };

    return this.executeAgentTask(request);
  }

  async executeAgentTask(request: AgentExecutionRequest): Promise<AgentExecutionResponse> {
    const sessionId = await this.executeAgentTaskStream(request, {});
    return {
      id: sessionId,
      status: 'started',
      agent_type: request.agent_type,
    };
  }

   
  async executeTaskStream(
    description: string,
    prompt: string,
    onUpdate: (event: AgentTaskUpdateEvent) => void,
    agentType: string = 'general-purpose',
    options: {
      modelName?: string;
      workspacePath?: string;
      context?: Record<string, string>;
      safeMode?: boolean;
      verbose?: boolean;
    } = {}
  ): Promise<string> {
    const request: AgentExecutionRequest = {
      agent_type: agentType,
      prompt,
      description,
      model_name: options.modelName,
      workspace_path: options.workspacePath,
      context: options.context,
      safe_mode: options.safeMode,
      verbose: options.verbose,
    };

    return this.startAgentTaskStream(request, onUpdate);
  }

   
  async executeTaskStreamNew(
    description: string,
    prompt: string,
    callbacks: {
      onChunk?: (text: string) => void;
      onToolUse?: (toolName: string, input: any) => void;
      onToolResult?: (content: string) => void;
      onProgress?: () => void;
      onComplete?: (result?: any) => void;
      onError?: (error: string) => void;
    },
    agentType: string = 'general-purpose',
    options: {
      modelName?: string;
      workspacePath?: string;
      context?: Record<string, string>;
      safeMode?: boolean;
      verbose?: boolean;
    } = {}
  ): Promise<string> {
    const request: AgentExecutionRequest = {
      agent_type: agentType,
      prompt,
      description,
      model_name: options.modelName,
      workspace_path: options.workspacePath,
      context: options.context,
      safe_mode: options.safeMode,
      verbose: options.verbose,
    };

    return this.executeAgentTaskStream(request, {
      onChunk: callbacks.onChunk ? (event) => callbacks.onChunk!(event.content) : undefined,
      onToolUse: callbacks.onToolUse ? (event) => callbacks.onToolUse!(event.tool_name, event.input) : undefined,
      onToolResult: callbacks.onToolResult ? (event) => callbacks.onToolResult!(event.content) : undefined,
      onProgress: callbacks.onProgress,
      onComplete: callbacks.onComplete ? (event) => callbacks.onComplete!(event.result) : undefined,
      onError: callbacks.onError ? (event) => callbacks.onError!(event.error) : undefined,
    });
  }

   
  async isAgentAvailable(agentType: string): Promise<boolean> {
    const availableAgents = await this.getAvailableAgents();
    return availableAgents.includes(agentType);
  }

   
  async getRecommendedAgent(_taskDescription: string): Promise<string> {
    
    return 'general-purpose';
  }

   
  async confirmToolExecution(
    sessionId: string,
    toolUseId: string, 
    action: 'confirm' | 'reject', 
    updatedInput?: any
  ): Promise<any> {
    const requestPayload = {
      sessionId,
      toolId: toolUseId,
      action,
      updatedInput: updatedInput || null
    };
    
    return toolAPI.confirmToolExecution(requestPayload);
  }
}


export const agentService = AgentService.getInstance();
