import type {
  AppearanceRendererId,
  AppearanceRendererAdapter,
  AppearanceSurfaceDescriptor,
  RegisteredAppearanceRendererAdapter,
} from '../types';
import {
  DEFAULT_APPEARANCE_PROPERTY_PROFILE,
  getAppearanceProfileProperties,
  getDefaultForceableAppearanceProperties,
} from '../appearancePropertyProfiles';

const SURFACE_ID_PATTERN = /^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$/;
const CONTRACT_ID_PATTERN = /^[a-z][a-zA-Z0-9]*$/;
const FACET_ATTRIBUTE_PATTERN = /^data-openbitfun-[a-z][a-z0-9-]*$/;
const STATE_SUFFIX_PATTERN = /^(?::(?:hover|active|focus-visible|focus-within|disabled)(?::not\(:disabled\))?|:has\(input:checked\)|\[data-openbitfun-state~="[a-z][a-zA-Z0-9-]*"\])$/;

export class AppearanceRegistry {
  private readonly components = new Map<string, AppearanceSurfaceDescriptor>();
  private readonly scenes = new Map<string, AppearanceSurfaceDescriptor>();
  private readonly renderers = new Map<AppearanceRendererId, RegisteredAppearanceRendererAdapter>();
  private frozen = false;

  registerComponent(descriptor: AppearanceSurfaceDescriptor): this {
    this.assertMutable();
    this.assertSurfaceIdAvailable(descriptor.id, 'component');
    this.registerUnique(this.components, descriptor.id, this.prepareDescriptor(descriptor), 'component');
    return this;
  }

  registerScene(descriptor: AppearanceSurfaceDescriptor): this {
    this.assertMutable();
    this.assertSurfaceIdAvailable(descriptor.id, 'scene');
    this.registerUnique(this.scenes, descriptor.id, this.prepareDescriptor(descriptor), 'scene');
    return this;
  }

  registerRenderer<K extends AppearanceRendererId>(adapter: AppearanceRendererAdapter<K>): this {
    this.assertMutable();
    this.registerUnique(
      this.renderers,
      adapter.id,
      adapter as RegisteredAppearanceRendererAdapter,
      'renderer',
    );
    return this;
  }

  getComponent(id: string): AppearanceSurfaceDescriptor | undefined {
    return this.components.get(id);
  }

  getScene(id: string): AppearanceSurfaceDescriptor | undefined {
    return this.scenes.get(id);
  }

  getRenderer(id: string): RegisteredAppearanceRendererAdapter | undefined {
    return this.renderers.get(id as AppearanceRendererId);
  }

  getRendererAdapters(): readonly RegisteredAppearanceRendererAdapter[] {
    return [...this.renderers.values()];
  }

  getComponents(): readonly AppearanceSurfaceDescriptor[] {
    return [...this.components.values()];
  }

  getScenes(): readonly AppearanceSurfaceDescriptor[] {
    return [...this.scenes.values()];
  }

  freeze(): this {
    this.frozen = true;
    return this;
  }

  isFrozen(): boolean {
    return this.frozen;
  }

  private assertMutable(): void {
    if (this.frozen) {
      throw new Error('Appearance registry is frozen');
    }
  }

  private assertSurfaceIdAvailable(id: string, kind: 'component' | 'scene'): void {
    if (this.components.has(id) || this.scenes.has(id)) {
      throw new Error(`Duplicate appearance surface id: ${id} (${kind})`);
    }
  }

  private prepareDescriptor(descriptor: AppearanceSurfaceDescriptor): AppearanceSurfaceDescriptor {
    if (!SURFACE_ID_PATTERN.test(descriptor.id)) {
      throw new Error(`Invalid appearance surface id: ${descriptor.id}`);
    }
    if (descriptor.parts.length === 0) {
      throw new Error(`Appearance surface ${descriptor.id} must declare at least one part`);
    }
    this.assertUniqueIds(descriptor.id, 'part', descriptor.parts.map(part => part.id));
    this.assertUniqueIds(descriptor.id, 'facet', (descriptor.facets ?? []).map(facet => facet.id));
    this.assertUniqueIds(descriptor.id, 'state', (descriptor.states ?? []).map(state => state.id));

    const parts = descriptor.parts.map(part => {
      if (!CONTRACT_ID_PATTERN.test(part.id)) {
        throw new Error(`Invalid appearance part id: ${descriptor.id}.${part.id}`);
      }
      const propertyProfile = part.propertyProfile ?? DEFAULT_APPEARANCE_PROPERTY_PROFILE;
      const allowedProperties = part.allowedProperties ?? getAppearanceProfileProperties(propertyProfile);
      const allowedPropertySet = new Set(allowedProperties);
      const requestedForceableProperties = part.forceableProperties ?? getDefaultForceableAppearanceProperties();
      if (part.forceableProperties?.some(property => !allowedPropertySet.has(property))) {
        throw new Error(`Appearance part ${descriptor.id}.${part.id} forces a property outside its allowed contract`);
      }
      const forceableProperties = [...new Set(requestedForceableProperties)]
        .filter(property => allowedPropertySet.has(property));
      return Object.freeze({
        ...part,
        propertyProfile,
        allowedProperties: part.allowedProperties
          ? Object.freeze([...part.allowedProperties])
          : undefined,
        forceableProperties: Object.freeze(forceableProperties),
      });
    });
    const facets = (descriptor.facets ?? []).map(facet => {
      if (!CONTRACT_ID_PATTERN.test(facet.id) || !FACET_ATTRIBUTE_PATTERN.test(facet.attribute)) {
        throw new Error(`Invalid appearance facet: ${descriptor.id}.${facet.id}`);
      }
      if (facet.values.length === 0 || new Set(facet.values).size !== facet.values.length) {
        throw new Error(`Appearance facet ${descriptor.id}.${facet.id} must declare unique values`);
      }
      return Object.freeze({ ...facet, values: Object.freeze([...facet.values]) });
    });
    const states = (descriptor.states ?? []).map(state => {
      const selector = state.selector;
      if (!CONTRACT_ID_PATTERN.test(state.id)
        || !selector
        || !STATE_SUFFIX_PATTERN.test(selector.suffix)
        || (selector.kind !== 'self' && selector.kind !== 'ancestorPart')
        || (selector.kind === 'ancestorPart'
          && !parts.some(part => part.id === selector.part))) {
        throw new Error(`Invalid appearance state: ${descriptor.id}.${state.id}`);
      }
      return Object.freeze({ ...state, selector: Object.freeze({ ...selector }) });
    });

    return Object.freeze({
      ...descriptor,
      parts: Object.freeze(parts),
      facets: facets.length > 0 ? Object.freeze(facets) : undefined,
      states: states.length > 0 ? Object.freeze(states) : undefined,
    });
  }

  private assertUniqueIds(surfaceId: string, kind: string, ids: readonly string[]): void {
    if (new Set(ids).size !== ids.length) {
      throw new Error(`Appearance surface ${surfaceId} contains duplicate ${kind} ids`);
    }
  }

  private registerUnique<T>(map: Map<string, T>, id: string, value: T, kind: string): void {
    if (map.has(id)) {
      throw new Error(`Duplicate appearance ${kind} id: ${id}`);
    }
    map.set(id, value);
  }
}
