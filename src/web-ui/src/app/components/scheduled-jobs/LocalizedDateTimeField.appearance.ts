import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const localizedDateTimeFieldAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'localized-datetime-field',
  parts: [
    { id: 'root' },
    { id: 'date' },
    { id: 'time' },
  ],
  states: [
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
