import { describe, expect, it } from 'vitest';
import { AppearanceRegistry } from './AppearanceRegistry';

describe('AppearanceRegistry', () => {
  it('deep-freezes registered host surface contracts', () => {
    const descriptor = {
      id: 'test-surface',
      parts: [{ id: 'root', allowedProperties: ['color'] as const }],
      facets: [{ id: 'size', attribute: 'data-openbitfun-size' as const, values: ['small', 'large'] }],
      states: [{ id: 'active', selector: { kind: 'self' as const, suffix: '[data-openbitfun-state~="active"]' } }],
    };
    const registry = new AppearanceRegistry().registerComponent(descriptor).freeze();
    const registered = registry.getComponent('test-surface')!;

    expect(Object.isFrozen(registered)).toBe(true);
    expect(Object.isFrozen(registered.parts)).toBe(true);
    expect(Object.isFrozen(registered.parts[0])).toBe(true);
    expect(Object.isFrozen(registered.parts[0].allowedProperties)).toBe(true);
    expect(Object.isFrozen(registered.parts[0].forceableProperties)).toBe(true);
    expect(Object.isFrozen(registered.facets?.[0].values)).toBe(true);
  });

  it('applies safe property defaults and rejects forceable properties outside the part contract', () => {
    const registry = new AppearanceRegistry().registerComponent({
      id: 'default-contract',
      parts: [{ id: 'root' }],
    });
    const part = registry.getComponent('default-contract')!.parts[0];
    expect(part.propertyProfile).toBe('container');
    expect(part.forceableProperties).toContain('backgroundColor');
    expect(part.forceableProperties).not.toContain('position');

    expect(() => new AppearanceRegistry().registerComponent({
      id: 'invalid-forceable-contract',
      parts: [{
        id: 'root',
        allowedProperties: ['color'],
        forceableProperties: ['position'],
      }],
    })).toThrow('forces a property outside its allowed contract');
  });

  it('rejects duplicate ids and malformed host selectors', () => {
    expect(() => new AppearanceRegistry().registerComponent({
      id: 'duplicate-parts',
      parts: [{ id: 'root' }, { id: 'root' }],
    })).toThrow('duplicate part ids');

    const registry = new AppearanceRegistry().registerComponent({
      id: 'shared-id',
      parts: [{ id: 'root' }],
    });
    expect(() => registry.registerScene({
      id: 'shared-id',
      parts: [{ id: 'root' }],
    })).toThrow('Duplicate appearance surface id');

    expect(() => new AppearanceRegistry().registerComponent({
      id: 'unsafe-state',
      parts: [{ id: 'root' }],
      states: [{ id: 'unsafe', selector: { kind: 'self', suffix: ' body *' } }],
    })).toThrow('Invalid appearance state');
  });
});
