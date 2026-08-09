import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const copyOutputButtonAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'copy-output-button',
  parts: [{ id: 'root' }, { id: 'action' }, { id: 'icon' }, { id: 'text' }],
  facets: [{ id: 'action', attribute: 'data-bf-action', values: ['copy', 'edit'] }],
  states: [{ id: 'copied', selector: { kind: 'self', suffix: '[data-bf-state~="copied"]' } }],
};
