export const DEFAULT_ROOT = 'src/web-ui/src';
export const DEFAULT_BASELINE_PATH = 'scripts/theme-color-governance-baseline.json';

export const COLOR_EXTENSIONS = new Set(['.css', '.scss', '.sass', '.ts', '.tsx', '.js', '.jsx']);

export const TOKEN_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'BitFun-Installer/src/theme',
  'component-library/styles',
  'infrastructure/theme',
  'theme/presets',
];

export const TOKEN_ALIAS_SOURCE_PATH_PARTS = [
  'component-library/styles/tokens.scss',
];

export const CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'BitFun-Installer/src/theme/installerThemeRuntime.ts',
  'component-library/styles',
  'infrastructure/theme',
  'src/mobile-web/src/theme/presets',
  'tools/bitfun-canvas/runtime/styles',
  'tools/generative-widget/themePayload.ts',
];

export const STATIC_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'component-library/styles',
];

export const RUNTIME_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/theme/installerThemeRuntime.ts',
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
    pathParts: ['BitFun-Installer/src/theme', 'infrastructure/theme/presets', 'theme/presets'],
  },
  {
    key: 'themeRuntime',
    label: 'Theme runtime',
    pathParts: ['infrastructure/theme/core'],
  },
  {
    key: 'tokenContract',
    label: 'Token contracts',
    pathParts: ['BitFun-Installer/src/styles/variables.css', 'component-library/styles'],
  },
  {
    key: 'generatedWidget',
    label: 'Generated widget',
    pathParts: ['tools/generative-widget'],
  },
  {
    key: 'bitfunCanvas',
    label: 'BitFun Canvas',
    pathParts: ['tools/bitfun-canvas'],
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

export const COLOR_DOMAIN_CONTRACTS = [
  {
    key: 'themePreset',
    owner: 'src/web-ui/src/infrastructure/theme/presets',
    reason: 'Builtin themes own primitive palette mapping and must keep per-theme personality instead of being folded into shared app tokens.',
    mergePolicy: 'Only merge exact duplicate primitive values after confirming the theme still exposes distinct semantic roles.',
  },
  {
    key: 'themeRuntime',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Runtime theme injection is the cross-platform bridge for static CSS, desktop WebView, web preview, and generated widget payloads.',
    mergePolicy: 'Do not remove runtime aliases until static contract, runtime contract, and widget iframe compatibility fallback all stop requiring them.',
  },
  {
    key: 'tokenContract',
    owner: 'src/web-ui/src/component-library/styles',
    reason: 'Static token files are the canonical contract for component styling and first paint before runtime theme injection completes.',
    mergePolicy: 'Prefer aliasing to canonical tokens; only keep raw values for primitives or documented component roots.',
  },
  {
    key: 'generatedWidget',
    owner: 'src/web-ui/src/tools/generative-widget',
    reason: 'Generated widgets run in an isolated iframe boundary and need an explicit payload instead of scraping host CSS variables.',
    mergePolicy: 'Keep payload variables canonical; keep legacy aliases in iframe fallback until widget consumers no longer read them.',
  },
  {
    key: 'bitfunCanvas',
    owner: 'src/web-ui/src/tools/bitfun-canvas',
    reason: 'BitFun Canvas renders generated TSX inside a dedicated iframe runtime with an SDK palette that must stay isolated from app chrome tokens.',
    mergePolicy: 'Keep Canvas iframe and SDK colors in the Canvas runtime contract; promote only reusable host chrome roles to shared app tokens.',
  },
  {
    key: 'boundaryFallback',
    owner: 'src/web-ui/src/shared/theme/themeBoundaryFallbacks.ts',
    reason: 'Boundary fallback colors cover iframe, mini app, and capture surfaces before the host theme contract is available.',
    mergePolicy: 'Centralize fallback values here; do not duplicate fallback palettes in component selectors.',
  },
  {
    key: 'mermaid',
    owner: 'src/web-ui/src/tools/mermaid-editor',
    reason: 'Mermaid rendering owns graph palette semantics that do not map one-to-one to app surface states.',
    mergePolicy: 'Treat as a specialized palette unless a graph role is proven to be equivalent across all Mermaid themes.',
  },
  {
    key: 'editor',
    owner: 'src/web-ui/src/tools/editor; src/web-ui/src/component-library/components/CodeEditor',
    reason: 'Code editor and Monaco palettes encode syntax, diff, selection, and editor chrome states beyond generic app UI.',
    mergePolicy: 'Do not merge editor states into app tokens without code-editor focused visual evidence.',
  },
  {
    key: 'syntax',
    owner: 'src/web-ui/src/shared/prism; src/web-ui/src/shared/theme/syntaxHighlightAccents.ts',
    reason: 'Syntax highlight colors preserve token class contrast and language readability, not generic app emphasis.',
    mergePolicy: 'Only merge within the syntax palette after checking token adjacency and light/dark contrast.',
  },
  {
    key: 'terminal',
    owner: 'src/web-ui/src/tools/terminal; src/web-ui/src/flow_chat/tool-cards/TerminalToolCard',
    reason: 'Terminal colors include ANSI and terminal surface roles that must stay compatible with shell output semantics.',
    mergePolicy: 'Keep ANSI roles independent even when values resemble app semantic colors.',
  },
  {
    key: 'debugOverlay',
    owner: 'src/web-ui/src/shared/inspector',
    reason: 'Inspector overlays need high-visibility diagnostic marks and should not influence product token budgets.',
    mergePolicy: 'Keep diagnostic overlays isolated; merge only if the overlay no longer carries a debugging role.',
  },
  {
    key: 'uiException',
    owner: 'src/web-ui/src/shared/theme/uiExceptionAccents.ts',
    reason: 'UI exception accents centralize fixed role and identity colors that are intentionally not global semantic tokens.',
    mergePolicy: 'Require a role owner before adding; promote to component or semantic token only when multiple surfaces share the role.',
  },
  {
    key: 'languageIdentity',
    owner: 'src/web-ui/src/infrastructure/language-detection; src/web-ui/src/shared/theme/languageIdentityAccents.ts',
    reason: 'Language identity colors help recognition of files and snippets and are not interchangeable with status colors.',
    mergePolicy: 'Do not merge adjacent language identities solely by numeric color distance.',
  },
  {
    key: 'visualEffect',
    owner: 'src/web-ui/src/component-library/components/TextStrokeEffect; src/web-ui/src/component-library/components/StreamText',
    reason: 'Visual effects use decorative gradients and animation colors that are separate from UI state semantics.',
    mergePolicy: 'Merge only extremely similar decorative colors when they are not adjacent and do not encode separate modes.',
  },
];

export const TOKEN_COMPATIBILITY_ALIAS_CONTRACTS = [];

export const TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS = [
  {
    prefix: '--radius-',
    canonicalPrefix: '--size-radius-',
    owner: 'src/web-ui/src/tools/generative-widget/themePayloadCompatibility.ts',
    reason: 'Radius aliases are retired from root/runtime but remain recognized so old generated widget iframe content maps to the canonical shape scale.',
    removal: 'Retire after generated widget iframe compatibility no longer needs --radius-* fallbacks.',
  },
  {
    prefix: '--spacing-',
    canonicalPrefix: '--size-gap-',
    owner: 'src/web-ui/src/tools/generative-widget/themePayloadCompatibility.ts',
    reason: 'Spacing aliases are retired from root/runtime but remain recognized so old generated widget iframe content maps to the canonical spacing scale.',
    removal: 'Retire after generated widget iframe compatibility no longer needs --spacing-* fallbacks.',
  },
];

export const FALLBACK_VAR_CONTRACTS = [];

export const SURFACE_TOKEN_RENAME_CONTRACTS = [
  {
    key: '--primary-color',
    canonical: '--base-tool-card-accent-color',
    owner: 'src/web-ui/src/component-library/components/FlowChatCards/BaseToolCard',
    reason: 'BaseToolCard used a generic local primary color key; the explicit component key prevents accidental global primary-token coupling.',
  },
  {
    key: '--operation-color',
    canonical: '--snapshot-card-operation-color',
    owner: 'src/web-ui/src/component-library/components/FlowChatCards/SnapshotCard',
    reason: 'Snapshot operation color is a card-local role and should not look like a reusable operation namespace for other surfaces.',
  },
  {
    key: '--delay',
    canonical: '--flowchat-scroll-anchor-delay',
    owner: 'src/web-ui/src/flow_chat/components/modern/ScrollAnchor',
    reason: 'ScrollAnchor animation delay is a Flow Chat runtime input; the generic key is too easy to collide with unrelated animation code.',
  },
  {
    key: '--um-failed-fs',
    canonical: '--user-message-failed-font-size',
    owner: 'src/web-ui/src/flow_chat/components/modern/UserMessageItem.scss',
    reason: 'UserMessage failed-state sizing should use readable Flow Chat surface names instead of an abbreviated local key family.',
  },
  {
    key: '--um-failed-lh',
    canonical: '--user-message-failed-line-height',
    owner: 'src/web-ui/src/flow_chat/components/modern/UserMessageItem.scss',
    reason: 'UserMessage failed-state line-height should use readable Flow Chat surface names instead of an abbreviated local key family.',
  },
  {
    key: '--um-failed-line-box',
    canonical: '--user-message-failed-line-box',
    owner: 'src/web-ui/src/flow_chat/components/modern/UserMessageItem.scss',
    reason: 'UserMessage failed-state line box should use readable Flow Chat surface names instead of an abbreviated local key family.',
  },
  {
    key: '--m-editor-highlight-rgb',
    canonical: '--private-markdown-editor-highlight-rgb',
    owner: 'src/web-ui/src/tools/editor/meditor/components/TiptapEditor.scss',
    reason: 'Markdown editor highlight color should use the component-private markdown-editor helper instead of the abbreviated meditor local key.',
  },
  {
    key: '--m-editor-highlight-border-rgb',
    canonical: '--private-markdown-editor-highlight-border-rgb',
    owner: 'src/web-ui/src/tools/editor/meditor/components/TiptapEditor.scss',
    reason: 'Markdown editor highlight border color should use the component-private markdown-editor helper instead of the abbreviated meditor local key.',
  },
];

export const DYNAMIC_VAR_FAMILY_CONTRACTS = [
  {
    prefix: '--bitfun-canvas-',
    owner: 'src/web-ui/src/tools/bitfun-canvas/runtime/canvasRuntimeInstaller.ts; src/web-ui/src/tools/bitfun-canvas/runtime/styles/canvas-runtime.scss',
    reason: 'BitFun Canvas iframe runtime receives host theme values through a scoped CSS variable family that must stay isolated from app root tokens.',
  },
  {
    prefix: '--blur-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Theme runtime exports configurable blur scale entries from the active theme effects.',
  },
  {
    prefix: '--color-accent-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts; src/mobile-web/src/theme/presets',
    reason: 'Theme runtime and mobile presets export the active accent palette scale by numeric stop; installer exposes only exact --color-accent-500.',
  },
  {
    prefix: '--color-purple-',
    owner: 'src/web-ui/src/infrastructure/theme/core/ThemeService.ts; src/mobile-web/src/theme/presets',
    reason: 'Theme runtime and mobile presets export the secondary purple palette scale by numeric stop; installer keeps only the accent family it renders.',
  },
  {
    prefix: '--color-pink-',
    owner: 'src/mobile-web/src/theme/presets',
    reason: 'Mobile presets export assistant-mode identity accents by numeric stop for session and picker states.',
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
];

export const REGISTERED_DYNAMIC_VAR_PREFIXES = new Set(
  DYNAMIC_VAR_FAMILY_CONTRACTS.map(contract => contract.prefix),
);
