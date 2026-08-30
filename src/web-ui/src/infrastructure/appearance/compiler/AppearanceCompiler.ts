import type { AppearanceRegistry } from '../registry/AppearanceRegistry';
import { appearancePackageValidator } from '../schema/AppearancePackageValidator';
import { AppearancePackageValidationError } from '../schema/AppearancePackageValidationError';
import { migrateAppearancePackage } from '../schema/migrateAppearancePackage';
import { getAppearanceCompositionLayers } from '../builtins/composeAppearancePackage';
import type {
  AppearanceDiagnostic,
  AppearanceEasingValue,
  AppearanceGlobalTokens,
  AppearancePackage,
  AppearancePartRule,
  AppearanceReference,
  AppearanceShadowValue,
  AppearanceStyle,
  AppearanceStyleProperty,
  AppearanceSurfaceDefinition,
  AppearanceSurfaceDescriptor,
  ResolvedAppearance,
  ResolvedAppearanceStyle,
  ResolvedAppearanceSurface,
} from '../types';

const CSS_PROPERTIES: Record<AppearanceStyleProperty, string> = {
  backgroundColor: 'background-color',
  backgroundImage: 'background-image',
  backgroundSize: 'background-size',
  backgroundPosition: 'background-position',
  backgroundRepeat: 'background-repeat',
  backgroundImages: 'background-image',
  backgroundSizes: 'background-size',
  backgroundPositions: 'background-position',
  backgroundRepeats: 'background-repeat',
  backgroundBlendModes: 'background-blend-mode',
  color: 'color',
  caretColor: 'caret-color',
  accentColor: 'accent-color',
  textDecorationColor: 'text-decoration-color',
  borderColor: 'border-color',
  borderTopColor: 'border-top-color',
  borderRightColor: 'border-right-color',
  borderBottomColor: 'border-bottom-color',
  borderLeftColor: 'border-left-color',
  borderWidth: 'border-width',
  borderTopWidth: 'border-top-width',
  borderRightWidth: 'border-right-width',
  borderBottomWidth: 'border-bottom-width',
  borderLeftWidth: 'border-left-width',
  borderStyle: 'border-style',
  borderRadius: 'border-radius',
  borderTopLeftRadius: 'border-top-left-radius',
  borderTopRightRadius: 'border-top-right-radius',
  borderBottomRightRadius: 'border-bottom-right-radius',
  borderBottomLeftRadius: 'border-bottom-left-radius',
  boxSizing: 'box-sizing',
  outlineColor: 'outline-color',
  outlineWidth: 'outline-width',
  outlineOffset: 'outline-offset',
  outlineStyle: 'outline-style',
  boxShadow: 'box-shadow',
  opacity: 'opacity',
  textAlign: 'text-align',
  verticalAlign: 'vertical-align',
  textIndent: 'text-indent',
  textDecoration: 'text-decoration',
  textTransform: 'text-transform',
  textOverflow: 'text-overflow',
  whiteSpace: 'white-space',
  wordBreak: 'word-break',
  overflowWrap: 'overflow-wrap',
  display: 'display',
  flexDirection: 'flex-direction',
  flexWrap: 'flex-wrap',
  flexGrow: 'flex-grow',
  flexShrink: 'flex-shrink',
  flexBasis: 'flex-basis',
  alignItems: 'align-items',
  alignContent: 'align-content',
  alignSelf: 'align-self',
  justifyContent: 'justify-content',
  justifySelf: 'justify-self',
  justifyItems: 'justify-items',
  placeItems: 'place-items',
  placeContent: 'place-content',
  order: 'order',
  gridTemplateColumns: 'grid-template-columns',
  gridTemplateRows: 'grid-template-rows',
  gridAutoFlow: 'grid-auto-flow',
  gridColumnSpan: 'grid-column',
  gridRowSpan: 'grid-row',
  gap: 'gap',
  rowGap: 'row-gap',
  columnGap: 'column-gap',
  width: 'width',
  minWidth: 'min-width',
  maxWidth: 'max-width',
  height: 'height',
  minHeight: 'min-height',
  maxHeight: 'max-height',
  padding: 'padding',
  paddingBlock: 'padding-block',
  paddingInline: 'padding-inline',
  paddingTop: 'padding-top',
  paddingRight: 'padding-right',
  paddingBottom: 'padding-bottom',
  paddingLeft: 'padding-left',
  margin: 'margin',
  marginBlock: 'margin-block',
  marginInline: 'margin-inline',
  marginTop: 'margin-top',
  marginRight: 'margin-right',
  marginBottom: 'margin-bottom',
  marginLeft: 'margin-left',
  position: 'position',
  insetBlock: 'inset-block',
  insetInline: 'inset-inline',
  top: 'top',
  right: 'right',
  bottom: 'bottom',
  left: 'left',
  zIndex: 'z-index',
  aspectRatio: 'aspect-ratio',
  overflow: 'overflow',
  overflowX: 'overflow-x',
  overflowY: 'overflow-y',
  cursor: 'cursor',
  transition: 'transition',
  transform: 'transform',
  filter: 'filter',
  backdropBlur: 'backdrop-filter',
  objectFit: 'object-fit',
  objectPosition: 'object-position',
  mixBlendMode: 'mix-blend-mode',
  isolation: 'isolation',
};

