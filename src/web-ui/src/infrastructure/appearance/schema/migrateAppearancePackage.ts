import { themeCssVariables, type ThemeTokenName } from '@bitfun/theme-bitfun';

import { WIDGET_APPEARANCE_VARIABLE_NAMES } from '../adapters/widgetAppearanceVariables';
import {
  APPEARANCE_ROOT_TOKEN_NAMES,
  APPEARANCE_SCOPED_TOKEN_NAMES,
} from '../appearanceTokenContract';
import {
  APPEARANCE_SCHEMA_VERSION,
  type AppearanceThemeTokenName,
} from '../types';

const LEGACY_APPEARANCE_SCHEMA_VERSION = 1;

const LEGACY_THEME_TOKEN_SOURCES = {
  'color.surface.canvas': '--bf-appearance-token-color-bg-primary',
  'color.surface.panel': '--bf-appearance-token-color-bg-secondary',
  'color.surface.raised': '--bf-appearance-token-color-bg-elevated',
  'color.surface.scene': '--bf-appearance-token-color-bg-scene',
  'color.surface.workbench': '--bf-appearance-token-color-bg-workbench',
  'color.surface.tertiary': '--bf-appearance-token-color-bg-tertiary',
  'color.surface.chrome': '--bf-appearance-token-color-bg-chrome',
  'color.surface.subtle': '--bf-appearance-token-element-bg-subtle',
  'color.overlay.scrim': '--bf-appearance-token-color-overlay-black-50',
  'color.content.primary': '--bf-appearance-token-color-text-primary',
  'color.content.secondary': '--bf-appearance-token-color-text-secondary',
  'color.content.muted': '--bf-appearance-token-color-text-muted',
  'color.content.disabled': '--bf-appearance-token-color-text-disabled',
  'color.content.inverse': '--bf-appearance-token-btn-primary-color',
  'color.content.onDark': '--bf-appearance-token-color-static-white',
  'color.content.onLight': '--bf-appearance-token-color-static-black',
  'color.accent.default': '--bf-appearance-token-color-accent-500',
  'color.accent.hover': '--bf-appearance-token-color-accent-600',
  'color.accent.secondary': '--bf-appearance-token-color-purple-500',
  'color.accent.secondaryHover': '--bf-appearance-token-color-purple-600',
  'color.link.default': '--bf-appearance-token-flowchat-link-color',
  'color.link.hover': '--bf-appearance-token-flowchat-link-hover-color',
  'color.scrollbar.thumb': '--bf-appearance-token-scrollbar-thumb',
  'color.scrollbar.thumbHover': '--bf-appearance-token-scrollbar-thumb-hover',
  'color.border.subtle': '--bf-appearance-token-border-subtle',
  'color.border.default': '--bf-appearance-token-border-base',
  'color.border.strong': '--bf-appearance-token-border-strong',
  'color.action.neutral.border': '--bf-appearance-token-border-base',
  'color.action.neutral.content': '--bf-appearance-token-color-text-secondary',
  'color.action.neutral.contentDisabled': '--bf-appearance-token-color-text-disabled',
  'color.action.neutral.fillBorder': '--bf-appearance-token-element-bg-base',
  'color.action.neutral.surface': '--bf-appearance-token-element-bg-base',
  'color.action.neutral.surfaceHover': '--bf-appearance-token-element-bg-medium',
  'color.action.neutral.surfacePressed': '--bf-appearance-token-element-bg-strong',
  'color.action.primary.background': '--bf-appearance-token-btn-primary-bg',
  'color.action.primary.hover': '--bf-appearance-token-btn-primary-hover-bg',
  'color.action.primary.pressed': '--bf-appearance-token-btn-primary-active-bg',
  'color.action.primary.content': '--bf-appearance-token-btn-primary-color',
  'color.action.secondary.background': '--bf-appearance-token-color-accent-100',
  'color.action.secondary.hover': '--bf-appearance-token-color-accent-200',
  'color.action.secondary.pressed': '--bf-appearance-token-color-accent-300',
  'color.action.secondary.content': '--bf-appearance-token-color-accent-600',
  'color.action.quiet.hover': '--bf-appearance-token-element-bg-soft',
  'color.action.quiet.pressed': '--bf-appearance-token-element-bg-base',
  'color.action.quiet.content': '--bf-appearance-token-color-text-secondary',
  'color.field.background': '--bf-appearance-token-color-bg-secondary',
  'color.field.backgroundHover': '--bf-appearance-token-element-bg-subtle',
  'color.field.border': '--bf-appearance-token-border-base',
  'color.field.borderHover': '--bf-appearance-token-border-medium',
  'color.field.borderFocus': '--bf-appearance-token-color-accent-500',
  'color.focus.ring': '--bf-appearance-token-color-accent-500',
  'color.status.info.content': '--bf-appearance-token-color-info',
  'color.status.info.surface': '--bf-appearance-token-color-info-bg',
  'color.status.info.border': '--bf-appearance-token-color-info-border',
  'color.status.success.content': '--bf-appearance-token-color-success',
  'color.status.success.surface': '--bf-appearance-token-color-success-bg',
  'color.status.success.border': '--bf-appearance-token-color-success-border',
  'color.status.warning.content': '--bf-appearance-token-color-warning',
  'color.status.warning.surface': '--bf-appearance-token-color-warning-bg',
  'color.status.warning.border': '--bf-appearance-token-color-warning-border',
  'color.status.danger.content': '--bf-appearance-token-color-error',
  'color.status.danger.surface': '--bf-appearance-token-color-error-bg',
  'color.status.danger.border': '--bf-appearance-token-color-error-border',
  'shadow.xs': '--bf-appearance-token-shadow-xs',
  'shadow.sm': '--bf-appearance-token-shadow-sm',
  'shadow.base': '--bf-appearance-token-shadow-base',
  'shadow.lg': '--bf-appearance-token-shadow-lg',
  'shadow.xl': '--bf-appearance-token-shadow-xl',
  'shadow.raised': '--bf-appearance-token-shadow-sm',
  'shadow.overlay': '--bf-appearance-token-shadow-lg',
  'shadow.accentGlow': '--bf-appearance-token-glow-shadow-blue',
  'shadow.innerHighlight': '--bf-appearance-token-inner-glow-top',
  'shadow.innerHighlightHover': '--bf-appearance-token-inner-glow-top-hover',
  'effect.blur.subtle': '--bf-appearance-token-blur-subtle',
  'effect.blur.base': '--bf-appearance-token-blur-base',
  'effect.blur.medium': '--bf-appearance-token-blur-medium',
  'opacity.disabled': '--bf-appearance-token-opacity-disabled',
  'opacity.hover': '--bf-appearance-token-opacity-hover',
  'opacity.focus': '--bf-appearance-token-opacity-focus',
  'opacity.muted': '--bf-appearance-token-opacity-hover',
} as const satisfies Partial<Record<ThemeTokenName, string>>;

