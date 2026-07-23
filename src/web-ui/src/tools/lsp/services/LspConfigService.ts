/**
 * LSP config service (user-facing settings).
 */

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LspConfigService');

export interface LspSettings {
  autoStartEnabled: boolean;
}

const DEFAULT_LSP_SETTINGS: LspSettings = {
  autoStartEnabled: true
};

const LSP_SETTINGS_KEY = 'bitfun_lsp_settings';

class LspConfigService {
  private static instance: LspConfigService;

  private constructor() {}

  static getInstance(): LspConfigService {
    if (!this.instance) {
      this.instance = new LspConfigService();
    }
    return this.instance;
  }

  getSettings(): LspSettings {
    try {
      const saved = localStorage.getItem(LSP_SETTINGS_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        return { ...DEFAULT_LSP_SETTINGS, ...parsed };
      }
    } catch (error) {
      log.error('Failed to load settings', { error });
    }
    return DEFAULT_LSP_SETTINGS;
  }

  saveSettings(settings: LspSettings): void {
    try {
      localStorage.setItem(LSP_SETTINGS_KEY, JSON.stringify(settings));
    } catch (error) {
      log.error('Failed to save settings', { error });
      throw error;
    }
  }

  isAutoStartEnabled(): boolean {
    return this.getSettings().autoStartEnabled;
  }
}

export const lspConfigService = LspConfigService.getInstance();

