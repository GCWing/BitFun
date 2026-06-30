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

export const TOKEN_COMPATIBILITY_ALIAS_CONTRACTS = [
  {
    key: '--color-bg-flowchat',
    canonical: '--color-bg-scene',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Flow chat background remains a named surface alias while the scene background is the canonical root value.',
    removal: 'Retire only after FlowChat, generated widget iframe fallback, and any persisted custom CSS no longer read the alias.',
  },
  {
    key: '--color-bg-surface',
    canonical: '--color-bg-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Surface background is an older shared role that currently resolves to the secondary background in every builtin theme.',
    removal: 'Retire after component-level surface tokens replace generic surface reads.',
  },
  {
    key: '--color-bg-subtle',
    canonical: '--element-bg-subtle',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Subtle background is an element-layer alias, not an independent app background palette.',
    removal: 'Retire after callers migrate to element surface tokens.',
  },
  {
    key: '--color-bg-hover',
    canonical: '--element-bg-hover',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Hover background historically lived under color-bg but now belongs to the element interaction layer.',
    removal: 'Retire after hover callers move to element or component interaction tokens.',
  },
  {
    key: '--color-bg-elevated-hover',
    canonical: '--element-bg-hover',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Elevated hover resolves to the same element hover role and does not represent a separate theme primitive today.',
    removal: 'Retire after elevated surfaces expose component-specific hover tokens.',
  },
  {
    key: '--color-bg-base',
    canonical: '--color-bg-primary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Base background is a historical alias for the primary application background.',
    removal: 'Retire after legacy layout selectors stop reading base background.',
  },
  {
    key: '--color-surface-elevated',
    canonical: '--element-bg-elevated',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Elevated surface is implemented by the element elevated layer, not a separate color family.',
    removal: 'Retire after elevated component tokens cover all consumers.',
  },
  {
    key: '--color-surface-hover',
    canonical: '--element-bg-hover',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Surface hover is a compatibility alias for the element hover layer.',
    removal: 'Retire after component selectors migrate to element or component hover tokens.',
  },
  {
    key: '--color-hover',
    canonical: '--element-bg-hover',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Generic hover is retained for old selectors that predate the element layer naming.',
    removal: 'Retire after all callers use role-specific hover tokens.',
  },
  {
    key: '--bg-primary',
    canonical: '--color-bg-primary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short background aliases are kept for historical component and external widget compatibility.',
    removal: 'Retire after source and generated widget iframe fallback stop requiring short background aliases.',
  },
  {
    key: '--bg-secondary',
    canonical: '--color-bg-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short background aliases are kept for historical component and external widget compatibility.',
    removal: 'Retire after source and generated widget iframe fallback stop requiring short background aliases.',
  },
  {
    key: '--bg-tertiary',
    canonical: '--color-bg-tertiary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short background aliases are kept for historical component and external widget compatibility.',
    removal: 'Retire after source and generated widget iframe fallback stop requiring short background aliases.',
  },
  {
    key: '--bg-elevated',
    canonical: '--color-bg-elevated',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short elevated background alias preserves older selector and generated widget iframe fallback compatibility.',
    removal: 'Retire after elevated component tokens replace the short alias.',
  },
  {
    key: '--bg-hover',
    canonical: '--element-bg-hover',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short hover background alias maps to the canonical element hover layer.',
    removal: 'Retire after all hover consumers use element or component tokens.',
  },
  {
    key: '--secondary-bg',
    canonical: '--color-bg-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Legacy secondary background alias remains for older CSS modules and generated widget compatibility.',
    removal: 'Retire after workspace and legacy CSS callers migrate to color-bg-secondary.',
  },
  {
    key: '--background-primary',
    canonical: '--color-bg-primary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Background primary alias preserves older naming used by app and embedded surfaces.',
    removal: 'Retire after all background-* callers migrate to color-bg-* names.',
  },
  {
    key: '--background-secondary',
    canonical: '--color-bg-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Background secondary alias preserves older naming used by app and embedded surfaces.',
    removal: 'Retire after all background-* callers migrate to color-bg-* names.',
  },
  {
    key: '--background-tertiary',
    canonical: '--color-bg-tertiary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Background tertiary alias preserves older naming used by app and embedded surfaces.',
    removal: 'Retire after all background-* callers migrate to color-bg-* names.',
  },
  {
    key: '--color-background-secondary',
    canonical: '--color-bg-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Color-background secondary exists only as a historical spelling variant.',
    removal: 'Retire after callers use color-bg-secondary.',
  },
  {
    key: '--color-background-tertiary',
    canonical: '--color-bg-tertiary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Color-background tertiary exists only as a historical spelling variant.',
    removal: 'Retire after callers use color-bg-tertiary.',
  },
  {
    key: '--color-text-tertiary',
    canonical: '--color-text-muted',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Tertiary text resolves to muted text in the current theme model and should not imply a fourth text ramp.',
    removal: 'Retire after consumers choose either muted text or a component-specific subdued text role.',
  },
  {
    key: '--text-primary',
    canonical: '--color-text-primary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short text aliases are retained for legacy CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after all text-* consumers move to color-text-* names.',
  },
  {
    key: '--text-secondary',
    canonical: '--color-text-secondary',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short text aliases are retained for legacy CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after all text-* consumers move to color-text-* names.',
  },
  {
    key: '--text-tertiary',
    canonical: '--color-text-muted',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short tertiary text alias maps to muted text and should not become an independent text scale.',
    removal: 'Retire after consumers migrate to muted text or component-specific subdued text tokens.',
  },
  {
    key: '--text-muted',
    canonical: '--color-text-muted',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short muted text alias is kept for legacy CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after all text-* consumers move to color-text-* names.',
  },
  {
    key: '--text-disabled',
    canonical: '--color-text-disabled',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short disabled text alias is kept for legacy CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after all text-* consumers move to color-text-* names.',
  },
  {
    key: '--color-primary',
    canonical: '--color-accent-500',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary currently means the active accent midpoint; it is kept as compatibility for older primary-button and focus selectors.',
    removal: 'Retire only after primary action tokens are componentized and widget iframe fallback no longer requires this key.',
  },
  {
    key: '--color-primary-rgb',
    canonical: '--color-accent-500-rgb',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary RGB channels are historical accent channels used for alpha composition; the canonical name follows the accent scale.',
    removal: 'Retire after alpha-composition callers and generated widget iframe fallback stop requiring primary-rgb.',
  },
  {
    key: '--color-primary-hover',
    canonical: '--color-accent-600',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary hover is the active accent hover stop in all builtin themes.',
    removal: 'Retire after callers use accent hover or component action tokens.',
  },
  {
    key: '--color-accent',
    canonical: '--color-accent-500',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Generic accent alias remains for legacy selectors that predate numeric accent scale usage.',
    removal: 'Retire after callers use explicit accent scale stops or component tokens.',
  },
  {
    key: '--color-accent-primary',
    canonical: '--color-accent-500',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Accent-primary is a historical spelling of the active accent midpoint.',
    removal: 'Retire after generated widget iframe fallback and source callers stop reading it.',
  },
  {
    key: '--accent-primary',
    canonical: '--color-accent-500',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Accent-primary short alias is kept for older CSS modules and generated widget iframe fallback compatibility.',
    removal: 'Retire after callers use color-accent-500 or component action tokens.',
  },
  {
    key: '--accent-primary-hover',
    canonical: '--color-accent-600',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Accent-primary hover short alias resolves to the canonical accent hover stop.',
    removal: 'Retire after callers use color-accent-600 or component action tokens.',
  },
  {
    key: '--color-primary-400',
    canonical: '--color-accent-400',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary scale aliases mirror accent scale stops for historical primary naming.',
    removal: 'Retire after primary-* scale reads migrate to color-accent-*.',
  },
  {
    key: '--color-primary-500',
    canonical: '--color-accent-500',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary scale aliases mirror accent scale stops for historical primary naming.',
    removal: 'Retire after primary-* scale reads migrate to color-accent-*.',
  },
  {
    key: '--color-primary-alpha',
    canonical: '--color-accent-100',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary alpha is a compatibility alias for the faint accent surface.',
    removal: 'Retire after callers use color-accent-100 or component-specific accent backgrounds.',
  },
  {
    key: '--color-primary-bg',
    canonical: '--color-accent-100',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary background is a compatibility alias for the faint accent surface.',
    removal: 'Retire after callers use color-accent-100 or component-specific accent backgrounds.',
  },
  {
    key: '--color-primary-bg-subtle',
    canonical: '--color-accent-50',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary subtle background is a compatibility alias for the faintest accent surface.',
    removal: 'Retire after callers use color-accent-50 or component-specific accent backgrounds.',
  },
  {
    key: '--color-accent-alpha',
    canonical: '--color-accent-100',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Accent alpha is a compatibility name for the faint accent surface stop.',
    removal: 'Retire after callers use explicit accent scale stops.',
  },
  {
    key: '--color-success-100',
    canonical: '--color-success-bg',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Semantic numeric aliases mirror background and foreground roles for older components.',
    removal: 'Retire after callers use semantic role names instead of numeric status stops.',
  },
  {
    key: '--color-success-500',
    canonical: '--color-success',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Semantic numeric aliases mirror background and foreground roles for older components.',
    removal: 'Retire after callers use semantic role names instead of numeric status stops.',
  },
  {
    key: '--color-warning-100',
    canonical: '--color-warning-bg',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Semantic numeric aliases mirror background and foreground roles for older components.',
    removal: 'Retire after callers use semantic role names instead of numeric status stops.',
  },
  {
    key: '--color-warning-500',
    canonical: '--color-warning',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Semantic numeric aliases mirror background and foreground roles for older components.',
    removal: 'Retire after callers use semantic role names instead of numeric status stops.',
  },
  {
    key: '--color-warning-700',
    canonical: '--color-warning',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Warning 700 currently resolves to warning foreground and is not a separate warning ramp stop.',
    removal: 'Retire after warning state roles use semantic names or a real multi-stop warning palette exists.',
  },
  {
    key: '--color-semantic-error',
    canonical: '--color-error',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Semantic error is a historical spelling of the canonical error foreground.',
    removal: 'Retire after callers use color-error.',
  },
  {
    key: '--color-danger',
    canonical: '--color-error',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger action color currently shares the error palette but stays named to avoid silently changing destructive-action semantics.',
    removal: 'Retire only after destructive actions have a separate component action token or explicitly choose color-error.',
  },
  {
    key: '--color-danger-500',
    canonical: '--color-error',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger numeric foreground currently maps to the canonical error foreground.',
    removal: 'Retire after destructive action callers use role names rather than numeric danger stops.',
  },
  {
    key: '--color-danger-text',
    canonical: '--color-error',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger text currently maps to error foreground while preserving destructive-action intent at call sites.',
    removal: 'Retire only after destructive text call sites explicitly migrate to error or action tokens.',
  },
  {
    key: '--color-danger-bg',
    canonical: '--color-error-bg',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger background currently maps to error background while preserving destructive-action intent at call sites.',
    removal: 'Retire only after destructive surfaces explicitly migrate to error or action tokens.',
  },
  {
    key: '--color-danger-border',
    canonical: '--color-error-border',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger border currently maps to error border while preserving destructive-action intent at call sites.',
    removal: 'Retire only after destructive surfaces explicitly migrate to error or action tokens.',
  },
  {
    key: '--color-danger-hover',
    canonical: '--color-error',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Danger hover currently maps to error foreground while preserving destructive-action intent at call sites.',
    removal: 'Retire only after destructive hover states move to component action tokens.',
  },
  {
    key: '--border-color',
    canonical: '--border-subtle',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Generic border color is retained for older selectors and maps to the subtle border role.',
    removal: 'Retire after callers use explicit border-subtle or component border tokens.',
  },
  {
    key: '--border-hover',
    canonical: '--border-medium',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Border hover maps to the medium border role in the current interaction scale.',
    removal: 'Retire after hover states use component interaction border tokens.',
  },
  {
    key: '--border-muted',
    canonical: '--border-subtle',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Muted border is a historical spelling for subtle border.',
    removal: 'Retire after callers use border-subtle.',
  },
  {
    key: '--border-primary',
    canonical: '--border-base',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Primary border means base border in the current contract and is kept for legacy selector compatibility.',
    removal: 'Retire after callers use border-base or component border tokens.',
  },
  {
    key: '--color-border',
    canonical: '--border-base',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Color-border is a legacy spelling of the canonical base border token.',
    removal: 'Retire after callers use border-base.',
  },
  {
    key: '--color-border-primary',
    canonical: '--border-base',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Color-border-primary is a legacy spelling of the canonical base border token.',
    removal: 'Retire after callers use border-base.',
  },
  {
    key: '--color-border-subtle',
    canonical: '--border-subtle',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Color-border-subtle is a legacy spelling of the canonical subtle border token.',
    removal: 'Retire after callers use border-subtle.',
  },
  {
    key: '--element-bg',
    canonical: '--element-bg-base',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Generic element background remains as compatibility for older element surface selectors.',
    removal: 'Retire after callers use explicit element-bg-base or component surface tokens.',
  },
  {
    key: '--motion-normal',
    canonical: '--motion-base',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Motion-normal is a historical alias for the base motion duration.',
    removal: 'Retire after callers use motion-base.',
  },
  {
    key: '--font-sans',
    canonical: '--font-family-sans',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short font aliases are kept for historical CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after callers use font-family-* names and widget iframe fallback no longer requires short aliases.',
  },
  {
    key: '--font-mono',
    canonical: '--font-family-mono',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Short font aliases are kept for historical CSS and generated widget iframe fallback compatibility.',
    removal: 'Retire after callers use font-family-* names and widget iframe fallback no longer requires short aliases.',
  },
  {
    key: '--markdown-font-mono',
    canonical: '--font-family-mono',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Markdown monospace remains a named alias so markdown surfaces can diverge later without breaking callers.',
    removal: 'Retire only if markdown and app monospace are confirmed to remain the same contract.',
  },
  {
    key: '--tool-compact-summary-font',
    canonical: '--font-family-sans',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss',
    reason: 'Tool compact summaries currently use the global sans font but keep a surface alias for future tool-card typography changes.',
    removal: 'Retire only if tool card typography will not diverge from global sans.',
  },
];

