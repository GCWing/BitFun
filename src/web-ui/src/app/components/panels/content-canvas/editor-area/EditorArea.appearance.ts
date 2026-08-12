import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasEditorAreaAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-editor-area',
  parts: [
    { id: 'root' }, { id: 'primary' }, { id: 'secondary' },
    { id: 'tertiary' }, { id: 'topRow' },
    { id: 'grid9Cell' },
  ],
  facets: [{ id: 'layout', attribute: 'data-bf-layout', values: ['none', 'horizontal', 'vertical', 'grid', 'grid9'] }],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } }],
};