const LEGACY_CHROME_TOKEN_SOURCES = {
  'color.surface.canvas': '--bf-appearance-token-chrome-bg-primary',
  'color.surface.panel': '--bf-appearance-token-chrome-bg-secondary',
  'color.surface.raised': '--bf-appearance-token-chrome-bg-elevated',
  'color.surface.scene': '--bf-appearance-token-chrome-bg-scene',
  'color.surface.workbench': '--bf-appearance-token-chrome-bg-workbench',
  'color.surface.tertiary': '--bf-appearance-token-chrome-bg-tertiary',
  'color.surface.chrome': '--bf-appearance-token-chrome-bg-primary',
  'color.surface.subtle': '--bf-appearance-token-chrome-element-bg-subtle',
  'color.content.primary': '--bf-appearance-token-chrome-text-primary',
  'color.content.secondary': '--bf-appearance-token-chrome-text-secondary',
  'color.content.muted': '--bf-appearance-token-chrome-text-muted',
  'color.content.disabled': '--bf-appearance-token-chrome-text-disabled',
  'color.accent.default': '--bf-appearance-token-chrome-accent-500',
  'color.accent.hover': '--bf-appearance-token-chrome-accent-600',
  'color.border.subtle': '--bf-appearance-token-chrome-border-subtle',
  'color.border.default': '--bf-appearance-token-chrome-border-base',
  'color.border.strong': '--bf-appearance-token-chrome-border-strong',
  'color.action.neutral.border': '--bf-appearance-token-chrome-border-base',
  'color.action.neutral.content': '--bf-appearance-token-chrome-text-secondary',
  'color.action.neutral.contentDisabled': '--bf-appearance-token-chrome-text-disabled',
  'color.action.neutral.fillBorder': '--bf-appearance-token-chrome-element-bg-base',
  'color.action.neutral.surface': '--bf-appearance-token-chrome-element-bg-base',
  'color.action.neutral.surfaceHover': '--bf-appearance-token-chrome-element-bg-medium',
  'color.action.neutral.surfacePressed': '--bf-appearance-token-chrome-element-bg-strong',
  'color.action.secondary.background': '--bf-appearance-token-chrome-accent-100',
  'color.action.secondary.hover': '--bf-appearance-token-chrome-accent-200',
  'color.action.secondary.pressed': '--bf-appearance-token-chrome-accent-300',
  'color.action.secondary.content': '--bf-appearance-token-chrome-accent-600',
  'color.action.quiet.hover': '--bf-appearance-token-chrome-element-bg-soft',
  'color.action.quiet.pressed': '--bf-appearance-token-chrome-element-bg-base',
  'color.action.quiet.content': '--bf-appearance-token-chrome-text-secondary',
  'color.field.background': '--bf-appearance-token-chrome-bg-secondary',
  'color.field.backgroundHover': '--bf-appearance-token-chrome-element-bg-subtle',
  'color.field.border': '--bf-appearance-token-chrome-border-base',
  'color.field.borderHover': '--bf-appearance-token-chrome-border-medium',
  'color.field.borderFocus': '--bf-appearance-token-chrome-accent-500',
  'color.focus.ring': '--bf-appearance-token-chrome-accent-500',
  'color.scrollbar.thumb': '--bf-appearance-token-chrome-scrollbar-thumb',
  'color.scrollbar.thumbHover': '--bf-appearance-token-chrome-scrollbar-thumb-hover',
} as const satisfies Partial<Record<ThemeTokenName, string>>;

