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
  facets: [{ id: 'group', attribute: 'data-bf-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-bf-state~="dirty"]' } },
    { id: 'dragging', selector: { kind: 'self', suffix: '[data-bf-state~="dragging"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-bf-state~="deleted"]' } },
    { id: 'pinned', selector: { kind: 'self', suffix: '[data-bf-state~="pinned"]' } },
    { id: 'preview', selector: { kind: 'self', suffix: '[data-bf-state~="preview"]' } },
  ],
};
