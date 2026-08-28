import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const reasoningPresetSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'reasoning-preset-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'menu' },
    { id: 'header' },
    { id: 'auto' },
    { id: 'options' },
    { id: 'option' },
  ],
  facets: [
    {
      id: 'presentation',
      attribute: 'data-bf-presentation',
      values: ['meter', 'label'],
    },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
