import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const canvasEditorGroupAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-editor-group',
  parts: [{ id: 'root' }, { id: 'content' }, { id: 'tabContent' }, { id: 'empty' }, { id: 'emptyContent' }],
  facets: [{ id: 'group', attribute: 'data-bf-group', values: ['primary', 'secondary', 'tertiary'] }],
  states: [{ id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } }],
};
