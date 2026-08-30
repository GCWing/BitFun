import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  mergeTokenDocuments,
  resolveTokens,
  tokenNameToCssVariable,
} from '../design-system/tooling/token-engine/src/index.mjs';

const contractDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(contractDirectory, '..');

function readDesignSystemTokenDocument(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repositoryRoot, relativePath), 'utf8'));
}

const designSystemTokens = resolveTokens(readDesignSystemTokenDocument(
  'design-system/packages/design-tokens/src/system.tokens.json',
));
const bitfunThemeTokens = resolveTokens(mergeTokenDocuments(
  readDesignSystemTokenDocument('design-system/packages/theme-bitfun/src/reference.tokens.json'),
  readDesignSystemTokenDocument('design-system/packages/theme-bitfun/src/light.tokens.json'),
));
const publicThemePrefixes = ['color.', 'effect.', 'opacity.', 'shadow.'];

export const PACKAGE_CSS_VAR_DEFINITION_CONTRACTS = Object.freeze([
  Object.freeze({
    owner: 'design-system/packages/design-tokens/src/system.tokens.json',
    packageName: '@bitfun/design-tokens',
    variables: Object.freeze(Object.keys(designSystemTokens).map(name => tokenNameToCssVariable(name))),
  }),
  Object.freeze({
    owner: 'design-system/packages/theme-bitfun/src/light.tokens.json',
    packageName: '@bitfun/theme-bitfun',
    variables: Object.freeze(
      Object.keys(bitfunThemeTokens)
        .filter(name => publicThemePrefixes.some(prefix => name.startsWith(prefix)))
        .map(name => tokenNameToCssVariable(name)),
    ),
  }),
]);

export const DEFAULT_ROOT = 'src/web-ui/src';
export const DEFAULT_BASELINE_PATH = 'scripts/theme-color-governance-baseline.json';

export const COLOR_EXTENSIONS = new Set(['.css', '.scss', '.sass', '.ts', '.tsx', '.js', '.jsx']);

export const TOKEN_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'BitFun-Installer/src/theme',
  'component-library/styles',
  'theme/presets',
];

export const TOKEN_ALIAS_SOURCE_PATH_PARTS = [
  'component-library/styles/tokens.scss',
];

export const CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'BitFun-Installer/src/theme/installerThemeRuntime.ts',
  'component-library/styles',
  'infrastructure/appearance',
  'src/mobile-web/src/theme/presets',
  'tools/bitfun-canvas/runtime/styles',
  'tools/generative-widget/appearancePayload.ts',
];

export const STATIC_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/styles/variables.css',
  'component-library/styles',
];

export const RUNTIME_CONTRACT_VAR_DEFINITION_PATH_PARTS = [
  'BitFun-Installer/src/theme/installerThemeRuntime.ts',
  'infrastructure/appearance',
];

export const EXCEPTION_PATH_PARTS = [
  'monaco',
  'terminal',
  'mermaid',
  'syntax',
  'CodeEditor',
];