interface CompileContext {
  pkg: AppearancePackage;
  revision: number;
  globals: Map<string, unknown>;
  concreteGlobals: Map<string, string>;
  diagnostics: AppearanceDiagnostic[];
}

interface CompiledRule {
  selector: string;
  style: ResolvedAppearanceStyle;
  important: boolean;
  forceableProperties: ReadonlySet<AppearanceStyleProperty>;
}

export class AppearanceCompiler {
  constructor(private readonly registry: AppearanceRegistry) {}

  compile(input: unknown, revision: number): ResolvedAppearance {
    const validation = appearancePackageValidator.validate(input, this.registry);
    if (!validation.valid || !validation.value) {
      throw new AppearancePackageValidationError(validation.errors);
    }
    const pkg = validation.value;
    const globals = this.flattenGlobals(pkg.globals);
    const context: CompileContext = {
      pkg,
      revision,
      globals,
      concreteGlobals: new Map(),
      diagnostics: validation.warnings.map(issue => ({ level: 'warning', ...issue })),
    };
    const globalDeclarations = this.compileGlobalDeclarations(context);
    const materials = Object.fromEntries(
      Object.entries(pkg.materials ?? {}).map(([id, definition]) => [
        id,
        this.normalizeResolvedStyle(this.serializeStyle(definition.style, context)),
      ]),
    );
    const rawCompositionLayers = getAppearanceCompositionLayers(pkg);
    const compositionLayers = rawCompositionLayers ? {
      base: migrateAppearancePackage(
        rawCompositionLayers.base as unknown as Record<string, unknown>,
      ) as unknown as AppearancePackage,
      override: migrateAppearancePackage(
        rawCompositionLayers.override as unknown as Record<string, unknown>,
      ) as unknown as AppearancePackage,
    } : undefined;
    const componentResult = this.compileSurfaceLayers(
      compositionLayers
        ? [compositionLayers.base.components ?? {}, compositionLayers.override.components ?? {}]
        : [pkg.components ?? {}],
      id => this.registry.getComponent(id),
      'data-bf-component',
      materials,
      context,
    );
    const sceneResult = this.compileSurfaceLayers(
      compositionLayers
        ? [compositionLayers.base.scenes ?? {}, compositionLayers.override.scenes ?? {}]
        : [pkg.scenes ?? {}],
      id => this.registry.getScene(id),
      'data-bf-scene',
      materials,
      context,
    );

    const rootSelector = `:root[data-bf-appearance="${pkg.id}"][data-bf-appearance-revision="${revision}"]`;
    const appearanceRules = [
      `${rootSelector}{${globalDeclarations}}`,
      ...componentResult.rules.map(rule => this.renderRule(rule)),
      ...sceneResult.rules.map(rule => this.renderRule(rule)),
    ].filter(Boolean).join('\n');
    const cssText = appearanceRules;

    return Object.freeze({
      revision,
      id: pkg.id,
      name: pkg.name,
      version: pkg.version,
      mode: pkg.mode,
      backgroundMedia: pkg.backgroundMedia ? Object.freeze({
        ...pkg.backgroundMedia,
        fit: pkg.backgroundMedia.fit ?? 'cover',
        position: pkg.backgroundMedia.position ?? 'center',
      }) : undefined,
      globals: Object.freeze(Object.fromEntries(
        [...globals.keys()].map(path => [path, this.resolveConcreteReference(path, context, new Set())]),
      )),
      materials: Object.freeze(materials),
      components: Object.freeze(componentResult.surfaces),
      scenes: Object.freeze(sceneResult.surfaces),
      renderers: Object.freeze(Object.fromEntries(
        Object.entries(pkg.renderers ?? {}).map(([id, definition]) => [id, Object.freeze({ ...definition.settings })]),
      )),
      assets: Object.freeze(Object.fromEntries(
        Object.entries(pkg.assets ?? {}).map(([id, definition]) => [id, Object.freeze({ definition })]),
      )),
      diagnostics: Object.freeze(context.diagnostics),
      cssText,
    });
  }

