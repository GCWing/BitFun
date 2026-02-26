/**
 * settingsConfig — static shape of settings categories and tabs.
 *
 * Shared by SettingsNav (left sidebar) and SettingsScene (content renderer).
 * Labels are i18n keys resolved at render time via useTranslation('settings').
 */

export type ConfigTab =
  | 'theme'
  | 'models'
  | 'agents'
  | 'ai-features'
  | 'ai-context'
  | 'prompt-templates'
  | 'skills'
  | 'mcp-tools'
  | 'lsp'
  | 'debug'
  | 'logging'
  | 'terminal'
  | 'editor';

export interface ConfigTabDef {
  id: ConfigTab;
  labelKey: string;
}

export interface ConfigCategoryDef {
  id: string;
  nameKey: string;
  tabs: ConfigTabDef[];
}

export const SETTINGS_CATEGORIES: ConfigCategoryDef[] = [
  {
    id: 'general',
    nameKey: 'configCenter.categories.general',
    tabs: [
      { id: 'theme',   labelKey: 'configCenter.tabs.theme'   },
      { id: 'models',  labelKey: 'configCenter.tabs.models'  },
    ],
  },
  {
    id: 'smartCapabilities',
    nameKey: 'configCenter.categories.smartCapabilities',
    tabs: [
      { id: 'ai-features',       labelKey: 'configCenter.tabs.aiFeatures'      },
      { id: 'prompt-templates',  labelKey: 'configCenter.tabs.promptTemplates' },
      { id: 'agents',            labelKey: 'configCenter.tabs.agents'          },
      { id: 'ai-context',        labelKey: 'configCenter.tabs.aiContext'       },
      { id: 'skills',            labelKey: 'configCenter.tabs.skills'          },
      { id: 'mcp-tools',         labelKey: 'configCenter.tabs.mcpTools'        },
    ],
  },
  {
    id: 'devkit',
    nameKey: 'configCenter.categories.devkit',
    tabs: [
      { id: 'editor',  labelKey: 'configCenter.tabs.editor'  },
      { id: 'lsp',     labelKey: 'configCenter.tabs.lsp'     },
      { id: 'debug',   labelKey: 'configCenter.tabs.debug'   },
      { id: 'terminal',labelKey: 'configCenter.tabs.terminal'},
      { id: 'logging', labelKey: 'configCenter.tabs.logging' },
    ],
  },
];

export const DEFAULT_SETTINGS_TAB: ConfigTab = 'models';
