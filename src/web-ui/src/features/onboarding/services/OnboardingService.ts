/**
 * First-launch onboarding service.
 * OnboardingService - handles first-launch detection and config persistence (backend supported).
 */

import { useOnboardingStore, isModelConfigComplete, type OnboardingModelConfig } from '../store/onboardingStore';
import type { LocaleId } from '@/infrastructure/i18n/types';
import type { ThemeSelectionId } from '@/infrastructure/theme/types';
import { configAPI } from '@/infrastructure/api';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { modelConfigManager } from '@/infrastructure/config/services/modelConfigs';
import type { AIModelConfig as AIModelConfigType } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('OnboardingService');

const ONBOARDING_COMPLETED_KEY = 'bitfun_onboarding_completed';
const ONBOARDING_VERSION_KEY = 'bitfun_onboarding_version';

// Backend config path
const BACKEND_ONBOARDING_PATH = 'app.onboarding';

// Current onboarding version. Bump to trigger new onboarding.
const CURRENT_ONBOARDING_VERSION = '1.0.0';

/**
 * First-launch onboarding service.
 */
class OnboardingServiceClass {
  private initialized = false;

  /**
   * Check whether this is the first launch.
   * Prefer backend config, fallback to localStorage.
   */
  async checkFirstLaunch(): Promise<boolean> {
    try {
      // 1. Prefer backend (cross-device sync)
      try {
        const backendConfig = await configAPI.getConfig(BACKEND_ONBOARDING_PATH, { skipRetryOnNotFound: true });
        if (backendConfig?.completed === true && backendConfig?.version === CURRENT_ONBOARDING_VERSION) {
          // Sync to localStorage
          localStorage.setItem(ONBOARDING_COMPLETED_KEY, 'true');
          localStorage.setItem(ONBOARDING_VERSION_KEY, CURRENT_ONBOARDING_VERSION);
          log.debug('Onboarding completed according to backend');
          return false;
        }
      } catch (error) {
        log.debug('Failed to get onboarding status from backend, using localStorage', error);
      }

      // 2. Check localStorage
      const completed = localStorage.getItem(ONBOARDING_COMPLETED_KEY);
      const version = localStorage.getItem(ONBOARDING_VERSION_KEY);
      
      // If completed and version matches, it's not first launch
      if (completed === 'true' && version === CURRENT_ONBOARDING_VERSION) {
        return false;
      }
      
      return true;
    } catch (error) {
      log.error('Failed to check first launch', error);
      // Assume first launch on error
      return true;
    }
  }

  /**
   * Mark onboarding as completed.
   * Save to localStorage and backend.
   */
  async markCompleted(): Promise<void> {
    try {
      // Save to localStorage
      localStorage.setItem(ONBOARDING_COMPLETED_KEY, 'true');
      localStorage.setItem(ONBOARDING_VERSION_KEY, CURRENT_ONBOARDING_VERSION);
      
      // Save to backend
      try {
        await configAPI.setConfig(BACKEND_ONBOARDING_PATH, {
          completed: true,
          version: CURRENT_ONBOARDING_VERSION,
          completedAt: new Date().toISOString()
        });
        log.info('Onboarding marked as completed and synced to backend');
      } catch (backendError) {
        log.warn('Failed to save to backend, but saved locally', backendError);
      }
    } catch (error) {
      log.error('Failed to mark onboarding as completed', error);
    }
  }

  /**
   * Reset onboarding state (for tests or reruns).
   */
  async resetOnboarding(): Promise<void> {
    try {
      // Clear localStorage
      localStorage.removeItem(ONBOARDING_COMPLETED_KEY);
      localStorage.removeItem(ONBOARDING_VERSION_KEY);
      
      // Clear backend record
      try {
        await configAPI.setConfig(BACKEND_ONBOARDING_PATH, {
          completed: false,
          version: CURRENT_ONBOARDING_VERSION
        });
      } catch (backendError) {
        log.debug('Failed to reset backend record', backendError);
      }
      
      // Reset store
      useOnboardingStore.getState().resetOnboarding();
      
      log.info('Onboarding reset');
    } catch (error) {
      log.error('Failed to reset onboarding', error);
    }
  }