  private compileSurfaceLayers(
    layers: Record<string, AppearanceSurfaceDefinition>[],
    getDescriptor: (id: string) => AppearanceSurfaceDescriptor | undefined,
    surfaceAttribute: 'data-bf-component' | 'data-bf-scene',
    materials: Record<string, ResolvedAppearanceStyle>,
    context: CompileContext,
  ): { surfaces: Record<string, ResolvedAppearanceSurface>; rules: CompiledRule[] } {
    const surfaces: Record<string, ResolvedAppearanceSurface> = {};
    const rules: CompiledRule[] = [];
    layers.forEach(layer => {
      const compiled = this.compileSurfaces(layer, getDescriptor, surfaceAttribute, materials, context);
      rules.push(...compiled.rules);
      Object.entries(compiled.surfaces).forEach(([surfaceId, surface]) => {
        const existing = surfaces[surfaceId];
        const parts: Record<string, readonly ResolvedAppearanceStyle[]> = { ...(existing?.parts ?? {}) };
        Object.entries(surface.parts).forEach(([partId, styles]) => {
          parts[partId] = [...(parts[partId] ?? []), ...styles];
        });
        surfaces[surfaceId] = Object.freeze({ parts: Object.freeze(parts) });
      });
    });
    return { surfaces, rules };
  }

  private flattenGlobals(globals: AppearanceGlobalTokens | undefined): Map<string, unknown> {
    const flattened = new Map<string, unknown>();
    Object.entries(globals ?? {}).forEach(([group, values]) => {
      Object.entries(values ?? {}).forEach(([id, value]) => {
        flattened.set(`globals.${group}.${id}`, value);
      });
    });
    return flattened;
  }

  private compileGlobalDeclarations(context: CompileContext): string {
    return [...context.globals.entries()]
      .map(([path, value]) => `${this.referenceToVariable(path)}:${this.serializeValue(value, context)};`)
      .join('');
  }

  private compileSurfaces(
    definitions: Record<string, AppearanceSurfaceDefinition>,
    getDescriptor: (id: string) => AppearanceSurfaceDescriptor | undefined,
    surfaceAttribute: 'data-bf-component' | 'data-bf-scene',
    materials: Record<string, ResolvedAppearanceStyle>,
    context: CompileContext,
  ): { surfaces: Record<string, ResolvedAppearanceSurface>; rules: CompiledRule[] } {
    const surfaces: Record<string, ResolvedAppearanceSurface> = {};
    const rules: CompiledRule[] = [];
    Object.entries(definitions).forEach(([surfaceId, definition]) => {
      const descriptor = getDescriptor(surfaceId);
      if (!descriptor) return;
      const resolvedSurfaceAttribute = surfaceAttribute === 'data-bf-component'
        ? descriptor.componentAttribute ?? surfaceAttribute
        : surfaceAttribute;
      const partAttribute = resolvedSurfaceAttribute === 'data-bf-product-component'
        ? 'data-bf-product-part'
        : 'data-bf-part';
      const resolvedParts: Record<string, ResolvedAppearanceStyle[]> = {};
      Object.entries(definition.parts).forEach(([partId, partRule]) => {
        const baseSelector = `:root[data-bf-appearance="${context.pkg.id}"][data-bf-appearance-revision="${context.revision}"] [${resolvedSurfaceAttribute}="${surfaceId}"][${partAttribute}="${partId}"]`;
        const compiled = this.compilePart(baseSelector, partRule, descriptor, materials, context);
        resolvedParts[partId] = compiled.map(rule => rule.style);
        rules.push(...compiled);
      });
      surfaces[surfaceId] = Object.freeze({ parts: Object.freeze(resolvedParts) });
    });
    return { surfaces, rules };
  }

