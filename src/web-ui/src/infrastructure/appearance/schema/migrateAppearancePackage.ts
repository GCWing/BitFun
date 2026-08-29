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

const RETIRED_COMPONENT_SURFACE_IDS = new Set(['button', 'switch']);

const RETIRED_COMPONENT_PARTS: Readonly<Record<string, ReadonlySet<string>>> = {
  'assistant-card': new Set(['configure', 'newSession']),
  'branch-quick-switch': new Set(['list']),
  'canvas-tab-overflow': new Set(['list']),
  'computer-use-tool-card': new Set(['settingsButton']),
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
  'subagent-projection': new Set(['expandAction']),
  'tiptap-editor': new Set(['quickAction']),
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

/**
 * Read-only upgrade boundary for Appearance packages authored against settings
 * surface ids that predate the Settings information architecture.
 */
export function migrateAppearancePackage(input: Record<string, unknown>): Record<string, unknown> {
  let components = isRecord(input.components) ? { ...input.components } : null;
  let scenes = isRecord(input.scenes) ? { ...input.scenes } : null;
  let changed = false;

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
  }

  if (scenes) {
    const retiredScenes = migrateRetiredSurfaceParts(scenes, RETIRED_SCENE_PARTS);
    scenes = retiredScenes.surfaces;
    changed = changed || retiredScenes.changed;
  }

  if (!changed) return input;

  const migrated = Object.create(Object.getPrototypeOf(input)) as Record<string, unknown>;
  Object.defineProperties(migrated, Object.getOwnPropertyDescriptors(input));
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
