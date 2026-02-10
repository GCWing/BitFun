 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SendMessageRequest } from './tauri-commands';

export interface CreateAISessionRequest {
  session_id?: string;
  agent_type: string;
  model_name: string;
  description?: string;
}

export interface CreateAISessionResponse {
  session_id: string;
}

export interface ConnectionTestResult {
  success: boolean;
  response_time_ms: number;
  model_response?: string;
  error_details?: string;
}

export class AIApi {
   
  async listModels(): Promise<any[]> {
    try {
      return await api.invoke('list_ai_models', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('list_ai_models', error);
    }
  }

   
  async getModelInfo(modelId: string): Promise<any> {
    try {
      return await api.invoke('get_model_info', { 
        request: { modelId } 
      });
    } catch (error) {
      throw createTauriCommandError('get_model_info', error, { modelId });
    }
  }

   
  async testConnection(config: any): Promise<ConnectionTestResult> {
    try {
      return await api.invoke('test_ai_connection', { 
        request: config 
      });
    } catch (error) {
      throw createTauriCommandError('test_ai_connection', error, { config });
    }
  }

   
  async testConfigConnection(config: any): Promise<ConnectionTestResult> {
    try {
      return await api.invoke('test_ai_config_connection', { 
        request: { config } 
      });
    } catch (error) {
      throw createTauriCommandError('test_ai_config_connection', error, { config });
    }
  }

   
  async sendMessage(request: SendMessageRequest): Promise<any> {
    try {
      return await api.invoke('send_ai_message', { 
        request 
      });
    } catch (error) {
      throw createTauriCommandError('send_ai_message', error, request);
    }
  }

   
  async initializeAI(config: any): Promise<void> {
    try {
      await api.invoke('initialize_ai', { 
        request: { config } 
      });
    } catch (error) {
      throw createTauriCommandError('initialize_ai', error, { config });
    }
  }

   
  async testAIConfigConnection(config: any): Promise<ConnectionTestResult> {
    try {
      return await api.invoke('test_ai_config_connection', { 
        request: { config } 
      });
    } catch (error) {
      throw createTauriCommandError('test_ai_config_connection', error, { config });
    }
  }

   
  async createAISession(config: CreateAISessionRequest): Promise<CreateAISessionResponse> {
    try {
      return await api.invoke('create_ai_session', { 
        request: config 
      });
    } catch (error) {
      throw createTauriCommandError('create_ai_session', error, { config });
    }
  }

   
  async invokeAICommand<T = any>(command: string, config: any, additionalArgs?: Record<string, any>): Promise<T> {
    try {
      const args = {
        config,
        ...additionalArgs
      };
      return await api.invoke(command, args);
    } catch (error) {
      throw createTauriCommandError(command, error, { config, additionalArgs });
    }
  }

   
  async fixMermaidCode(request: { sourceCode: string; errorMessage: string }): Promise<string> {
    try {
      return await api.invoke('fix_mermaid_code', { 
        request 
      });
    } catch (error) {
      throw createTauriCommandError('fix_mermaid_code', error, request);
    }
  }
}


export const aiApi = new AIApi();