const LEGACY_COMPONENT_TOKEN_SOURCES: Readonly<Record<string, AppearanceThemeTokenName>> = {
  '--bf-appearance-token-config-page-section-bg': '--bf-component-config-page-section-background',
  '--bf-appearance-token-config-page-section-border': '--bf-component-config-page-section-border',
  '--bf-appearance-token-config-page-section-border-width': '--bf-component-config-page-section-border-width',
  '--bf-appearance-token-config-page-section-shadow': '--bf-component-config-page-section-shadow',
  '--bf-appearance-token-config-page-divider': '--bf-component-config-page-divider',
  '--bf-appearance-token-config-page-row-hover-bg': '--bf-component-config-page-row-hover-background',
  '--bf-appearance-token-scene-viewport-border-width': '--bf-component-scene-viewport-border-width',
  '--bf-appearance-token-badge-padding-y': '--bf-component-badge-padding-block',
};

const LEGACY_COMPONENT_SURFACE_IDS: Readonly<Record<string, string>> = {
  'basics-config': 'application-settings',
  'ai-model-config': 'model-settings',
  'appearance-config': 'appearance-settings',
  'session-config': 'runtime-settings',
  'worktrees-config': 'worktree-settings',
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

const ROOT_TOKEN_NAMES = new Set<string>(APPEARANCE_ROOT_TOKEN_NAMES);
const SCOPED_TOKEN_NAMES = new Set<string>(APPEARANCE_SCOPED_TOKEN_NAMES);

function projectLegacyThemeTokens(
  sourceTokens: Readonly<Record<string, unknown>>,
  sources: Partial<Record<ThemeTokenName, string>>,
): Record<AppearanceThemeTokenName, string> {
  const projected: Record<AppearanceThemeTokenName, string> = {};
  Object.entries(sources).forEach(([themeTokenName, legacyTokenName]) => {
    if (!legacyTokenName) return;
    const value = sourceTokens[legacyTokenName];
    if (typeof value !== 'string') return;
    projected[themeCssVariables[themeTokenName as ThemeTokenName] as AppearanceThemeTokenName] = value;
  });
  return projected;
}

function migrateLegacyOwnedTokenName(name: string): AppearanceThemeTokenName | null {
  const explicit = LEGACY_COMPONENT_TOKEN_SOURCES[name];
  if (explicit) return explicit;
  if (name.startsWith('--bf-appearance-token-domain-')) {
    return `--bf-domain-${name.slice('--bf-appearance-token-domain-'.length)}`;
  }
  if (name.startsWith('--bf-appearance-token-language-')) {
    return `--bf-domain-language-${name.slice('--bf-appearance-token-language-'.length)}`;
  }
  if (name.startsWith('--bf-appearance-token-prism-')) {
    return `--bf-domain-prism-${name.slice('--bf-appearance-token-prism-'.length)}`;
  }
  const gitNames: Readonly<Record<string, AppearanceThemeTokenName>> = {
    '--bf-appearance-token-git-color-branch': '--bf-domain-git-branch',
    '--bf-appearance-token-git-color-branch-bg': '--bf-domain-git-branch-background',
    '--bf-appearance-token-git-color-branch-bg-hover': '--bf-domain-git-branch-background-hover',
    '--bf-appearance-token-git-color-changes': '--bf-domain-git-changes',
    '--bf-appearance-token-git-color-added': '--bf-domain-git-added',
    '--bf-appearance-token-git-color-deleted': '--bf-domain-git-deleted',
    '--bf-appearance-token-git-color-staged': '--bf-domain-git-staged',
  };
  return gitNames[name] ?? null;
}

function migrateLegacyRootTokens(
  sourceTokens: Readonly<Record<string, unknown>>,
  background: unknown,
): Record<AppearanceThemeTokenName, string> {
  const migrated = projectLegacyThemeTokens(sourceTokens, LEGACY_THEME_TOKEN_SOURCES);
  Object.entries(sourceTokens).forEach(([name, value]) => {
    if (typeof value !== 'string') return;
    if (ROOT_TOKEN_NAMES.has(name)) {
      migrated[name as AppearanceThemeTokenName] = value;
      return;
    }
    const target = migrateLegacyOwnedTokenName(name);
    if (target && ROOT_TOKEN_NAMES.has(target)) migrated[target] = value;
  });
  const chromeSurface = themeCssVariables['color.surface.chrome'] as AppearanceThemeTokenName;
  if (!migrated[chromeSurface] && typeof background === 'string') migrated[chromeSurface] = background;
  return migrated;
}

function migrateLegacyChromeTokens(
  sourceTokens: Readonly<Record<string, unknown>>,
): Record<AppearanceThemeTokenName, string> | undefined {
  const migrated = projectLegacyThemeTokens(sourceTokens, LEGACY_CHROME_TOKEN_SOURCES);
  const filtered = Object.fromEntries(
    Object.entries(migrated).filter(([name]) => SCOPED_TOKEN_NAMES.has(name)),
  ) as Record<AppearanceThemeTokenName, string>;
  return Object.keys(filtered).length > 0 ? filtered : undefined;
}

function migrateLegacyRendererDefinitions(value: unknown): Record<string, unknown> | undefined {
  if (!isRecord(value)) return undefined;
  const renderers = { ...value };
  const legacyDefinition = renderers['css-tokens'];
  if (isRecord(legacyDefinition) && isRecord(legacyDefinition.settings)) {
    const sourceTokens = isRecord(legacyDefinition.settings.tokens)
      ? legacyDefinition.settings.tokens
      : {};
    const tokens = migrateLegacyRootTokens(sourceTokens, legacyDefinition.settings.background);
    const chrome = migrateLegacyChromeTokens(sourceTokens);
    renderers['theme-tokens'] = {
      version: 1,
      settings: {
        tokens,
        ...(chrome ? { scopes: { chrome } } : {}),
      },
    };

    const widgetDefinition = renderers['generative-widget'];
    if (isRecord(widgetDefinition) && isRecord(widgetDefinition.settings)) {
      renderers['generative-widget'] = {
        ...widgetDefinition,
        settings: {
          ...widgetDefinition.settings,
          vars: Object.fromEntries(WIDGET_APPEARANCE_VARIABLE_NAMES.flatMap(name => (
            tokens[name] === undefined ? [] : [[name, tokens[name]]]
          ))),
        },
      };
    }
  }
  delete renderers['css-tokens'];
  return renderers;
}

const LEGACY_RUNTIME_VIEW_IDS: Readonly<Record<string, readonly string[]>> = {
  personalization: ['pet', 'session-workspace'],
  permissions: ['execution', 'device-control'],
};

const RETIRED_APPEARANCE_SETTINGS_PARTS = new Set([
  'packageGrid',
  'packageCard',
  'packageCardBody',
  'packageActiveIndicator',
  'packageEmpty',
]);

const RETIRED_COMPONENT_SURFACE_IDS = new Set(['button', 'card', 'switch', 'select']);

const RETIRED_COMPONENT_PARTS: Readonly<Record<string, ReadonlySet<string>>> = {
  'assistant-card': new Set(['configure', 'newSession']),
  'branch-quick-switch': new Set(['list', 'item', 'itemName']),
  'canvas-tab-overflow': new Set(['list', 'menu', 'missionControl', 'divider', 'item', 'itemClose']),
  'computer-use-tool-card': new Set(['settingsButton']),
  'context-menu': new Set(['item', 'separator', 'icon', 'label', 'shortcut', 'submenuArrow', 'submenu']),
  'context-list': new Set(['clear']),
  'copy-output-button': new Set(['action', 'icon', 'text']),
  'create-agent-page': new Set(['back']),
  'font-preference': new Set(['resetButton', 'levelGroup', 'levelButton']),
  'git-diff-view': new Set(['typeSwitcher', 'typeOption']),
  'git-nav': new Set(['sections']),
  'image-analysis-card': new Set(['expand']),
  'image-viewer': new Set(['toolbar', 'controls', 'action']),
  'markdown-editor': new Set(['modeToggle']),
  'market-account-controls': new Set(['menu', 'menuItem']),
  'mini-app-tool-display': new Set(['open']),
  'editor-breadcrumb': new Set(['menuHeader', 'menuBack', 'menuTitle', 'list', 'listItem']),
  'file-mention-picker': new Set(['back', 'loading', 'empty', 'list', 'item', 'itemName', 'itemDetail']),
  'nav-panel': new Set([
    'assistantSessionMenu',
    'footerMenu', 'footerMenuItem', 'footerMenuDivider',
    'workspaceMenu', 'workspaceMenuItem', 'workspaceMenuDivider', 'workspaceMenuTitle', 'workspaceMenuEmpty',
    'sections',
  ]),
  'notification-button': new Set(['menuItem']),
  'settings-nav': new Set(['sections']),
  'shell-nav': new Set(['content']),
  'workspace-item': new Set(['menuPopover', 'menuItem', 'menuDivider']),
  'peer-device': new Set(['switcherDisconnect']),
  'review-session-summary-card': new Set(['open']),
  'sessions-section': new Set(['retry', 'aggregateRetry']),
  'smart-recommendations': new Set(['action', 'label', 'loading']),
  'status-bar-popover': new Set(['list', 'item', 'itemIcon']),
  'subagent-projection': new Set(['expandAction']),
  'tiptap-editor': new Set(['quickAction']),
};

const RETIRED_COMPONENT_STATES: Readonly<Record<string, ReadonlySet<string>>> = {
  'branch-quick-switch': new Set(['selected', 'current']),
  'context-menu': new Set(['disabled', 'submenuActive']),
  'editor-breadcrumb': new Set(['selected']),
  'file-mention-picker': new Set(['selected']),
  'status-bar-popover': new Set(['selected']),
};

const RETIRED_SCENE_PARTS: Readonly<Record<string, ReadonlySet<string>>> = {
  skills: new Set(['addAction', 'discoverContent', 'suiteSections']),
};

function migrateRuntimePartRule(value: unknown): unknown {
  if (!isRecord(value)) return value;
  let changed = false;
  let facets = value.facets;
  if (isRecord(facets) && isRecord(facets.view)) {
    const view = { ...facets.view };
    for (const [legacyViewId, canonicalViewIds] of Object.entries(LEGACY_RUNTIME_VIEW_IDS)) {
      if (!(legacyViewId in view)) continue;
      for (const canonicalViewId of canonicalViewIds) {
        if (!(canonicalViewId in view)) view[canonicalViewId] = view[legacyViewId];
      }
      delete view[legacyViewId];
      changed = true;
    }
    if (changed) facets = { ...facets, view };
  }

  let contexts = value.contexts;
  if (Array.isArray(contexts)) {
    const migratedContexts = contexts.flatMap((context) => {
      if (!isRecord(context)) {
        return [context];
      }
      const when = context.when;
      if (!isRecord(when)) return [context];
      const whenFacets = when.facets;
      if (!isRecord(whenFacets)) return [context];
      const legacyViewId = whenFacets.view;
      if (typeof legacyViewId !== 'string' || !LEGACY_RUNTIME_VIEW_IDS[legacyViewId]) {
        return [context];
      }
      changed = true;
      return LEGACY_RUNTIME_VIEW_IDS[legacyViewId].map((canonicalViewId) => ({
        ...context,
        when: {
          ...when,
          facets: { ...whenFacets, view: canonicalViewId },
        },
      }));
    });
    if (changed) contexts = migratedContexts;
  }

  return changed ? { ...value, facets, contexts } : value;
}

function migrateRuntimeSurface(value: unknown): unknown {
  if (!isRecord(value) || !isRecord(value.parts)) return value;
  return {
    ...value,
    parts: Object.fromEntries(
      Object.entries(value.parts).map(([partId, rule]) => [partId, migrateRuntimePartRule(rule)]),
    ),
  };
}

function migrateAppearanceSettingsSurface(value: unknown): unknown {
  if (!isRecord(value) || !isRecord(value.parts)) return value;
  const parts = Object.fromEntries(
    Object.entries(value.parts).filter(([partId]) => !RETIRED_APPEARANCE_SETTINGS_PARTS.has(partId)),
  );
  return Object.keys(parts).length === Object.keys(value.parts).length
    ? value
    : { ...value, parts };
}

function dropRetiredSurfaceParts(value: unknown, retiredParts: ReadonlySet<string>): unknown {
  if (!isRecord(value) || !isRecord(value.parts)) return value;
  const parts = Object.fromEntries(
    Object.entries(value.parts).filter(([partId]) => !retiredParts.has(partId)),
  );
  return Object.keys(parts).length === Object.keys(value.parts).length
    ? value
    : { ...value, parts };
}

function migrateRetiredSurfaceParts(
  surfaces: Record<string, unknown>,
  retiredPartsBySurface: Readonly<Record<string, ReadonlySet<string>>>,
): { changed: boolean; surfaces: Record<string, unknown> } {
  let changed = false;
  const migrated = { ...surfaces };
  for (const [surfaceId, retiredParts] of Object.entries(retiredPartsBySurface)) {
    if (!(surfaceId in migrated)) continue;
    const nextSurface = dropRetiredSurfaceParts(migrated[surfaceId], retiredParts);
    if (nextSurface === migrated[surfaceId]) continue;
    migrated[surfaceId] = nextSurface;
    changed = true;
  }
  return { changed, surfaces: changed ? migrated : surfaces };
}

function dropRetiredRuleStates(value: unknown, retiredStates: ReadonlySet<string>): unknown {
  if (!isRecord(value)) return value;
  let changed = false;
  const migrated = { ...value };

  if (isRecord(value.states)) {
    const states = Object.fromEntries(
      Object.entries(value.states).filter(([stateId]) => !retiredStates.has(stateId)),
    );
    if (Object.keys(states).length !== Object.keys(value.states).length) {
      changed = true;
      if (Object.keys(states).length > 0) migrated.states = states;
      else delete migrated.states;
    }
  }

  if (Array.isArray(value.contexts)) {
    const contexts = value.contexts.filter((context) => {
      if (!isRecord(context) || !isRecord(context.when) || !Array.isArray(context.when.states)) {
        return true;
      }
      return !context.when.states.some(state => (
        typeof state === 'string' && retiredStates.has(state)
      ));
    });
    if (contexts.length !== value.contexts.length) {
      changed = true;
      if (contexts.length > 0) migrated.contexts = contexts;
      else delete migrated.contexts;
    }
  }

  return changed ? migrated : value;
}

function dropRetiredSurfaceStates(value: unknown, retiredStates: ReadonlySet<string>): unknown {
  if (!isRecord(value) || !isRecord(value.parts)) return value;
  let changed = false;
  const parts = Object.fromEntries(
    Object.entries(value.parts).map(([partId, rule]) => {
      const migratedRule = dropRetiredRuleStates(rule, retiredStates);
      if (migratedRule !== rule) changed = true;
      return [partId, migratedRule];
    }),
  );
  return changed ? { ...value, parts } : value;
}

function migrateRetiredSurfaceStates(
  surfaces: Record<string, unknown>,
  retiredStatesBySurface: Readonly<Record<string, ReadonlySet<string>>>,
): { changed: boolean; surfaces: Record<string, unknown> } {
  let changed = false;
  const migrated = { ...surfaces };
  for (const [surfaceId, retiredStates] of Object.entries(retiredStatesBySurface)) {
    if (!(surfaceId in migrated)) continue;
    const nextSurface = dropRetiredSurfaceStates(migrated[surfaceId], retiredStates);
    if (nextSurface === migrated[surfaceId]) continue;
    migrated[surfaceId] = nextSurface;
    changed = true;
  }
  return { changed, surfaces: changed ? migrated : surfaces };
}

/**
 * Read-only upgrade boundary for Appearance packages authored against settings
 * surface ids that predate the Settings information architecture.
 */
export function migrateAppearancePackage(input: Record<string, unknown>): Record<string, unknown> {
  let components = isRecord(input.components) ? { ...input.components } : null;
  let scenes = isRecord(input.scenes) ? { ...input.scenes } : null;
  let renderers = isRecord(input.renderers) ? { ...input.renderers } : null;
  let changed = false;

  if (input.schemaVersion === LEGACY_APPEARANCE_SCHEMA_VERSION) {
    renderers = migrateLegacyRendererDefinitions(input.renderers) ?? null;
    changed = true;
  }

  if (components) {
    for (const [legacyId, canonicalId] of Object.entries(LEGACY_COMPONENT_SURFACE_IDS)) {
      if (!(legacyId in components)) continue;
      if (!(canonicalId in components)) {
        components[canonicalId] = legacyId === 'session-config'
          ? migrateRuntimeSurface(components[legacyId])
          : legacyId === 'appearance-config'
            ? migrateAppearanceSettingsSurface(components[legacyId])
            : components[legacyId];
      }
      delete components[legacyId];
      changed = true;
    }

    for (const retiredSurfaceId of RETIRED_COMPONENT_SURFACE_IDS) {
      if (!(retiredSurfaceId in components)) continue;
      delete components[retiredSurfaceId];
      changed = true;
    }

    const retiredComponents = migrateRetiredSurfaceParts(components, RETIRED_COMPONENT_PARTS);
    components = retiredComponents.surfaces;
    changed = changed || retiredComponents.changed;

    const retiredComponentStates = migrateRetiredSurfaceStates(components, RETIRED_COMPONENT_STATES);
    components = retiredComponentStates.surfaces;
    changed = changed || retiredComponentStates.changed;
  }

  if (scenes) {
    const retiredScenes = migrateRetiredSurfaceParts(scenes, RETIRED_SCENE_PARTS);
    scenes = retiredScenes.surfaces;
    changed = changed || retiredScenes.changed;
  }

  if (!changed) return input;

  const migrated = Object.create(Object.getPrototypeOf(input)) as Record<string, unknown>;
  Object.defineProperties(migrated, Object.getOwnPropertyDescriptors(input));
  if (input.schemaVersion === LEGACY_APPEARANCE_SCHEMA_VERSION) {
    Object.defineProperty(migrated, 'schemaVersion', {
      value: APPEARANCE_SCHEMA_VERSION,
      enumerable: true,
      configurable: true,
      writable: true,
    });
    if (renderers) {
      Object.defineProperty(migrated, 'renderers', {
        value: renderers,
        enumerable: true,
        configurable: true,
        writable: true,
      });
    } else if ('renderers' in migrated) {
      delete migrated.renderers;
    }
  }
  if (components) {
    Object.defineProperty(migrated, 'components', {
      value: components,
      enumerable: true,
      configurable: true,
      writable: true,
    });
  }
  if (scenes) {
    Object.defineProperty(migrated, 'scenes', {
      value: scenes,
      enumerable: true,
      configurable: true,
      writable: true,
    });
  }
  return migrated;
}
