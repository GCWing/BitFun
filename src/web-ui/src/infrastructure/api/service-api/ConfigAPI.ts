 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type {
  ModeSkillInfo,
  ModeConfigItem,
  RuntimeLoggingInfo,
  SkillInfo,
  SkillLevel,
  SkillMarketDownloadResult,
  SkillMarketItem,
  SkillValidationResult,
} from '../../config/types';

export interface GetSkillConfigsParams {
  forceRefresh?: boolean;
  workspacePath?: string;
}

export interface GetModeSkillConfigsParams {
  modeId: string;
  forceRefresh?: boolean;
  workspacePath?: string;
}

export interface SetModeSkillDisabledParams {
  modeId: string;
  skillKey: string;
  disabled: boolean;
  workspacePath?: string;
}

export interface ReplaceModeSkillSelectionParams {
  modeId: string;
  enabledSkillKeys: string[];
  workspacePath?: string;
}

export interface AddSkillParams {
  sourcePath: string;
  level: SkillLevel;
  workspacePath?: string;
}

export interface DeleteSkillParams {
  skillKey: string;
  workspacePath?: string;
}

export interface DownloadSkillMarketParams {
  packageId: string;
  level?: SkillLevel;
  workspacePath?: string;
}


export class ConfigAPI {
   
  async getConfig(path?: string, options?: { skipRetryOnNotFound?: boolean }): Promise<any> {
    try {
      
      const shouldSkipRetry = options?.skipRetryOnNotFound ?? false;
      
      return await api.invoke('get_config', 
        { request: path ? { path } : {} },
        shouldSkipRetry ? { retries: 0 } : undefined
      );
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      const normalized = errorMessage.toLowerCase();
      if (
        normalized.includes('not found:') &&
        normalized.includes('config path') &&
        normalized.includes(`'${path}'`)
      ) {
        return undefined;
      }
      throw createTauriCommandError('get_config', error, { path });
    }
  }

   
  async setConfig(path: string, value: any): Promise<void> {
    try {
      await api.invoke('set_config', { 
        request: { path, value } 
      });
    } catch (error) {
      throw createTauriCommandError('set_config', error, { path, value });
    }
  }

   
  async resetConfig(path?: string): Promise<void> {
    try {
      await api.invoke('reset_config', { 
        request: path ? { path } : {} 
      });
    } catch (error) {
      throw createTauriCommandError('reset_config', error, { path });
    }
  }

   
  async exportConfig(): Promise<any> {
    try {
      return await api.invoke('export_config', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('export_config', error);
    }
  }

   
  async importConfig(configData: any): Promise<void> {
    try {
      await api.invoke('import_config', { 
        request: { configData } 
      });
    } catch (error) {
      throw createTauriCommandError('import_config', error, { configData });
    }
  }

   
  async reloadConfig(): Promise<void> {
    try {
      await api.invoke('reload_config', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('reload_config', error);
    }
  }

  async getRuntimeLoggingInfo(): Promise<RuntimeLoggingInfo> {
    try {
      return await api.invoke('get_runtime_logging_info', {
        request: {},
      });
    } catch (error) {
      throw createTauriCommandError('get_runtime_logging_info', error);
    }
  }

   
  async getModelConfigs(): Promise<any[]> {
    try {
      return await api.invoke('get_model_configs', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_model_configs', error);
    }
  }

   
  async saveModelConfig(config: any): Promise<void> {
    try {
      await api.invoke('save_model_config', { 
        request: { config } 
      });
    } catch (error) {
      throw createTauriCommandError('save_model_config', error, { config });
    }
  }

   
  async deleteModelConfig(configId: string): Promise<void> {
    try {
      await api.invoke('delete_model_config', { 
        request: { configId } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_model_config', error, { configId });
    }
  }

  

   
  async getModeConfigs(): Promise<Record<string, ModeConfigItem>> {
    try {
      return await api.invoke<Record<string, ModeConfigItem>>('get_mode_configs');
    } catch (error) {
      throw createTauriCommandError('get_mode_configs', error);
    }
  }

   
  async getModeConfig(modeId: string): Promise<ModeConfigItem> {
    try {
      return await api.invoke<ModeConfigItem>('get_mode_config', { modeId });
    } catch (error) {
      throw createTauriCommandError('get_mode_config', error, { modeId });
    }
  }

   
  async setModeConfig(modeId: string, config: any): Promise<string> {
    try {
      return await api.invoke('set_mode_config', { modeId, config });
    } catch (error) {
      throw createTauriCommandError('set_mode_config', error, { modeId, config });
    }
  }

   
  async resetModeConfig(modeId: string): Promise<string> {
    try {
      return await api.invoke('reset_mode_config', { modeId });
    } catch (error) {
      throw createTauriCommandError('reset_mode_config', error, { modeId });
    }
  }

  

   
  async getSubagentConfigs(): Promise<Record<string, { enabled: boolean }>> {
    try {
      return await api.invoke('get_subagent_configs');
    } catch (error) {
      throw createTauriCommandError('get_subagent_configs', error);
    }
  }

   
  async setSubagentConfig(subagentId: string, enabled: boolean): Promise<string> {
    try {
      return await api.invoke('set_subagent_config', { subagentId, enabled });
    } catch (error) {
      throw createTauriCommandError('set_subagent_config', error, { subagentId, enabled });
    }
  }

   
  async deleteSubagent(subagentId: string): Promise<void> {
    try {
      await api.invoke('delete_subagent', {
        request: { subagentId },
      });
    } catch (error) {
      throw createTauriCommandError('delete_subagent', error, { subagentId });
    }
  }

  

   
  async getSkillConfigs({
    forceRefresh,
    workspacePath,
  }: GetSkillConfigsParams = {}): Promise<SkillInfo[]> {
    try {
      return await api.invoke('get_skill_configs', { forceRefresh, workspacePath });
    } catch (error) {
      throw createTauriCommandError('get_skill_configs', error, { forceRefresh, workspacePath });
    }
  }

   
  async getModeSkillConfigs({
    modeId,
    forceRefresh,
    workspacePath,
  }: GetModeSkillConfigsParams): Promise<ModeSkillInfo[]> {
    try {
      return await api.invoke('get_mode_skill_configs', { modeId, forceRefresh, workspacePath });
    } catch (error) {
      throw createTauriCommandError('get_mode_skill_configs', error, { modeId, forceRefresh, workspacePath });
    }
  }

   
  async setModeSkillDisabled({
    modeId,
    skillKey,
    disabled,
    workspacePath,
  }: SetModeSkillDisabledParams): Promise<string> {
    try {
      return await api.invoke('set_mode_skill_disabled', { modeId, skillKey, disabled, workspacePath });
    } catch (error) {
      throw createTauriCommandError('set_mode_skill_disabled', error, { modeId, skillKey, disabled, workspacePath });
    }
  }

  async replaceModeSkillSelection({
    modeId,
    enabledSkillKeys,
    workspacePath,
  }: ReplaceModeSkillSelectionParams): Promise<string> {
    try {
      return await api.invoke('replace_mode_skill_selection', {
        request: { modeId, enabledSkillKeys, workspacePath },
      });
    } catch (error) {
      throw createTauriCommandError('replace_mode_skill_selection', error, {
        modeId,
        enabledSkillKeys,
        workspacePath,
      });
    }
  }

   
  async validateSkillPath(path: string): Promise<SkillValidationResult> {
    try {
      return await api.invoke('validate_skill_path', { path });
    } catch (error) {
      throw createTauriCommandError('validate_skill_path', error, { path });
    }
  }

   
  async addSkill({
    sourcePath,
    level,
    workspacePath,
  }: AddSkillParams): Promise<string> {
    try {
      return await api.invoke('add_skill', { sourcePath, level, workspacePath });
    } catch (error) {
      throw createTauriCommandError('add_skill', error, { sourcePath, level, workspacePath });
    }
  }

   
  async deleteSkill({
    skillKey,
    workspacePath,
  }: DeleteSkillParams): Promise<string> {
    try {
      return await api.invoke('delete_skill', { skillKey, workspacePath });
    } catch (error) {
      throw createTauriCommandError('delete_skill', error, { skillKey, workspacePath });
    }
  }

  async listSkillMarket(query?: string, limit?: number): Promise<SkillMarketItem[]> {
    try {
      return await api.invoke('list_skill_market', {
        request: { query, limit }
      });
    } catch (error) {
      throw createTauriCommandError('list_skill_market', error, { query, limit });
    }
  }

  async searchSkillMarket(query: string, limit?: number): Promise<SkillMarketItem[]> {
    try {
      return await api.invoke('search_skill_market', {
        request: { query, limit }
      });
    } catch (error) {
      throw createTauriCommandError('search_skill_market', error, { query, limit });
    }
  }

  async downloadSkillMarket({
    packageId,
    level = 'project',
    workspacePath,
  }: DownloadSkillMarketParams): Promise<SkillMarketDownloadResult> {
    try {
      return await api.invoke('download_skill_market', {
        request: { package: packageId, level, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('download_skill_market', error, {
        package: packageId,
        level,
        workspacePath,
      });
    }
  }
}


export const configAPI = new ConfigAPI();
