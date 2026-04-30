 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { ImageContextData as ImageInputContextData } from './ImageContextTypes';



export interface SessionTitleGeneratedEvent {
  sessionId: string;
  title: string;
  method: 'ai' | 'fallback';
  timestamp: number;
}

export interface SessionModelAutoMigratedEvent {
  sessionId: string;
  previousModelId: string;
  newModelId: string;
  reason: string;
}

 
export interface SessionConfig {
  modelName?: string;
  maxContextTokens?: number;
  autoCompact?: boolean;
  enableTools?: boolean;
  safeMode?: boolean;
  maxTurns?: number;
  enableContextCompression?: boolean;
  compressionThreshold?: number;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

 
export interface CreateSessionRequest {
  sessionId?: string; 
  sessionName: string;
  agentType: string;
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  config?: SessionConfig;
}

 
export interface CreateSessionResponse {
  sessionId: string;
  sessionName: string;
  agentType: string;
}

 
export interface StartDialogTurnRequest {
  sessionId: string;
  userInput: string;
  originalUserInput?: string;
  turnId?: string; 
  agentType: string; 
  workspacePath?: string;
  /** Optional multimodal image contexts (snake_case fields, aligned with backend ImageContextData). */
  imageContexts?: ImageInputContextData[];
  userMessageMetadata?: Record<string, unknown>;
}

export interface CompactSessionRequest {
  sessionId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

 
export interface SessionInfo {
  sessionId: string;
  sessionName: string;
  agentType: string;
  state: string;
  turnCount: number;
  createdAt: number;
}

export interface EnsureAssistantBootstrapRequest {
  sessionId: string;
  workspacePath: string;
}

export type EnsureAssistantBootstrapStatus = 'started' | 'skipped' | 'blocked';

export type EnsureAssistantBootstrapReason =
  | 'bootstrap_started'
  | 'bootstrap_not_required'
  | 'session_has_existing_turns'
  | 'session_not_idle'
  | 'model_unavailable';

export interface EnsureAssistantBootstrapResponse {
  status: EnsureAssistantBootstrapStatus;
  reason: EnsureAssistantBootstrapReason;
  sessionId: string;
  turnId?: string;
  detail?: string;
}

export interface UpdateSessionModelRequest {
  sessionId: string;
  modelName: string;
}

export interface UpdateSessionTitleRequest {
  sessionId: string;
  title: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

 
export interface ModeInfo {
  id: string;
  name: string;
  description: string;
  isReadonly: boolean;
  toolCount: number;
  defaultTools?: string[];
  enabled: boolean;
}



export interface SubagentParentInfo {
  toolCallId: string;
  sessionId: string;
  dialogTurnId: string;
}

export interface AgenticEvent {
  sessionId: string;
  turnId?: string;
  subagentParentInfo?: SubagentParentInfo;
  [key: string]: any;
}

export interface TextChunkEvent extends AgenticEvent {
  roundId: string;
  text: string;
  contentType?: 'text' | 'thinking';
  isThinkingEnd?: boolean;
  subagentParentInfo?: SubagentParentInfo;
}

export interface ToolEvent extends AgenticEvent {
  toolEvent: any;
  subagentParentInfo?: SubagentParentInfo;
}

 
export interface ImageAnalysisEvent extends AgenticEvent {
  imageCount?: number;
  userInput?: string;
  success?: boolean;
  durationMs?: number;
}

export interface CompressionEvent extends AgenticEvent {
  compressionId: string;          
  
  trigger?: string;                // "auto" | "manual" | "user_message"
  tokensBefore?: number;           
  contextWindow?: number;          
  threshold?: number;              
  
  compressionCount?: number;       
  tokensAfter?: number;            
  compressionRatio?: number;       
  durationMs?: number;             
  hasSummary?: boolean;            
  summarySource?: 'model' | 'local_fallback' | 'none';
  
  error?: string;                  
  subagentParentInfo?: SubagentParentInfo;
}



export class AgentAPI {
  
  

  

   
  async createSession(request: CreateSessionRequest): Promise<CreateSessionResponse> {
    try {
      return await api.invoke<CreateSessionResponse>('create_session', { request });
    } catch (error) {
      throw createTauriCommandError('create_session', error, request);
    }
  }

   
  async startDialogTurn(request: StartDialogTurnRequest): Promise<{ success: boolean; message: string }> {
    try {
      return await api.invoke<{ success: boolean; message: string }>('start_dialog_turn', { request });
    } catch (error) {
      throw createTauriCommandError('start_dialog_turn', error, request);
    }
  }

