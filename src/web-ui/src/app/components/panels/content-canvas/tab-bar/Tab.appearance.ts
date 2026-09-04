import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasTabAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tab',
  parts: [
    { id: 'root', propertyProfile: 'control', visualRole: 'continuous-surface', continuityGroup: 'canvas-tabs' },
    { id: 'typeIcon', propertyProfile: 'paint', visualRole: 'decoration' },
    { id: 'title', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'dirtyIndicator', propertyProfile: 'paint', visualRole: 'decoration' },
    { id: 'action', propertyProfile: 'control', visualRole: 'control' },
  ],
  facets: [{ id: 'group', attribute: 'data-openbitfun-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="dirty"]' } },
    { id: 'dragging', selector: { kind: 'self', suffix: '[data-openbitfun-state~="dragging"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-openbitfun-state~="deleted"]' } },
    { id: 'pinned', selector: { kind: 'self', suffix: '[data-openbitfun-state~="pinned"]' } },
    { id: 'preview', selector: { kind: 'self', suffix: '[data-openbitfun-state~="preview"]' } },
  ],
};
