import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fontPreferenceAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'font-preference',
  parts: [
    { id: 'root' },
    { id: 'levelGroup' },
    { id: 'levelButton' },
    { id: 'customControls' },
    { id: 'numberInput' },
    { id: 'error' },
    { id: 'preview' },
    { id: 'flowChatControls' },
    { id: 'resetButton' },
  ],
  facets: [
    { id: 'level', attribute: 'data-bf-level', values: ['compact', 'small', 'default', 'medium', 'large', 'custom'] },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
  ],
};
