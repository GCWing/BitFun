import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const modelSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'model-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'name' },
    { id: 'contextUsage' },
    { id: 'dropdown' },
    { id: 'dropdownHeader' },
    { id: 'list' },
    { id: 'option' },
    { id: 'optionMain' },
    { id: 'configRow' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
  ],
};
