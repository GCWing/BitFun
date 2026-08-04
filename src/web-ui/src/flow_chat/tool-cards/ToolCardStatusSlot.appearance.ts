import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const toolCardStatusSlotAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'tool-card-status-slot',
  parts: [{ id: 'root' }, { id: 'statusLayer' }, { id: 'iconLayer' }],
  facets: [{ id: 'defaultIcon', attribute: 'data-bf-default-icon', values: ['status', 'tool'] }],
};
