export const DEFAULT_ROOT = 'src/web-ui/src';
export const DEFAULT_BASELINE_PATH = 'scripts/theme-color-governance-baseline.json';

export const COLOR_EXTENSIONS = new Set(['.css', '.scss', '.sass', '.ts', '.tsx', '.js', '.jsx']);

export const TOKEN_PATH_PARTS = [
  'component-library/styles',
  'infrastructure/theme',
  'theme/presets',
];

export const TOKEN_ALIAS_SOURCE_PATH_PARTS = [
  'component-library/styles/tokens.scss',
];

export const CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'component-library/styles',
  'infrastructure/theme',
  'tools/generative-widget/themePayload.ts',
];

export const STATIC_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'component-library/styles',
];

export const RUNTIME_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'infrastructure/theme',
];

export const EXCEPTION_PATH_PARTS = [
  'shared/theme/uiExceptionAccents',
  'shared/theme/languageIdentityAccents',
  'shared/theme/syntaxHighlightAccents',
  'shared/theme/themeBoundaryFallbacks',
  'monaco',
  'terminal',
  'mermaid',
  'syntax',
  'CodeEditor',
  'tools/editor/themes',
];

export const COLOR_DOMAIN_RULES = [
  {
    key: 'themePreset',
    label: 'Theme presets',
    pathParts: ['infrastructure/theme/presets', 'theme/presets'],
  },
  {
    key: 'themeRuntime',
    label: 'Theme runtime',
    pathParts: ['infrastructure/theme/core'],
  },
  {
    key: 'tokenContract',
    label: 'Token contracts',
    pathParts: ['component-library/styles'],
  },
  {
    key: 'generatedWidget',
    label: 'Generated widget',
    pathParts: ['tools/generative-widget'],
  },
  {
    key: 'boundaryFallback',
    label: 'Boundary fallback',
    pathParts: ['shared/theme/themeBoundaryFallbacks'],
  },
  {
    key: 'mermaid',
    label: 'Mermaid',
    pathParts: ['tools/mermaid-editor'],
  },
  {
    key: 'editor',
    label: 'Editor',
    pathParts: ['tools/editor', 'component-library/components/CodeEditor', 'infrastructure/theme/integrations/MonacoThemeSync'],
  },
  {
    key: 'syntax',
    label: 'Syntax',
    pathParts: ['shared/prism', 'shared/theme/syntaxHighlightAccents'],
  },
  {
    key: 'terminal',
    label: 'Terminal',
    pathParts: [
      'tools/terminal',
      'flow_chat/tool-cards/TerminalToolCard',
      'app/components/panels/TerminalEditModal',
    ],
  },
  {
    key: 'debugOverlay',
    label: 'Debug overlay',
    pathParts: ['shared/inspector'],
  },
  {
    key: 'uiException',
    label: 'UI exception registry',
    pathParts: ['shared/theme/uiExceptionAccents'],
  },
  {
    key: 'languageIdentity',
    label: 'Language identity',
    pathParts: ['infrastructure/language-detection', 'shared/theme/languageIdentityAccents'],
  },
  {
    key: 'visualEffect',
    label: 'Visual effects',
    pathParts: [
      'component-library/components/TextStrokeEffect',
      'component-library/components/StreamText',
    ],
  },
];

export const COLOR_DOMAIN_KEYS = [
  ...COLOR_DOMAIN_RULES.map(rule => rule.key),
  'appUi',
];

export const COLOR_DOMAIN_LABELS = Object.fromEntries([
  ...COLOR_DOMAIN_RULES.map(rule => [rule.key, rule.label]),
  ['appUi', 'App UI'],
]);

export const DYNAMIC_VAR_FAMILY_CONTRACTS = [
  {
    prefix: '--blur-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable blur scale entries from the active theme effects.',
  },
  {
    prefix: '--color-accent-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports the active accent palette scale by numeric stop.',
  },
  {
    prefix: '--color-purple-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports the secondary purple palette scale by numeric stop.',
  },
  {
    prefix: '--easing-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports motion easing aliases from theme motion tokens.',
  },
  {
    prefix: '--flowchat-font-size-',
    owner: 'src/web-ui/src/infrastructure/font-preference/core/FontPreferenceService.ts',
    reason: 'Font preference runtime exports FlowChat font-size aliases from the adjusted typography scale.',
  },
  {
    prefix: '--font-size-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts; src/web-ui/src/infrastructure/font-preference/core/FontPreferenceService.ts',
    reason: 'Theme runtime exports baseline typography size entries; font preference runtime can override the same family for user scaling.',
  },
  {
    prefix: '--font-weight-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable typography weight entries.',
  },
  {
    prefix: '--line-height-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable typography line-height entries.',
  },
  {
    prefix: '--motion-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports motion duration entries from active theme motion tokens.',
  },
  {
    prefix: '--nav-font-size-',
    owner: 'src/web-ui/src/infrastructure/font-preference/core/FontPreferenceService.ts',
    reason: 'Font preference runtime exports navigation font-size aliases from the adjusted typography scale.',
  },
  {
    prefix: '--radius-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable radius entries.',
  },
  {
    prefix: '--shadow-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable shadow entries.',
  },
  {
    prefix: '--size-gap-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Size gap aliases are derived from theme spacing entries.',
  },
  {
    prefix: '--size-radius-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Size radius aliases are derived from theme radius entries.',
  },
  {
    prefix: '--spacing-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable spacing entries.',
  },
];

export const REGISTERED_DYNAMIC_VAR_PREFIXES = new Set(
  DYNAMIC_VAR_FAMILY_CONTRACTS.map(contract => contract.prefix),
);