  /**
   * Initialize onboarding service.
   * @param skipFirstLaunchCheck Whether to skip first-launch check (for tests)
   */
  async initialize(skipFirstLaunchCheck = false): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      const store = useOnboardingStore.getState();
      
      if (skipFirstLaunchCheck) {
        // Test mode: show onboarding directly
        log.debug('Test mode: forcing onboarding display');
        store.forceShowOnboarding();
      } else {
        // Normal mode: detect first launch
        const isFirstLaunch = await this.checkFirstLaunch();
        
        if (isFirstLaunch) {
          log.info('First launch detected, starting onboarding');
          store.startOnboarding();
        } else {
          log.debug('Not first launch, skipping onboarding');
        }
      }

      this.initialized = true;
    } catch (error) {
      log.error('Initialization failed', error);
    }
  }

  /**
   * Apply configuration selected during onboarding.
   */
  async applyConfiguration(config: {
    language?: LocaleId;
    theme?: ThemeSelectionId;
    modelConfig?: OnboardingModelConfig | null;
  }): Promise<void> {
    try {
      // Apply language setting
      if (config.language) {
        const { i18nService } = await import('@/infrastructure/i18n');
        await i18nService.changeLanguage(config.language);
        log.debug('Language setting applied', { language: config.language });
      }

      // Apply theme setting
      if (config.theme) {
        const { themeService } = await import('@/infrastructure/theme');
        await themeService.applyTheme(config.theme);
        log.debug('Theme setting applied', { theme: config.theme });
      }

      // Save model config to ai.models if all required fields are filled
      if (config.modelConfig && isModelConfigComplete(config.modelConfig)) {
        await this.saveModelConfig(config.modelConfig);
      }
    } catch (error) {
      log.error('Failed to apply configuration', error);
      throw error;
    }
  }

  /**
   * Save model configuration to ai.models and set as primary default.
   */
  private async saveModelConfig(modelConfig: OnboardingModelConfig): Promise<void> {
    try {
      const existingModels = await configManager.getConfig<AIModelConfigType[]>('ai.models') || [];
      const newConfig: AIModelConfigType = {
        id: `model_${Date.now()}`,
        name: modelConfig.configName || modelConfig.modelName || 'Custom Model',
        base_url: modelConfig.baseUrl || '',
        api_key: modelConfig.apiKey,
        model_name: modelConfig.modelName || '',
        provider: modelConfig.format || 'openai',
        enabled: true,
        description: '',
        context_window: 128000,
        max_tokens: 8192,
        category: 'general_chat',
        capabilities: ['text_chat', 'function_calling'],
        inline_think_in_text: false,
        custom_request_body: modelConfig.customRequestBody || undefined,
        skip_ssl_verify: modelConfig.skipSslVerify || undefined,
        custom_headers: modelConfig.customHeaders || undefined,
        custom_headers_mode: modelConfig.customHeaders ? modelConfig.customHeadersMode : undefined,
      };

      await configManager.setConfig('ai.models', [...existingModels, newConfig]);

      // Set as primary default model
      const currentDefaultModels = await configManager.getConfig<Record<string, unknown>>('ai.default_models') || {};
      await configManager.setConfig('ai.default_models', {
        ...currentDefaultModels,
        primary: newConfig.id,
      });

      // Reload modelConfigManager to pick up the new config
      await modelConfigManager.reload();

      log.info('Model configuration saved as primary model', {
        provider: modelConfig.provider,
        modelName: modelConfig.modelName,
        modelId: newConfig.id
      });
    } catch (error) {
      log.error('Failed to save model configuration', error);
      throw error;
    }
  }

  /**
   * Get current onboarding version.
   */
  getCurrentVersion(): string {
    return CURRENT_ONBOARDING_VERSION;
  }
}

// Export singleton
export const onboardingService = new OnboardingServiceClass();
