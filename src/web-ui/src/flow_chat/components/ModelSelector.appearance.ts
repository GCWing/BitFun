import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const modelSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'model-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'name' },
    { id: 'contextUsage' },
    { id: 'dropdown' },
    { id: 'level' },
    { id: 'dropdownHeader' },
    { id: 'back' },
    { id: 'list' },
    { id: 'option' },
    { id: 'providerOption' },
    { id: 'optionMain' },
    { id: 'configRow' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
