 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { SendMessageRequest } from './tauri-commands';
import type { ConnectionTestMessageCode } from '@/shared/utils/aiConnectionTestMessages';

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
  message_code?: ConnectionTestMessageCode;
  error_details?: string;
}

export interface RemoteModelInfo {
  id: string;
  display_name?: string;
}

export type CliCredentialKind = 'codex' | 'gemini';
export type CliCredentialMode = 'api_key' | 'chat_gpt' | 'oauth_personal';

export interface DiscoveredCliCredential {
  kind: CliCredentialKind;
  mode: CliCredentialMode;
  display_label: string;
  account?: string | null;
  expires_at?: number | null;
  source_path: string;
  suggested_format: string;
  suggested_base_url: string;
  suggested_model: string;
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

  async listModelsByConfig(config: any): Promise<RemoteModelInfo[]> {
    try {
      return await api.invoke<RemoteModelInfo[]>('list_ai_models_by_config', {
        request: { config }
      });
    } catch (error) {
      throw createTauriCommandError('list_ai_models_by_config', error, { config });
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

   
  async discoverCliCredentials(): Promise<DiscoveredCliCredential[]> {
    try {
      return await api.invoke<DiscoveredCliCredential[]>('discover_cli_credentials', {});
    } catch (error) {
      throw createTauriCommandError('discover_cli_credentials', error);
    }
  }

  async refreshCliCredential(kind: CliCredentialKind): Promise<DiscoveredCliCredential> {
    try {
      return await api.invoke<DiscoveredCliCredential>('refresh_cli_credential', {
        request: { kind }
      });
    } catch (error) {
      throw createTauriCommandError('refresh_cli_credential', error, { kind });
    }
  }
}

export const aiApi = new AIApi();