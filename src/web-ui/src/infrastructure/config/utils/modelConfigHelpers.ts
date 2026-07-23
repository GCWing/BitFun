 

import { configManager } from '../services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ModelConfigHelper');

 
export async function getDefaultPrimaryModel(): Promise<string | null> {
  try {
    const defaultModels = await configManager.getConfig<any>('ai.default_models');
    const primaryModelId = defaultModels?.primary;

    if (!primaryModelId) {
      log.warn('Default primary model not configured');
      return null;
    }

    
    const allModels = await configManager.getConfig<any[]>('ai.models') || [];
    const targetModel = allModels.find(m => m.id === primaryModelId);

    if (!targetModel) {
      log.error('Model configuration not found', { 
        primaryModelId, 
        availableIds: allModels.map(m => m.id) 
      });
    }

    return primaryModelId;
  } catch (error) {
    log.error('Failed to get default model configuration', error);
    return null;
  }
}

