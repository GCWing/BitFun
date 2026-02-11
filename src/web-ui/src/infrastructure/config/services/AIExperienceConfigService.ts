 

import { configManager } from './ConfigManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AIExperienceConfig');

export interface AIExperienceSettings {
   
  enable_session_title_generation: boolean;
  
   
  enable_welcome_panel_ai_analysis: boolean;
}

const CONFIG_PATH = 'app.ai_experience';

const defaultSettings: AIExperienceSettings = {
  enable_session_title_generation: true,
  enable_welcome_panel_ai_analysis: false,
};

 
export class AIExperienceConfigService {
  private static instance: AIExperienceConfigService;
  private cachedSettings: AIExperienceSettings | null = null;
  private listeners: Set<(settings: AIExperienceSettings) => void> = new Set();
  private unwatchConfig: (() => void) | null = null;

  private constructor() {
    // Defer configManager access to avoid circular dependency TDZ at module evaluation time.
    // By the next microtask, all ESM modules have finished evaluating and configManager is available.
    Promise.resolve().then(() => {
      this.unwatchConfig = configManager.watch(CONFIG_PATH, () => {
        this.reload();
      });
      this.loadSettings();
    });
  }

   
  static getInstance(): AIExperienceConfigService {
    if (!AIExperienceConfigService.instance) {
      AIExperienceConfigService.instance = new AIExperienceConfigService();
    }
    return AIExperienceConfigService.instance;
  }

   
  private async loadSettings(): Promise<void> {
    try {
      const settings = await configManager.getConfig<AIExperienceSettings>(CONFIG_PATH);
      this.cachedSettings = { ...defaultSettings, ...settings };
    } catch (error) {
      log.warn('Failed to load config, using defaults', error);
      this.cachedSettings = defaultSettings;
    }
  }

   
  getSettings(): AIExperienceSettings {
    if (this.cachedSettings) {
      return { ...this.cachedSettings };
    }
    
    return { ...defaultSettings };
  }

   
  async getSettingsAsync(): Promise<AIExperienceSettings> {
    try {
      const settings = await configManager.getConfig<AIExperienceSettings>(CONFIG_PATH);
      this.cachedSettings = { ...defaultSettings, ...settings };
      return this.cachedSettings;
    } catch (error) {
      log.error('Failed to get config', error);
      return this.getSettings(); 
    }
  }

   
  async saveSettings(settings: AIExperienceSettings): Promise<void> {
    try {
      await configManager.setConfig(CONFIG_PATH, settings);
      this.cachedSettings = settings;
      this.notifyListeners();
    } catch (error) {
      log.error('Failed to save config', error);
      throw error;
    }
  }

   
  isSessionTitleGenerationEnabled(): boolean {
    return this.getSettings().enable_session_title_generation;
  }

   
  isWelcomePanelAIAnalysisEnabled(): boolean {
    return this.getSettings().enable_welcome_panel_ai_analysis;
  }

   
  addChangeListener(listener: (settings: AIExperienceSettings) => void): () => void {
    this.listeners.add(listener);
    
    
    return () => {
      this.listeners.delete(listener);
    };
  }

   
  private notifyListeners(): void {
    this.listeners.forEach(listener => {
      try {
        listener(this.getSettings());
      } catch (error) {
        log.error('Listener execution failed', error);
      }
    });
  }

   
  async reload(): Promise<void> {
    await this.loadSettings();
    this.notifyListeners();
  }

   
  dispose(): void {
    if (this.unwatchConfig) {
      this.unwatchConfig();
      this.unwatchConfig = null;
    }
    this.listeners.clear();
  }
}

 
export const aiExperienceConfigService = AIExperienceConfigService.getInstance();