  private compilePart(
    baseSelector: string,
    rule: AppearancePartRule,
    descriptor: AppearanceSurfaceDescriptor,
    materials: Record<string, ResolvedAppearanceStyle>,
    context: CompileContext,
  ): CompiledRule[] {
    const rules: CompiledRule[] = [];
    const important = rule.cascade === 'override';
    const surfaceMatch = /^(.*) \[(data-bf-(?:component|product-component|scene))="([^"]+)"\]\[(data-bf-(?:part|product-part))="([^"]+)"\]$/.exec(baseSelector);
    if (!surfaceMatch) throw new Error(`Invalid host Appearance selector: ${baseSelector}`);
    const [, rootSelector, surfaceAttribute, surfaceId, partAttribute, partId] = surfaceMatch;
    const partDescriptor = descriptor.parts.find(candidate => candidate.id === partId);
    if (!partDescriptor) throw new Error(`Unknown Appearance part descriptor: ${surfaceId}.${partId}`);
    const forceableProperties = new Set(partDescriptor.forceableProperties ?? []);
    const compileRule = (selector: string, style: ResolvedAppearanceStyle): CompiledRule => {
      const normalized = this.normalizeResolvedStyle(style);
      if (important) {
        Object.keys(normalized).forEach(property => {
          if (forceableProperties.has(property as AppearanceStyleProperty)) return;
          context.diagnostics.push({
            level: 'warning',
            code: 'OVERRIDE_PROPERTY_NOT_FORCEABLE',
            path: selector,
            message: `${property} remains in normal cascade because the host contract does not allow forcing it`,
          });
        });
      }
      return { selector, style: normalized, important, forceableProperties };
    };
    const selectorFor = (
      facets: Record<string, string> = {},
      stateIds: readonly string[] = [],
    ): string => {
      let targetSelector = `[${surfaceAttribute}="${surfaceId}"][${partAttribute}="${partId}"]`;
      Object.entries(facets).forEach(([facetId, option]) => {
        const facet = descriptor.facets?.find(candidate => candidate.id === facetId);
        if (facet) targetSelector += `[${facet.attribute}="${option}"]`;
      });
      const ancestors: string[] = [];
      stateIds.forEach(stateId => {
        const state = descriptor.states?.find(candidate => candidate.id === stateId);
        if (!state) return;
        if (state.selector.kind === 'self' || state.selector.part === partId) {
          targetSelector += state.selector.suffix;
        } else {
          ancestors.push(
            `[${surfaceAttribute}="${surfaceId}"][${partAttribute}="${state.selector.part}"]${state.selector.suffix}`,
          );
        }
      });
      return `${rootSelector} ${[...ancestors, targetSelector].join(' ')}`;
    };
    const materialStyle = (rule.materials ?? []).reduce<ResolvedAppearanceStyle>(
      (combined, materialId) => ({ ...combined, ...(materials[materialId] ?? {}) }),
      {},
    );
    const base = { ...materialStyle, ...this.serializeStyle(rule.base ?? {}, context) };
    if (Object.keys(base).length > 0) {
      rules.push(compileRule(baseSelector, base));
      this.auditContrast(base, baseSelector, context);
    }

    Object.entries(rule.facets ?? {}).forEach(([facetId, options]) => {
      const facet = descriptor.facets?.find(candidate => candidate.id === facetId);
      if (!facet) return;
      Object.entries(options).forEach(([option, style]) => {
        const resolved = this.serializeStyle(style, context);
        rules.push(compileRule(selectorFor({ [facetId]: option }), resolved));
        this.auditContrast({ ...base, ...resolved }, `${baseSelector}.${facetId}.${option}`, context);
      });
    });

    Object.entries(rule.states ?? {}).forEach(([stateId, style]) => {
      const state = descriptor.states?.find(candidate => candidate.id === stateId);
      if (!state) return;
      const resolved = this.serializeStyle(style, context);
      rules.push(compileRule(selectorFor({}, [stateId]), resolved));
      this.auditContrast({ ...base, ...resolved }, `${baseSelector}.${stateId}`, context);
    });

    (rule.contexts ?? []).forEach(contextRule => {
      const selector = selectorFor(contextRule.when.facets, contextRule.when.states);
      const resolved = this.serializeStyle(contextRule.style, context);
      rules.push(compileRule(selector, resolved));
      this.auditContrast({ ...base, ...resolved }, selector, context);
    });
    return rules;
  }

