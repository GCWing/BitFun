import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fontPreferenceAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'font-preference',
  parts: [
    { id: 'root' },
    { id: 'customControls' },
    { id: 'numberInput' },
    { id: 'error' },
    { id: 'preview' },
    { id: 'flowChatControls' },
  ],
  states: [
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