export const COLOR_DOMAIN_RULES = [
  {
    key: 'appearanceProjection',
    label: 'Appearance projections',
    pathParts: ['infrastructure/appearance/builtins/buildBuiltinAppearance'],
  },
  {
    key: 'themePreset',
    label: 'Appearance palettes',
    pathParts: ['BitFun-Installer/src/theme', 'infrastructure/appearance/builtins', 'theme/presets'],
  },
  {
    key: 'themeRuntime',
    label: 'Appearance runtime',
    pathParts: ['infrastructure/appearance/runtime', 'infrastructure/appearance/adapters'],
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
    key: 'mermaid',
    label: 'Mermaid',
    pathParts: ['tools/mermaid-editor'],
  },
  {
    key: 'editor',
    label: 'Editor',
    pathParts: ['tools/editor', 'component-library/components/CodeEditor', 'infrastructure/appearance/adapters/MonacoAppearanceAdapter'],
  },
  {
    key: 'syntax',
    label: 'Syntax',
    pathParts: ['shared/prism'],
  },
  {
    key: 'terminal',
    label: 'Terminal',
    pathParts: [
      'tools/terminal',
      'app/components/panels/TerminalEditModal',
    ],
  },
  {
    key: 'debugOverlay',
    label: 'Debug overlay',
    pathParts: ['shared/inspector'],
  },
  {
    key: 'appearanceDomain',
    label: 'Appearance domain tokens',
    pathParts: ['infrastructure/appearance/appearanceDomainTokens'],
  },
  {
    key: 'languageIdentity',
    label: 'Language identity',
    pathParts: ['infrastructure/language-detection'],
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
    key: 'appearanceProjection',
    owner: 'src/web-ui/src/infrastructure/appearance/builtins/buildBuiltinAppearance.ts',
    reason: 'The builtin Appearance projection owns renderer palettes and named product-domain defaults derived from each primitive palette.',
    mergePolicy: 'Keep values here only when a renderer or named domain role cannot be represented by the primitive palette shape; external packages remain free to override every projected role.',
  },
  {
    key: 'themePreset',
    owner: 'src/web-ui/src/infrastructure/appearance/builtins',
    reason: 'Builtin appearances own primitive palette mapping and must keep per-appearance personality instead of being folded into shared app tokens.',
    mergePolicy: 'Only merge exact duplicate primitive values after confirming the theme still exposes distinct semantic roles.',
  },
  {
    key: 'themeRuntime',
    owner: 'src/web-ui/src/infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts',
    reason: 'AppearanceRuntime applies the registered CSS token projection for static CSS, web preview, and embedded surface payloads.',
    mergePolicy: 'Keep the runtime projection canonical and reject reintroduction of compatibility aliases or surface-local token owners.',
  },
  {
    key: 'tokenContract',
    owner: 'src/web-ui/src/component-library/styles',
    reason: 'Static Sass files bind component code to runtime-owned Appearance variables without owning visual values.',
    mergePolicy: 'Keep bindings value-free and move every visual value into an Appearance package renderer setting.',
  },
  {
    key: 'generatedWidget',
    owner: 'src/web-ui/src/tools/generative-widget',
    reason: 'Generated widgets run in an isolated iframe boundary and need an explicit payload instead of scraping host CSS variables.',
    mergePolicy: 'Derive fallback values from a builtin Appearance package and keep iframe payload keys canonical.',
  },
  {
    key: 'bitfunCanvas',
    owner: 'src/web-ui/src/tools/bitfun-canvas',
    reason: 'BitFun Canvas renders generated TSX inside a dedicated iframe runtime with an SDK palette that must stay isolated from app chrome tokens.',
    mergePolicy: 'Keep Canvas iframe and SDK colors in the Canvas Appearance contract; promote only reusable host chrome roles to shared app tokens.',
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
    owner: 'src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts; src/web-ui/src/shared/prism',
    reason: 'Prism consumes named Appearance token roles for token-class contrast and readability.',
    mergePolicy: 'Keep syntax values in the Appearance package and keep Prism consumers value-free.',
  },
  {
    key: 'terminal',
    owner: 'src/web-ui/src/tools/terminal; src/web-ui/src/app/components/panels/TerminalEditModal',
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
    key: 'appearanceDomain',
    owner: 'src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts',
    reason: 'Named product-domain roles expose stable CSS variable references while Appearance packages own their values.',
    mergePolicy: 'Add a named role only when a visible semantic distinction is real; never place raw colors in the token reference module.',
  },
  {
    key: 'languageIdentity',
    owner: 'src/web-ui/src/infrastructure/appearance/appearanceDomainTokens.ts; src/web-ui/src/infrastructure/language-detection',
    reason: 'Language identity consumers use named Appearance tokens rather than owning fixed colors.',
    mergePolicy: 'Keep language role values package-controlled and do not hard-code colors in the language registry.',
  },
  {
    key: 'visualEffect',
    owner: 'src/web-ui/src/component-library/components/TextStrokeEffect; src/web-ui/src/component-library/components/StreamText',
    reason: 'Visual effects use decorative gradients and animation colors that are separate from UI state semantics.',
    mergePolicy: 'Merge only extremely similar decorative colors when they are not adjacent and do not encode separate modes.',
  },
];

export const TOKEN_COMPATIBILITY_ALIAS_CONTRACTS = [];

export const TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS = [];

export const FALLBACK_VAR_CONTRACTS = [];

export const SURFACE_TOKEN_RENAME_CONTRACTS = [
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
    prefix: '--bf-appearance-asset-',
    owner: 'src/web-ui/src/infrastructure/appearance/compiler/AppearanceCompiler.ts; src/web-ui/src/infrastructure/appearance/runtime/AppearanceRuntime.ts',
    reason: 'Appearance package image ids are validated by the package schema, then projected to host-created blob URL variables for registered component parts.',
  },
  {
    prefix: '--bitfun-canvas-',
    owner: 'src/web-ui/src/tools/bitfun-canvas/runtime/canvasRuntimeInstaller.ts; src/web-ui/src/tools/bitfun-canvas/runtime/styles/canvas-runtime.scss',
    reason: 'BitFun Canvas iframe runtime receives host Appearance values through a scoped CSS variable family that must stay isolated from app root tokens.',
  },
  {
    prefix: '--color-accent-',
    owner: 'src/mobile-web/src/theme/presets',
    reason: 'Mobile presets export the active accent palette scale by numeric stop.',
  },
  {
    prefix: '--color-purple-',
    owner: 'src/mobile-web/src/theme/presets',
    reason: 'Mobile presets export the secondary accent palette by numeric stop.',
  },
  {
    prefix: '--color-pink-',
    owner: 'src/mobile-web/src/theme/presets',
    reason: 'Mobile presets export assistant-mode identity accents by numeric stop for session and picker states.',
  },
  {
    prefix: '--bf-font-size-',
    owner: 'src/web-ui/src/infrastructure/font-preference/core/FontPreferenceService.ts',
    reason: 'Font preference runtime overrides the canonical design-system font-size primitives so every semantic role follows one global scale.',
  },
];

export const REGISTERED_DYNAMIC_VAR_PREFIXES = new Set(
  DYNAMIC_VAR_FAMILY_CONTRACTS.map(contract => contract.prefix),
);