  private serializeStyle(style: AppearanceStyle, context: CompileContext): ResolvedAppearanceStyle {
    const resolved: ResolvedAppearanceStyle = {};
    (Object.entries(style) as [AppearanceStyleProperty, unknown][]).forEach(([property, value]) => {
      resolved[property] = this.serializeProperty(property, value, context);
    });
    return Object.freeze(resolved);
  }

  private serializeProperty(property: AppearanceStyleProperty, value: unknown, context: CompileContext): string {
    if (property === 'backgroundImage') {
      return this.serializeAssetReference(value, context);
    }
    if (property === 'backgroundImages') {
      return (value as NonNullable<AppearanceStyle['backgroundImages']>)
        .map(asset => this.serializeAssetReference(asset, context))
        .join(', ');
    }
    if (property === 'backgroundSizes') {
      return (value as NonNullable<AppearanceStyle['backgroundSizes']>)
        .map(size => {
          if (typeof size === 'string') return size;
          const height = size.height ? ` ${this.serializeValue(size.height, context)}` : '';
          return `${this.serializeValue(size.width, context)}${height}`;
        })
        .join(', ');
    }
    if (property === 'backgroundPositions') {
      return (value as NonNullable<AppearanceStyle['backgroundPositions']>)
        .map(position => typeof position === 'string'
          ? position
          : `${this.serializeValue(position.x, context)} ${this.serializeValue(position.y, context)}`)
        .join(', ');
    }
    if (property === 'backgroundRepeats' || property === 'backgroundBlendModes') {
      return (value as string[]).join(', ');
    }
    if (property === 'boxShadow') {
      return (value as AppearanceShadowValue[]).map(shadow => this.serializeValue(shadow, context)).join(', ');
    }
    if (property === 'transition') {
      return (value as AppearanceStyle['transition'] ?? []).map(item => {
        const delay = item.delay ? ` ${this.serializeValue(item.delay, context)}` : '';
        return `${item.property} ${this.serializeValue(item.duration, context)} ${this.serializeValue(item.easing, context)}${delay}`;
      }).join(', ');
    }
    if (property === 'transform') {
      const transform = value as NonNullable<AppearanceStyle['transform']>;
      const parts: string[] = [];
      if (transform.translateX) parts.push(`translateX(${this.serializeValue(transform.translateX, context)})`);
      if (transform.translateY) parts.push(`translateY(${this.serializeValue(transform.translateY, context)})`);
      if (transform.rotate !== undefined) parts.push(`rotate(${transform.rotate}deg)`);
      if (transform.scale !== undefined) parts.push(`scale(${transform.scale})`);
      return parts.length > 0 ? parts.join(' ') : 'none';
    }
    if (property === 'gridTemplateColumns' || property === 'gridTemplateRows') {
      const template = value as NonNullable<AppearanceStyle[typeof property]>;
      return template.tracks.map(track => this.serializeValue(track, context)).join(' ');
    }
    if (property === 'gridColumnSpan' || property === 'gridRowSpan') {
      return `span ${this.serializeValue(value, context)}`;
    }
    if (property === 'aspectRatio') {
      const ratio = value as NonNullable<AppearanceStyle['aspectRatio']>;
      return `${ratio.width} / ${ratio.height}`;
    }
    if (property === 'filter') {
      const filters = value as NonNullable<AppearanceStyle['filter']>;
      if (filters.length === 0) return 'none';
      return filters.map(filter => {
        if (filter.kind === 'blur') return `blur(${this.serializeValue(filter.value, context)})`;
        if (filter.kind === 'hueRotate') return `hue-rotate(${filter.degrees}deg)`;
        return `${filter.kind}(${this.serializeValue(filter.value, context)})`;
      }).join(' ');
    }
    if (property === 'backdropBlur') {
      return `blur(${this.serializeValue(value, context)})`;
    }
    return this.serializeValue(value, context);
  }