  async compactSession(request: CompactSessionRequest): Promise<{ success: boolean; message: string }> {
    try {
      return await api.invoke<{ success: boolean; message: string }>('compact_session', { request });
    } catch (error) {
      throw createTauriCommandError('compact_session', error, request);
    }
  }

  async ensureAssistantBootstrap(
    request: EnsureAssistantBootstrapRequest
  ): Promise<EnsureAssistantBootstrapResponse> {
    try {
      return await api.invoke<EnsureAssistantBootstrapResponse>('ensure_assistant_bootstrap', {
        request
      });
    } catch (error) {
      throw createTauriCommandError('ensure_assistant_bootstrap', error, request);
    }
  }

   
  async cancelDialogTurn(sessionId: string, dialogTurnId: string): Promise<void> {
    try {
      await api.invoke<void>('cancel_dialog_turn', { request: { sessionId, dialogTurnId } });
    } catch (error) {
      throw createTauriCommandError('cancel_dialog_turn', error, { sessionId, dialogTurnId });
    }
  }

   
  async deleteSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<void> {
    try {
      await api.invoke<void>('delete_session', { 
        request: { sessionId, workspacePath, remoteConnectionId, remoteSshHost } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_session', error, { sessionId, workspacePath });
    }
  }

   
  async restoreSession(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<SessionInfo> {
    try {
      return await api.invoke<SessionInfo>('restore_session', {
        request: { sessionId, workspacePath, remoteConnectionId, remoteSshHost },
      });
    } catch (error) {
      throw createTauriCommandError('restore_session', error, { sessionId, workspacePath });
    }
  }

  /**
   * No-op if the session is already in the coordinator; otherwise loads it from disk
   * using the same workspace path resolution as restore_session (required for SSH remote workspaces).
   */
  async ensureCoordinatorSession(request: {
    sessionId: string;
    workspacePath: string;
    remoteConnectionId?: string;
    remoteSshHost?: string;
  }): Promise<void> {
    try {
      await api.invoke<void>('ensure_coordinator_session', { request });
    } catch (error) {
      throw createTauriCommandError('ensure_coordinator_session', error, request);
    }
  }

  async updateSessionModel(request: UpdateSessionModelRequest): Promise<void> {
    try {
      await api.invoke<void>('update_session_model', { request });
    } catch (error) {
      throw createTauriCommandError('update_session_model', error, request);
    }
  }

  async updateSessionTitle(request: UpdateSessionTitleRequest): Promise<string> {
    try {
      return await api.invoke<string>('update_session_title', { request });
    } catch (error) {
      throw createTauriCommandError('update_session_title', error, request);
    }
  }


   
  async listSessions(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): Promise<SessionInfo[]> {
    try {
      return await api.invoke<SessionInfo[]>('list_sessions', {
        request: { workspacePath, remoteConnectionId, remoteSshHost },
      });
    } catch (error) {
      throw createTauriCommandError('list_sessions', error, { workspacePath });
    }
  }

  async confirmToolExecution(sessionId: string, toolId: string): Promise<void> {
    try {
      await api.invoke<void>('confirm_tool_execution', {
        request: {
          sessionId,
          toolId
        }
      });
    } catch (error) {
      throw createTauriCommandError('confirm_tool_execution', error, { sessionId, toolId });
    }
  }

   
  async rejectToolExecution(sessionId: string, toolId: string, reason?: string): Promise<void> {
    try {
      await api.invoke<void>('reject_tool_execution', {
        request: {
          sessionId,
          toolId,
          reason
        }
      });
    } catch (error) {
      throw createTauriCommandError('reject_tool_execution', error, { sessionId, toolId, reason });
    }
  }
  

   
  onSessionCreated(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://session-created', callback);
  }

  onSessionDeleted(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://session-deleted', callback);
  }

  onSessionStateChanged(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://session-state-changed', callback);
  }

  onSessionModelAutoMigrated(
    callback: (event: SessionModelAutoMigratedEvent) => void
  ): () => void {
    return api.listen<SessionModelAutoMigratedEvent>(
      'agentic://session-model-auto-migrated',
      callback
    );
  }

   
  onDialogTurnStarted(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://dialog-turn-started', callback);
  }

   
  onModelRoundStarted(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://model-round-started', callback);
  }

   
  onTextChunk(callback: (event: TextChunkEvent) => void): () => void {
    return api.listen<TextChunkEvent>('agentic://text-chunk', callback);
  }

   
  onToolEvent(callback: (event: ToolEvent) => void): () => void {
    return api.listen<ToolEvent>('agentic://tool-event', callback);
  }

   
  onDialogTurnCompleted(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://dialog-turn-completed', callback);
  }

   
  onDialogTurnFailed(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://dialog-turn-failed', callback);
  }

   
  onDialogTurnCancelled(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://dialog-turn-cancelled', callback);
  }

   
  onTokenUsageUpdated(callback: (event: AgenticEvent) => void): () => void {
    return api.listen<AgenticEvent>('agentic://token-usage-updated', callback);
  }

   
  onContextCompressionStarted(callback: (event: CompressionEvent) => void): () => void {
    return api.listen<CompressionEvent>('agentic://context-compression-started', callback);
  }

   
  onContextCompressionCompleted(callback: (event: CompressionEvent) => void): () => void {
    return api.listen<CompressionEvent>('agentic://context-compression-completed', callback);
  }

   
  onContextCompressionFailed(callback: (event: CompressionEvent) => void): () => void {
    return api.listen<CompressionEvent>('agentic://context-compression-failed', callback);
  }

  onImageAnalysisStarted(callback: (event: ImageAnalysisEvent) => void): () => void {
    return api.listen<ImageAnalysisEvent>('agentic://image-analysis-started', callback);
  }

  onImageAnalysisCompleted(callback: (event: ImageAnalysisEvent) => void): () => void {
    return api.listen<ImageAnalysisEvent>('agentic://image-analysis-completed', callback);
  }

   
  async getAvailableTools(): Promise<string[]> {
    try {
      return await api.invoke<string[]>('get_available_tools');
    } catch (error) {
      throw createTauriCommandError('get_available_tools', error);
    }
  }

  async getDefaultReviewTeamDefinition(): Promise<unknown> {
    try {
      return await api.invoke<unknown>('get_default_review_team_definition');
    } catch (error) {
      throw createTauriCommandError('get_default_review_team_definition', error);
    }
  }

  async generateSessionTitle(
    sessionId: string,
    userMessage: string,
    maxLength?: number
  ): Promise<string> {
    try {
      return await api.invoke<string>('generate_session_title', {
        request: {
          sessionId,
          userMessage,
          maxLength: maxLength || 20
        }
      });
    } catch (error) {
      throw createTauriCommandError('generate_session_title', error, {
        sessionId,
        userMessage,
        maxLength
      });
    }
  }

   
  onSessionTitleGenerated(
    callback: (event: SessionTitleGeneratedEvent) => void
  ): () => void {
    return api.listen<SessionTitleGeneratedEvent>('session_title_generated', callback);
  }

  async cancelSession(sessionId: string): Promise<void> {
    try {
      await api.invoke<void>('cancel_session', {
        request: { sessionId }
      });
    } catch (error) {
      throw createTauriCommandError('cancel_session', error, { sessionId });
    }
  }

  async setSubagentTimeout(
    sessionId: string,
    action: { type: 'disable' } | { type: 'restore' } | { type: 'extend'; seconds: number },
  ): Promise<void> {
    const actionPayload = action.type === 'disable'
      ? 'Disable'
      : action.type === 'restore'
        ? 'Restore'
        : { Extend: { seconds: action.seconds } };
    try {
      await api.invoke<void>('set_subagent_timeout', {
        request: { session_id: sessionId, action: actionPayload },
      });
    } catch (error) {
      throw createTauriCommandError('set_subagent_timeout', error, { sessionId, action: action.type });
    }
  }

  async getAgentInfo(agentType: string): Promise<ModeInfo & { agent_type: string; when_to_use: string; tools: string; location: string }> {
    return {
      id: agentType,
      name: agentType,
      description: `${agentType} agent`,
      isReadonly: false,
      toolCount: 0,
      enabled: true,
      agent_type: agentType,
      when_to_use: `Use ${agentType} for related tasks`,
      tools: 'all',
      location: 'builtin',
    };
  }

  

   
  async getAvailableModes(): Promise<ModeInfo[]> {
    try {
      return await api.invoke<ModeInfo[]>('get_available_modes');
    } catch (error) {
      throw createTauriCommandError('get_available_modes', error);
    }
  }

}


export const agentAPI = new AgentAPI();
