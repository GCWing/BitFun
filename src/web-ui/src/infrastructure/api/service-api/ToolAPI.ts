 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type {
  ExecuteToolRequest,
  GetToolInfoRequest,
  ValidateToolInputRequest
} from './tauri-commands';
import { createLogger } from '@/shared/utils/logger';
import type { ToolInfo } from '@/shared/types/agent-api';

const log = createLogger('ToolAPI');

export class ToolAPI {
   
  async getAllToolsInfo(): Promise<ToolInfo[]> {
    try {
      return await api.invoke('get_all_tools_info');
    } catch (error) {
      throw createTauriCommandError('get_all_tools_info', error);
    }
  }

   
  async getToolInfo(toolName: string): Promise<ToolInfo | null> {
    try {
      const request: GetToolInfoRequest = { toolName };
      return await api.invoke('get_tool_info', { 
        request
      });
    } catch (error) {
      throw createTauriCommandError('get_tool_info', error, { toolName });
    }
  }

   
  async validateToolInput(request: ValidateToolInputRequest): Promise<any> {
    try {
      return await api.invoke('validate_tool_input', { 
        request
      });
    } catch (error) {
      throw createTauriCommandError('validate_tool_input', error, request);
    }
  }

   
  async executeTool(request: ExecuteToolRequest): Promise<any> {
    try {
      return await api.invoke('execute_tool', { 
        request: {
          toolName: request.toolName,
          input: request.parameters,
          workspacePath: request.workspacePath,
        }
      });
    } catch (error) {
      throw createTauriCommandError('execute_tool', error, request);
    }
  }

   
  async confirmToolExecution(request: any): Promise<any> {
    try {
      const action = request.action || 'confirm';
      
      if (action === 'confirm') {
        
        const confirmRequest = {
          sessionId: request.sessionId,
          toolId: request.toolId,
          updatedInput: request.updatedInput || null
        };
        
        const result = await api.invoke('confirm_tool_execution', { request: confirmRequest });
        return result;
      } else if (action === 'reject') {
        
        const rejectRequest = {
          sessionId: request.sessionId,
          toolId: request.toolId,
          reason: 'User rejected'
        };
        
        const result = await api.invoke('reject_tool_execution', { request: rejectRequest });
        return result;
      } else {
        throw new Error(`Unknown action type: ${action}`);
      }
    } catch (error) {
      log.error('Tool confirmation/rejection failed', { 
        action: request.action, 
        sessionId: request.sessionId, 
        toolId: request.toolId,
        error
      });
      throw createTauriCommandError(
        request.action === 'reject' ? 'reject_tool_execution' : 'confirm_tool_execution', 
        error, 
        request
      );
    }
  }

  /**
   * Submit user answers.
   */
  async submitUserAnswers(toolId: string, answers: Record<string, string | string[]>): Promise<void> {
    try {
      await api.invoke('submit_user_answers', { 
        toolId,
        answers 
      });
    } catch (error) {
      log.error('Failed to submit user answers', { toolId, error });
      throw createTauriCommandError('submit_user_answers', error, { toolId, answers });
    }
  }
}


export const toolAPI = new ToolAPI();
