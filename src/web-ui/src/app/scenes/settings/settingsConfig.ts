/**
 * settingsConfig — static shape of settings categories and tabs.
 *
 * Shared by SettingsNav (left sidebar) and SettingsScene (content renderer).
 * Labels are i18n keys resolved at render time via useTranslation('settings').
 */

export type ConfigTab =
  | 'basics'
  | 'models'
  | 'session-config'
  | 'ai-context'
  | 'mcp-tools'
  // | 'lsp' // temporarily hidden from config center
  | 'editor'
  | 'keyboard';

export interface ConfigTabDef {
  id: ConfigTab;
  labelKey: string;
  /** i18n key under settings namespace for tab description (search + discoverability). */
  descriptionKey?: string;
  /** Language-neutral extra tokens matched by search (ASCII recommended). */
  keywords?: string[];
  /** Show a Beta pill next to the tab label in the settings nav. */
  beta?: boolean;
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
      {
        id: 'basics',
        labelKey: 'configCenter.tabs.basics',
        descriptionKey: 'configCenter.tabDescriptions.basics',
        keywords: [
          'language',
          'locale',
          'i18n',
          'theme',
          'appearance',
          'logging',
          'log',
          'terminal',
          'shell',
          'pwsh',
          'powershell',
          'autostart',
          'login',
          'boot',
          'launch',
        ],
      },
      {
        id: 'models',
        labelKey: 'configCenter.tabs.models',
        descriptionKey: 'configCenter.tabDescriptions.models',
        keywords: [
          'api',
          'api key',
          'provider',
          'openai',
          'claude',
          'gpt',
          'base url',
          'proxy',
          'model',
          'temperature',
          'token',
        ],
      },
      {
        id: 'keyboard',
        labelKey: 'configCenter.tabs.keyboard',
        descriptionKey: 'configCenter.tabDescriptions.keyboard',
        beta: true,
        keywords: [
          'keyboard',
          'shortcut',
          'keybinding',
          'hotkey',
          'shortcut key',
          '快捷键',
          '键位',
          'beta',
        ],
      },
    ],
  },
  {
    id: 'smartCapabilities',
    nameKey: 'configCenter.categories.smartCapabilities',
    tabs: [
      {
        id: 'session-config',
        labelKey: 'configCenter.tabs.sessionConfig',
        descriptionKey: 'configCenter.tabDescriptions.sessionConfig',
        keywords: [
          'session',
          'chat',
          'streaming',
          'tool',
          'timeout',
          'confirmation',
          'history',
          'companion',
          'agent',
          'partner',
          '伙伴',
        ],
      },
      {
        id: 'ai-context',
        labelKey: 'configCenter.tabs.aiContext',
        descriptionKey: 'configCenter.tabDescriptions.aiContext',
        keywords: ['memory', 'context', 'rag', 'knowledge'],
      },
      {
        id: 'mcp-tools',
        labelKey: 'configCenter.tabs.mcpTools',
        descriptionKey: 'configCenter.tabDescriptions.mcpTools',
        keywords: ['mcp', 'server', 'plugin', 'stdio', 'sse', 'tools'],
      },
    ],
  },
  {
    id: 'devkit',
    nameKey: 'configCenter.categories.devkit',
    tabs: [
      {
        id: 'editor',
        labelKey: 'configCenter.tabs.editor',
        descriptionKey: 'configCenter.tabDescriptions.editor',
        keywords: [
          'font',
          'indent',
          'tab',
          'minimap',
          'word wrap',
          'line number',
          'format',
          'save',
        ],
      },
      // LSP / language server settings — temporarily hidden from nav
      // {
      //   id: 'lsp',
      //   labelKey: 'configCenter.tabs.lsp',
      //   descriptionKey: 'configCenter.tabDescriptions.lsp',
      //   keywords: ['lsp', 'language server', 'typescript', 'intellisense'],
      // },
    ],
  },
];

export const DEFAULT_SETTINGS_TAB: ConfigTab = 'basics';

const KNOWN_TABS: ConfigTab[] = SETTINGS_CATEGORIES.flatMap((c) => c.tabs.map((t) => t.id));

/** Map removed or renamed tabs; used by deep links and IDE actions. */
export function normalizeSettingsTab(section: string): ConfigTab {
  if (section === 'theme' || section === 'logging' || section === 'terminal') return 'basics';
  if (section === 'lsp') return DEFAULT_SETTINGS_TAB;
  if (section === 'shortcuts' || section === 'keybindings' || section === 'hotkeys') return 'keyboard';
  if ((KNOWN_TABS as readonly string[]).includes(section)) return section as ConfigTab;
  return DEFAULT_SETTINGS_TAB;
}
