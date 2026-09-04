import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasThumbnailAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-thumbnail',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'icon' }, { id: 'title' },
    { id: 'actions' }, { id: 'action' }, { id: 'preview' }, { id: 'code' },
    { id: 'placeholder' }, { id: 'groupBadge' },
  ],
  facets: [{ id: 'group', attribute: 'data-openbitfun-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-openbitfun-state~="active"]' } },
    { id: 'dirty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="dirty"]' } },
    { id: 'deleted', selector: { kind: 'self', suffix: '[data-openbitfun-state~="deleted"]' } },
    { id: 'pinned', selector: { kind: 'self', suffix: '[data-openbitfun-state~="pinned"]' } },
    { id: 'preview', selector: { kind: 'self', suffix: '[data-openbitfun-state~="preview"]' } },
  ],
};
