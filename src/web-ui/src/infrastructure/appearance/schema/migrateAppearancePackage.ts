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

/**
 * Read-only upgrade boundary for Appearance packages authored against settings
 * surface ids that predate the Settings information architecture.
 */
export function migrateAppearancePackage(input: Record<string, unknown>): Record<string, unknown> {
  if (!isRecord(input.components)) return input;

  const components = { ...input.components };
  let changed = false;
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

  if (!changed) return input;

  const migrated = Object.create(Object.getPrototypeOf(input)) as Record<string, unknown>;
  Object.defineProperties(migrated, Object.getOwnPropertyDescriptors(input));
  Object.defineProperty(migrated, 'components', {
    value: components,
    enumerable: true,
    configurable: true,
    writable: true,
  });
  return migrated;
}