  private serializeValue(value: unknown, context: CompileContext): string {
    if (typeof value === 'string') return value;
    if (!value || typeof value !== 'object') return String(value);
    const record = value as Record<string, unknown>;
    switch (record.kind) {
      case 'ref':
        return this.resolveCssReference((record as unknown as AppearanceReference).path, context);
      case 'hex':
        return String(record.value).toLowerCase();
      case 'rgb': {
        const alpha = record.a === undefined ? '' : ` / ${record.a}`;
        return `rgb(${record.r} ${record.g} ${record.b}${alpha})`;
      }
      case 'hsl': {
        const alpha = record.a === undefined ? '' : ` / ${record.a}`;
        return `hsl(${record.h} ${record.s}% ${record.l}%${alpha})`;
      }
      case 'transparent':
        return 'transparent';
      case 'px':
        return `${record.value}px`;
      case 'rem':
        return `${record.value}rem`;
      case 'em':
        return `${record.value}em`;
      case 'ch':
        return `${record.value}ch`;
      case 'vw':
        return `${record.value}vw`;
      case 'vh':
        return `${record.value}vh`;
      case 'percent':
        return `${record.value}%`;
      case 'lengthKeyword':
        return String(record.value);
      case 'fr':
        return `${record.value}fr`;
      case 'trackKeyword':
        return String(record.value);
      case 'zero':
        return '0';
      case 'number':
        return String(record.value);
      case 'ms':
        return `${record.value}ms`;
      case 'easing':
        return this.serializeEasing(value as AppearanceEasingValue);
      case 'none':
        return 'none';
      case 'shadow': {
        const shadow = value as AppearanceShadowValue & Record<string, unknown>;
        if (shadow.kind !== 'shadow') return 'none';
        const spread = shadow.spread ? ` ${this.serializeValue(shadow.spread, context)}` : '';
        return `${shadow.inset ? 'inset ' : ''}${this.serializeValue(shadow.x, context)} ${this.serializeValue(shadow.y, context)} ${this.serializeValue(shadow.blur, context)}${spread} ${this.serializeValue(shadow.color, context)}`;
      }
      default:
        throw new Error(`Unsupported structured appearance value: ${String(record.kind)}`);
    }
  }

  private serializeEasing(value: AppearanceEasingValue): string {
    if (value.kind === 'ref') return this.referenceToVariable(value.path);
    const easings = {
      linear: 'linear',
      standard: 'cubic-bezier(0.23, 1, 0.32, 1)',
      decelerate: 'cubic-bezier(0.23, 1, 0.32, 1)',
      accelerate: 'cubic-bezier(0.3, 0, 1, 1)',
    } as const;
    return easings[value.value];
  }

  private resolveCssReference(path: string, context: CompileContext): string {
    if (!context.globals.has(path)) {
      throw new Error(`Unknown appearance token reference: ${path}`);
    }
    return `var(${this.referenceToVariable(path)})`;
  }

  private referenceToVariable(path: string): string {
    return `--bf-appearance-${path.replace(/^globals\./, '').replace(/\./g, '-')}`;
  }

  private assetVariable(assetId: string): string {
    return `--bf-appearance-asset-${assetId.replace(/\./g, '-')}`;
  }

  private serializeAssetReference(value: unknown, context: CompileContext): string {
    const assetId = (value as { assetId: string }).assetId;
    if (!(assetId in (context.pkg.assets ?? {}))) {
      throw new Error(`Unknown appearance asset: ${assetId}`);
    }
    return `var(${this.assetVariable(assetId)}, none)`;
  }

  private resolveConcreteReference(path: string, context: CompileContext, stack: Set<string>): string {
    const cached = context.concreteGlobals.get(path);
    if (cached !== undefined) return cached;
    if (stack.has(path)) throw new Error(`Circular appearance token reference: ${path}`);
    const value = context.globals.get(path);
    if (value === undefined) throw new Error(`Unknown appearance token reference: ${path}`);
    stack.add(path);
    let concrete: string;
    if (this.isReference(value)) {
      concrete = this.resolveConcreteReference(value.path, context, stack);
    } else {
      concrete = this.serializeValue(value, context);
    }
    stack.delete(path);
    context.concreteGlobals.set(path, concrete);
    return concrete;
  }

  private isReference(value: unknown): value is AppearanceReference {
    return Boolean(value && typeof value === 'object' && (value as { kind?: unknown }).kind === 'ref');
  }

