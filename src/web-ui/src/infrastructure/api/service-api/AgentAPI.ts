 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { ImageContextData as ImageInputContextData } from './ImageAnalysisAPI';



export interface SessionTitleGeneratedEvent {
  sessionId: string;
  title: string;
  method: 'ai' | 'fallback';
  timestamp: number;
}

 
export interface SessionConfig {
  maxContextTokens?: number;
  autoCompact?: boolean;
  enableTools?: boolean;
  safeMode?: boolean;
  maxTurns?: number;
  enableContextCompression?: boolean;
  compressionThreshold?: number;
}

 
export interface CreateSessionRequest {
  sessionId?: string; 
  sessionName: string;
  agentType: string;
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
  turnId?: string; 
  agentType: string; 
  /** Optional multimodal image contexts (snake_case fields, aligned with backend ImageContextData). */
  imageContexts?: ImageInputContextData[];
}

 
export interface SessionInfo {
  sessionId: string;
  sessionName: string;
  agentType: string;
  state: string;
  turnCount: number;
  createdAt: number;
}

 
export interface Message {
  id: string;
  role: 'user' | 'assistant' | 'tool' | 'system';
  content: any;
  timestamp: number;
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
  subagentParentInfo?: SubagentParentInfo;
}

export interface ToolEvent extends AgenticEvent {
  toolEvent: any;
  subagentParentInfo?: SubagentParentInfo;
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

   
  async cancelDialogTurn(sessionId: string, dialogTurnId: string): Promise<void> {
    try {
      await api.invoke<void>('cancel_dialog_turn', { request: { sessionId, dialogTurnId } });
    } catch (error) {
      throw createTauriCommandError('cancel_dialog_turn', error, { sessionId, dialogTurnId });
    }
  }

   
  async deleteSession(sessionId: string): Promise<void> {
    try {
      await api.invoke<void>('delete_session', { 
        request: { sessionId } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_session', error, { sessionId });
    }
  }

   
  async restoreSession(sessionId: string): Promise<SessionInfo> {
    try {
      return await api.invoke<SessionInfo>('restore_session', { request: { sessionId } });
    } catch (error) {
      throw createTauriCommandError('restore_session', error, { sessionId });
    }
  }

   
  async listSessions(): Promise<SessionInfo[]> {
    try {
      return await api.invoke<SessionInfo[]>('list_sessions');
    } catch (error) {
      throw createTauriCommandError('list_sessions', error);
    }
  }

   
  async getSessionMessages(sessionId: string, limit?: number): Promise<Message[]> {
    try {
      return await api.invoke<Message[]>('get_session_messages', {
        request: {
          sessionId,
          limit
        }
      });
    } catch (error) {
      throw createTauriCommandError('get_session_messages', error, { sessionId, limit });
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

   
  async getAvailableTools(): Promise<string[]> {
    try {
      return await api.invoke<string[]>('get_available_tools');
    } catch (error) {
      throw createTauriCommandError('get_available_tools', error);
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

  

   
  async getAvailableModes(): Promise<ModeInfo[]> {
    try {
      return await api.invoke<ModeInfo[]>('get_available_modes');
    } catch (error) {
      throw createTauriCommandError('get_available_modes', error);
    }
  }

}


export const agentAPI = new AgentAPI();
