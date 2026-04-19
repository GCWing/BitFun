/**
 * settingsConfig — static shape of settings categories and tabs.
 *
 * Shared by SettingsNav (left sidebar) and SettingsScene (content renderer).
 * Labels are i18n keys resolved at render time via useTranslation('settings').
 */

export type ConfigTab =
  | 'appearance'
  | 'basics'
  | 'models'
  | 'personalization'
  | 'permissions'
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
        id: 'appearance',
        labelKey: 'configCenter.tabs.appearance',
        descriptionKey: 'configCenter.tabDescriptions.appearance',
        keywords: [
          'language',
          'locale',
          'i18n',
          'theme',
          'appearance',
          'font',
          'font size',
          'ui font',
          'chat font',
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
        id: 'personalization',
        labelKey: 'configCenter.tabs.personalization',
        descriptionKey: 'configCenter.tabDescriptions.personalization',
        keywords: [
          'session',
          'chat',
          'personalization',
          'title',
          'agent companion',
          'debug',
          'template',
          'ingest',
          'log path',
          'personality',
          '个性化',
          '标题',
          '调试',
          '伙伴',
        ],
      },
      {
        id: 'permissions',
        labelKey: 'configCenter.tabs.permissions',
        descriptionKey: 'configCenter.tabDescriptions.permissions',
        keywords: [
          'tool',
          'permission',
          'permissions',
          'streaming',
          'timeout',
          'confirmation',
          'computer use',
          'screen capture',
          'accessibility',
          'agent',
          '权限',
          '授权',
        ],
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
  if (section === 'theme' || section === 'appearance' || section === 'language' || section === 'font') return 'appearance';
  if (section === 'logging' || section === 'terminal') return 'basics';
  if (section === 'session-config' || section === 'personal' || section === 'companion' || section === 'debug-mode') return 'personalization';
  if (section === 'permission' || section === 'permissions' || section === 'computer-use' || section === 'tool-execution') return 'permissions';
  if (section === 'ai-context') return DEFAULT_SETTINGS_TAB;
  if (section === 'lsp') return DEFAULT_SETTINGS_TAB;
  if (section === 'shortcuts' || section === 'keybindings' || section === 'hotkeys') return 'keyboard';
  if ((KNOWN_TABS as readonly string[]).includes(section)) return section as ConfigTab;
  return DEFAULT_SETTINGS_TAB;
}
