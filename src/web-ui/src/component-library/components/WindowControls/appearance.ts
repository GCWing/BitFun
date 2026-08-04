import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const windowControlsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'window-controls',
  parts: [{ id: 'root' }, { id: 'minimize' }, { id: 'maximize' }, { id: 'close' }],
  states: [
    { id: 'disabled', selector: { kind: 'self', suffix: '[data-bf-state~="disabled"]' } },
    { id: 'maximized', selector: { kind: 'self', suffix: '[data-bf-state~="maximized"]' } },
  ],
};
