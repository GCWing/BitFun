/**
 * settingsConfig — static shape of settings categories and tabs.
 *
 * Shared by SettingsNav (left sidebar) and SettingsScene (content renderer).
 * Labels are i18n keys resolved at render time via useTranslation('settings').
 */

export type ConfigTab =
  | 'basics'
  | 'appearance'
  | 'models'
  | 'archived-sessions'
  | 'session-personalization'
  | 'session-permissions'
  | 'quick-actions'
  | 'review'
  | 'memories'
  | 'mcp-tools'
  | 'external-sources'
  | 'acp-agents'
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
          'notification',
          'notifications',
          'startup tips',
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
          'fonts',
          'typography',
          'size',
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
          'session title',
          'auto title',
          'subagent',
        ],
      },
      {
        id: 'archived-sessions',
        labelKey: 'configCenter.tabs.archivedSessions',
        descriptionKey: 'configCenter.tabDescriptions.archivedSessions',
        keywords: [
          'archive',
          'archived',
          'session',
          'sessions',
          'restore',
          'unarchive',
        ],
      },
      {
        id: 'keyboard',
        labelKey: 'configCenter.tabs.keyboard',
        descriptionKey: 'configCenter.tabDescriptions.keyboard',
        keywords: [
          'keyboard',
          'shortcut',
          'keybinding',
          'hotkey',
          'shortcut key',
        ],
      },
    ],
  },
  {
    id: 'smartCapabilities',
    nameKey: 'configCenter.categories.smartCapabilities',
    tabs: [
      {
        id: 'session-personalization',
        labelKey: 'configCenter.tabs.sessionPersonalization',
        descriptionKey: 'configCenter.tabDescriptions.sessionPersonalization',
        keywords: [
          'session',
          'companion',
          'agent',
          'pixel',
          'pet',
          'partner',
        ],
      },
      {
        id: 'session-permissions',
        labelKey: 'configCenter.tabs.sessionPermissions',
        descriptionKey: 'configCenter.tabDescriptions.sessionPermissions',
        keywords: [
          'session',
          'tool',
          'write',
          'file write',
          'timeout',
          'confirmation',
          'computer use',
          'browser',
          'cdp',
          'debug',
          'permission',
          'accessibility',
          'screen',
          'workspace',
          'search',
          'flashgrep',
          'index',
        ],
      },
      {
        id: 'quick-actions',
        labelKey: 'configCenter.tabs.quickActions',
        descriptionKey: 'configCenter.tabDescriptions.quickActions',
        keywords: [
          'quick action',
          'quick actions',
          'commit',
          'pr',
          'pull request',
          'post-coding',
          'shortcut',
        ],
      },
      {
        id: 'review',
        labelKey: 'configCenter.tabs.review',
        descriptionKey: 'configCenter.tabDescriptions.review',
        keywords: [
          'review',
          'code review',
          'strict review',
          'review coverage',
          'review strategy',
          'capacity',
          'cost',
          'latency',
          'audit',
        ],
      },
      {
        id: 'memories',
        labelKey: 'configCenter.tabs.memories',
        descriptionKey: 'configCenter.tabDescriptions.memories',
        keywords: [
          'memory',
          'memories',
          'remember',
          'recall',
          'consolidation',
          'rollout',
          'learning',
          'knowledge',
        ],
      },
      {
        id: 'external-sources',
        labelKey: 'configCenter.tabs.externalSources',
        descriptionKey: 'configCenter.tabDescriptions.externalSources',
        beta: true,
        keywords: [
          'external ai applications',
          'import work',
          'extensions',
          'commands',
          'opencode',
          'claude code',
          'codex',
          'compatibility',
        ],
      },
      {
        id: 'mcp-tools',
        labelKey: 'configCenter.tabs.mcpTools',
        descriptionKey: 'configCenter.tabDescriptions.mcpTools',
        keywords: ['mcp', 'server', 'plugin', 'stdio', 'sse', 'tools'],
      },
      {
        id: 'acp-agents',
        labelKey: 'configCenter.tabs.acpAgents',
        descriptionKey: 'configCenter.tabDescriptions.acpAgents',
        keywords: [
          'acp',
          'agent client protocol',
          'external agent',
          'opencode',
          'claude code',
          'codex',
          'stdio',
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
  if (section === 'theme' || section === 'font' || section === 'fonts') return 'appearance';
  if (section === 'logging' || section === 'terminal') return 'basics';
  if (section === 'lsp') return DEFAULT_SETTINGS_TAB;
  if (section === 'session-config') return 'session-personalization';
  if (section === 'deep-review' || section === 'code-review' || section === 'review-team') return 'review';
  if (section === 'shortcuts' || section === 'keybindings' || section === 'hotkeys') return 'keyboard';
  if ((KNOWN_TABS as readonly string[]).includes(section)) return section as ConfigTab;
  return DEFAULT_SETTINGS_TAB;
}