export const TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS = [
  {
    prefix: '--radius-',
    canonicalPrefix: '--size-radius-',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Radius aliases keep older selectors and generated widget iframe fallback working while size-radius is the canonical shape scale.',
    removal: 'Retire after all source and generated widget iframe consumers migrate to --size-radius-*.',
  },
  {
    prefix: '--spacing-',
    canonicalPrefix: '--size-gap-',
    owner: 'src/web-ui/src/component-library/styles/tokens.scss; src/web-ui/src/infrastructure/theme/core/ThemeService.ts',
    reason: 'Spacing aliases keep older selectors and generated widget iframe fallback working while size-gap is the canonical spacing scale.',
    removal: 'Retire after all source and generated widget iframe consumers migrate to --size-gap-*.',
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
    key: '--tool-command-preview-empty-rgb',
    canonical: '--tool-command-empty-rgb',
    owner: 'src/web-ui/src/flow_chat/tool-cards/ToolCommandPreview.scss; src/web-ui/src/flow_chat/tool-cards/TerminalToolCard.scss',
    reason: 'Empty command color is shared by command preview and terminal command rendering, so it needs one tool-command token.',
  },
  {
    key: '--m-editor-highlight-rgb',
    canonical: '--markdown-editor-highlight-rgb',
    owner: 'src/web-ui/src/tools/editor/meditor/components/TiptapEditor.scss',
    reason: 'Markdown editor highlight color should use the shared markdown-editor namespace instead of the abbreviated meditor local key.',
  },
  {
    key: '--m-editor-highlight-border-rgb',
    canonical: '--markdown-editor-highlight-border-rgb',
    owner: 'src/web-ui/src/tools/editor/meditor/components/TiptapEditor.scss',
    reason: 'Markdown editor highlight border color should use the shared markdown-editor namespace instead of the abbreviated meditor local key.',
  },
];

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
    canonicalPrefix: '--size-radius-',
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
    canonicalPrefix: '--size-gap-',
    reason: 'Theme runtime exports configurable spacing entries.',
  },
];

export const REGISTERED_DYNAMIC_VAR_PREFIXES = new Set(
  DYNAMIC_VAR_FAMILY_CONTRACTS.map(contract => contract.prefix),
);
