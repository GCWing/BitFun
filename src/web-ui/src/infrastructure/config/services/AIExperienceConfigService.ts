 

import { configManager } from './ConfigManager';
import { DEFAULT_AGENT_COMPANION_PET } from './AgentCompanionPetService';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AIExperienceConfig');

/** A quick action item shown in the post-coding actions menu. */
export interface QuickAction {
  id: string;
  label: string;
  prompt: string;
  enabled: boolean;
}

export interface AIExperienceSettings {
  enable_session_title_generation: boolean;
  enable_visual_mode: boolean;
  /** Pixel Agent companion in collapsed chat input (session settings). */
  enable_agent_companion: boolean;
  /** Where to show the Agent companion. */
  agent_companion_display_mode: AgentCompanionDisplayMode;
  /** Optional Petdex-compatible companion package selected by the user. */
  agent_companion_pet?: AgentCompanionPetSelection | null;
  /** Flashgrep-backed accelerated workspace search for local workspaces. */
  enable_workspace_search: boolean;
  /** User-defined quick actions shown in the post-coding actions menu. */
  quick_actions?: QuickAction[];
}

export type AgentCompanionDisplayMode = 'input' | 'desktop';

export interface AgentCompanionPetSelection {
  id: string;
  displayName: string;
  description?: string | null;
  source: 'preset' | 'user';
  packagePath: string;
  spritesheetPath: string;
  spritesheetMimeType: string;
}

const CONFIG_PATH = 'app.ai_experience';

export const DEFAULT_QUICK_ACTIONS: QuickAction[] = [
  {
    id: 'commit',
    label: 'Commit',
    prompt: 'Commit all current code changes',
    enabled: true,
  },
  {
    id: 'create_pr',
    label: 'Create PR',
    prompt: 'Create a Pull Request for the current branch',
    enabled: true,
  },
];

const defaultSettings: AIExperienceSettings = {
  enable_session_title_generation: true,
  enable_visual_mode: false,
  enable_agent_companion: true,
  agent_companion_display_mode: 'desktop',
  agent_companion_pet: DEFAULT_AGENT_COMPANION_PET,
  enable_workspace_search: false,
  quick_actions: DEFAULT_QUICK_ACTIONS,
};

function normalizeSettings(settings: AIExperienceSettings | null | undefined): AIExperienceSettings {
  const merged = { ...defaultSettings, ...settings };
  // Legacy configs used null to mean the built-in SVG panda. Panda is now the default preset.
  if (!merged.agent_companion_pet) {
    merged.agent_companion_pet = DEFAULT_AGENT_COMPANION_PET;
  }
  return merged;
}

 
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
      const merged = normalizeSettings(settings);
      // Seed quick_actions with defaults when the stored value is absent.
      if (!merged.quick_actions || merged.quick_actions.length === 0) {
        merged.quick_actions = DEFAULT_QUICK_ACTIONS;
      }
      this.cachedSettings = merged;
    } catch (error) {
      log.warn('Failed to load config, using defaults', error);
      this.cachedSettings = { ...defaultSettings };
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
      this.cachedSettings = normalizeSettings(settings);
      return this.cachedSettings;
    } catch (error) {
      log.error('Failed to get config', error);
      return this.getSettings(); 
    }
  }

   
  async saveSettings(settings: AIExperienceSettings): Promise<void> {
    try {
      const normalized = normalizeSettings(settings);
      await configManager.setConfig(CONFIG_PATH, normalized);
      this.cachedSettings = normalized;
      this.notifyListeners();
    } catch (error) {
      log.error('Failed to save config', error);
      throw error;
    }
  }

   
  isSessionTitleGenerationEnabled(): boolean {
    return this.getSettings().enable_session_title_generation;
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
