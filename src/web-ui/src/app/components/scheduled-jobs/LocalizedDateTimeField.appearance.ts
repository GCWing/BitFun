import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const localizedDateTimeFieldAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'localized-datetime-field',
  parts: [
    { id: 'root' },
  ],
  states: [
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