  private renderRule(rule: CompiledRule): string {
    const declarations = (Object.keys(CSS_PROPERTIES) as AppearanceStyleProperty[])
      .filter(property => rule.style[property] !== undefined)
      .map(property => {
        const priority = rule.important && rule.forceableProperties.has(property) ? ' !important' : '';
        return `${CSS_PROPERTIES[property]}:${rule.style[property]}${priority};`;
      })
      .join('');
    return declarations ? `${rule.selector}{${declarations}}` : '';
  }

  private normalizeResolvedStyle(style: ResolvedAppearanceStyle): ResolvedAppearanceStyle {
    const needsBorderReset = style.borderStyle !== undefined && style.borderWidth === undefined;
    const needsOutlineStyle = (style.outlineWidth !== undefined || style.outlineColor !== undefined)
      && style.outlineStyle === undefined;
    if (!needsBorderReset && !needsOutlineStyle) return Object.freeze({ ...style });
    return Object.freeze({
      ...(needsBorderReset ? { borderWidth: '0' } : {}),
      ...style,
      ...(needsOutlineStyle ? { outlineStyle: 'solid' } : {}),
    });
  }

  private auditContrast(style: ResolvedAppearanceStyle, path: string, context: CompileContext): void {
    if (!style.color || !style.backgroundColor) return;
    const foreground = this.resolveConcreteCssValue(style.color, context);
    const background = this.resolveConcreteCssValue(style.backgroundColor, context);
    const ratio = this.contrastRatio(foreground, background);
    if (ratio !== null && ratio < 3) {
      context.diagnostics.push({
        level: 'warning',
        code: 'LOW_CONTRAST',
        path,
        message: `Foreground/background contrast is ${ratio.toFixed(2)}:1`,
      });
    }
  }

  private resolveConcreteCssValue(value: string, context: CompileContext): string {
    const match = /^var\((--bf-appearance-[^)]+)\)$/.exec(value);
    if (!match) return value;
    const path = [...context.globals.keys()].find(candidate => this.referenceToVariable(candidate) === match[1]);
    return path ? this.resolveConcreteReference(path, context, new Set()) : value;
  }

  private contrastRatio(foreground: string, background: string): number | null {
    const fg = this.parseColor(foreground);
    const bg = this.parseColor(background);
    if (!fg || !bg || fg[3] < 1 || bg[3] < 1) return null;
    const fgLum = this.relativeLuminance(fg);
    const bgLum = this.relativeLuminance(bg);
    return (Math.max(fgLum, bgLum) + 0.05) / (Math.min(fgLum, bgLum) + 0.05);
  }

  private parseColor(value: string): [number, number, number, number] | null {
    const hex = /^#([0-9a-f]{3,8})$/i.exec(value);
    if (hex) {
      const raw = hex[1];
      const expanded = raw.length <= 4 ? raw.split('').map(char => char + char).join('') : raw;
      const alpha = expanded.length === 8 ? parseInt(expanded.slice(6, 8), 16) / 255 : 1;
      return [parseInt(expanded.slice(0, 2), 16), parseInt(expanded.slice(2, 4), 16), parseInt(expanded.slice(4, 6), 16), alpha];
    }
    const rgb = /^rgb\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)(?:\s*\/\s*([\d.]+))?\s*\)$/.exec(value);
    if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3]), rgb[4] === undefined ? 1 : Number(rgb[4])];
    const hsl = /^hsl\(\s*([\d.]+)\s+([\d.]+)%\s+([\d.]+)%(?:\s*\/\s*([\d.]+))?\s*\)$/.exec(value);
    if (!hsl) return null;
    const h = Number(hsl[1]) / 360;
    const s = Number(hsl[2]) / 100;
    const l = Number(hsl[3]) / 100;
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    const channel = (offset: number) => {
      let t = h + offset;
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1 / 6) return p + (q - p) * 6 * t;
      if (t < 1 / 2) return q;
      if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
      return p;
    };
    return [channel(1 / 3) * 255, channel(0) * 255, channel(-1 / 3) * 255, hsl[4] === undefined ? 1 : Number(hsl[4])];
  }

  private relativeLuminance(color: [number, number, number, number]): number {
    const channels = color.slice(0, 3).map(channel => {
      const normalized = channel / 255;
      return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
    });
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  }
}
