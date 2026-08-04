import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasThumbnailAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-thumbnail',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'icon' }, { id: 'title' },
    { id: 'actions' }, { id: 'action' }, { id: 'preview' }, { id: 'code' },
    { id: 'placeholder' }, { id: 'groupBadge' },
  ],
  facets: [{ id: 'group', attribute: 'data-bf-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-bf-state~="dirty"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-bf-state~="deleted"]' } },
    { id: 'pinned', selector: { kind: 'self', suffix: '[data-bf-state~="pinned"]' } },
    { id: 'preview', selector: { kind: 'self', suffix: '[data-bf-state~="preview"]' } },
  ],
};
