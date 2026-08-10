import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const filterPillAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'filter-pill',
  parts: [{ id: 'root' }, { id: 'label' }, { id: 'count' }, { id: 'group' }],
  states: [
    { id: 'active', selector: { kind: 'self', suffix: '[data-bf-state~="active"]' } },
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
  ],
};
