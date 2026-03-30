 

import { ToolExecutionEvent, ModelConfig } from '../../../shared/types';
import { aiApi } from '../../api';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('AIService');
// ToolExecution types are handled by backend now

interface AIServiceOptions {
  sessionId: string;
  conversationId: string;
  onToolExecution?: (event: ToolExecutionEvent) => void;
}

class AIService {
  private static isInitialized = false;
  private static currentConfig: ModelConfig | null = null;
  private static initializationPromise: Promise<void> | null = null;
  private static autoInitializeAttempted = false;

  
  static isAIInitialized(): boolean {
    return AIService.isInitialized;
  }

  
  static getCurrentConfig(): ModelConfig | null {
    return AIService.currentConfig;
  }

  
  static async initializeAI(config: ModelConfig): Promise<void> {
    log.info('Initializing AI client', { name: config.name });
    
    
    if (AIService.initializationPromise) {
      return AIService.initializationPromise;
    }

    
    if (AIService.isInitialized && AIService.currentConfig?.id === config.id) {
      return Promise.resolve();
    }

    AIService.initializationPromise = this.doInitialize(config);
    return AIService.initializationPromise;
  }

  private static async doInitialize(config: ModelConfig): Promise<void> {
    try {
      const backendConfig = {
        name: config.name,
        model: config.modelName,
        api_key: config.apiKey || '',
        base_url: config.baseUrl
      };
      
      await aiApi.initializeAI(backendConfig);
      
      
      AIService.currentConfig = config;
      AIService.isInitialized = true;
      AIService.initializationPromise = null;
      
      
      window.dispatchEvent(new CustomEvent('ai:initialized', {
        detail: { config }
      }));
      
      log.info('AI client initialized');
    } catch (error) {
      AIService.isInitialized = false;
      AIService.initializationPromise = null;
      
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      
      window.dispatchEvent(new CustomEvent('ai:error', {
        detail: { error: errorMessage }
      }));
      
      
      notificationService.warning(errorMessage, {
        title: i18nService.t('errors:ai.initializeFailedTitle'),
        duration: 8000,
        closable: true
      });
      
      log.error('AI client initialization failed', error);
      throw error;
    }
  }

  
  static async autoInitialize(config: ModelConfig): Promise<void> {
    
    if (AIService.autoInitializeAttempted && AIService.isInitialized) {
      return Promise.resolve();
    }
    
    AIService.autoInitializeAttempted = true;
    return this.initializeAI(config);
  }

  
  static async ensureInitialized(config?: ModelConfig): Promise<void> {
    if (AIService.isInitialized) {
      return Promise.resolve();
    }

    if (!config) {
      throw new Error('AI client is not initialized and no configuration was provided');
    }

    return this.initializeAI(config);
  }

  
  static reset(): void {
    AIService.isInitialized = false;
    AIService.currentConfig = null;
    AIService.initializationPromise = null;
    AIService.autoInitializeAttempted = false;
    
    
    window.dispatchEvent(new CustomEvent('ai:reset'));
  }

  async sendMessage(content: string, options: AIServiceOptions): Promise<{ response: string, sessionId: string }> {
    const { onToolExecution: _onToolExecution } = options;

    try {
      
      const workspacePath = await this.getCurrentWorkspacePath();

      
      
      // Backend expects: content (string), workspace_path (Option<String>)
      const result = await aiApi.sendMessage({
        message: content,
        context: workspacePath ? { workspacePath } : undefined,
      });

      return {
        response: result || '',
        sessionId: `session_${Date.now()}`
      };

    } catch (error) {
      log.error('Failed to send message', error);
      throw new Error(`AI service is temporarily unavailable. Details: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  
  private async getCurrentWorkspacePath(): Promise<string | undefined> {
    try {
      
      const { globalStateAPI } = await import('../../../shared/types/global-state');
      const workspace = await globalStateAPI.getCurrentWorkspace();
      
      if (workspace && workspace.rootPath) {
        return workspace.rootPath;
      }
      
      return undefined;
    } catch (error) {
      log.warn('Failed to get workspace path', error);
      return undefined;
    }
  }

  async cancelRequest(_requestId: string): Promise<void> {
    
  }
}

export const aiService = new AIService();
export { AIService };
