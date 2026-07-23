 

import { createLogger } from '../../../shared/utils/logger';
import { agentAPI } from '../../api';
import { i18nService } from '@/infrastructure/i18n';

const logger = createLogger('AgentService');


type AgentType = 'project_qa' | 'requirement_clarification' | 'core';

export interface AgentResponse {
  content: string;
  metadata: Record<string, any>;
}

export interface AgentCallOptions {
  agentType: AgentType;
  message: string;
  workspacePath?: string; 
}

 
export interface AgentExecutionRequest {
  agent_type: string;
  prompt: string;
  model_name?: string;
  workspace_path?: string;
  context?: Record<string, string>;
  verbose?: boolean;
}

 
class SessionManager {
  private sessions = new Map<string, string>(); // workspacePath::agentType -> sessionId

  private buildKey(agentType: string, workspacePath: string): string {
    return `${workspacePath}::${agentType}`;
  }

  getSession(agentType: string, workspacePath: string): string | undefined {
    return this.sessions.get(this.buildKey(agentType, workspacePath));
  }

  setSession(agentType: string, workspacePath: string, sessionId: string): void {
    this.sessions.set(this.buildKey(agentType, workspacePath), sessionId);
  }

  deleteSession(agentType: string, workspacePath: string): void {
    this.sessions.delete(this.buildKey(agentType, workspacePath));
  }

  clear(): void {
    this.sessions.clear();
  }
}

export class AgentService {
  private static sessionManager = new SessionManager();

   
  static async getOrCreateSession(agentType: string, workspacePath: string, modelName?: string): Promise<string> {
    
    const existingSessionId = this.sessionManager.getSession(agentType, workspacePath);
    if (existingSessionId) {
      logger.debug(`Using existing session: ${existingSessionId}`);
      return existingSessionId;
    }

    
    logger.info(`Creating new session: ${agentType}`);

    try {
      const response = await agentAPI.createSession({
        sessionName: `${agentType}-session-${Date.now()}`,
        agentType,
        workspacePath,
        config: {
          modelName,
          enableTools: true,
          safeMode: true,
          autoCompact: true,
          enableContextCompression: true,
        }
      });
      this.sessionManager.setSession(agentType, workspacePath, response.sessionId);
      logger.info(`Session created: ${response.sessionId}`);
      return response.sessionId;
    } catch (error) {
      logger.error('Failed to create session', error);
      throw error;
    }
  }

   
  static async executeAgentTaskStream(
    request: AgentExecutionRequest,
    callbacks: {
      onModelRoundStart?: (event: any) => void;
      onTextChunk?: (event: any) => void;
      onToolCall?: (event: any) => void;
      onToolResult?: (event: any) => void;
      onToolConfirmation?: (event: any) => void;
      onProgress?: (event: any) => void;
      onComplete?: (event: any) => void;
      onError?: (error: any) => void;
    }
  ): Promise<string> {
    logger.info('Executing agent task flow', {
      agentType: request.agent_type,
      hasContext: !!request.context
    });

    try {
      
      const workspacePath = request.workspace_path;
      if (!workspacePath) {
        throw new Error('Workspace path is required to start an agent task');
      }
      const sessionId = await this.getOrCreateSession(request.agent_type, workspacePath, request.model_name);

      
      const unlistenFunctions: Array<() => void> = [];

      
      if (callbacks.onTextChunk) {
        const unlisten = await agentAPI.onTextChunk((event) => {
          if (event.sessionId === sessionId) {
            callbacks.onTextChunk?.(event);
          }
        });
        unlistenFunctions.push(unlisten);
      }

      
      if (callbacks.onModelRoundStart) {
        const unlisten = await agentAPI.onModelRoundStarted((event) => {
          if (event.sessionId === sessionId) {
            callbacks.onModelRoundStart?.(event);
          }
        });
        unlistenFunctions.push(unlisten);
      }

      
      if (callbacks.onToolCall || callbacks.onToolResult || callbacks.onToolConfirmation) {
        const unlisten = await agentAPI.onToolEvent((event) => {
          if (event.sessionId === sessionId) {
            const toolEvent = event.toolEvent;
            
            
            if (toolEvent.Started || toolEvent.EarlyDetected) {
              callbacks.onToolCall?.(toolEvent);
            } else if (toolEvent.Completed || toolEvent.Failed) {
              callbacks.onToolResult?.(toolEvent);
            } else if (toolEvent.ConfirmationNeeded) {
              callbacks.onToolConfirmation?.(toolEvent);
            } else if (toolEvent.Progress || toolEvent.StreamChunk) {
              callbacks.onProgress?.(toolEvent);
            }
          }
        });
        unlistenFunctions.push(unlisten);
      }

      
      if (callbacks.onComplete) {
        const unlisten = await agentAPI.onDialogTurnCompleted((event) => {
          if (event.sessionId === sessionId) {
            callbacks.onComplete?.(event);
            
            unlistenFunctions.forEach(fn => fn());
          }
        });
        unlistenFunctions.push(unlisten);
      }

      
      await agentAPI.startDialogTurn({
        sessionId,
        userInput: request.prompt,
        agentType: request.agent_type,
        workspacePath,
      });

      
      return sessionId;
    } catch (error) {
      logger.error('Agent task flow failed', error);
      callbacks.onError?.(error);
      throw error;
    }
  }

   
  static async cancelAgentTask(taskId: string): Promise<void> {
    try {
      await agentAPI.cancelSession(taskId);
      logger.info(`Task cancelled: ${taskId}`);
    } catch (error) {
      logger.error('Failed to cancel task', error);
      throw error;
    }
  }

   
  static async getAgentHealth(agentType: AgentType): Promise<{ healthy: boolean; name: string; description: string }> {
    return {
      healthy: true,
      name: this.getAgentDisplayName(agentType),
      description: this.getAgentDescription(agentType)
    };
  }

   
  private static getAgentDisplayName(agentType: AgentType): string {
    const nameMap: Record<AgentType, string> = {
      'project_qa': i18nService.t('common:agents.projectQa.name'),
      'requirement_clarification': i18nService.t('common:agents.requirementClarification.name'),
      'core': i18nService.t('common:agents.core.name')
    };
    return nameMap[agentType] || agentType;
  }

   
  private static getAgentDescription(agentType: AgentType): string {
    const descMap: Record<AgentType, string> = {
      'project_qa': i18nService.t('common:agents.projectQa.description'),
      'requirement_clarification': i18nService.t('common:agents.requirementClarification.description'),
      'core': i18nService.t('common:agents.core.description')
    };
    return descMap[agentType] || i18nService.t('common:agents.general.description');
  }

   
   
  static requiresSpecialVisualization(agentType: AgentType, metadata?: Record<string, any>): boolean {
    if (agentType === 'requirement_clarification') {
      
      return !!(metadata?.interactive_sections && Array.isArray(metadata.interactive_sections));
    }
    
    return false;
  }


}
export default AgentService;
