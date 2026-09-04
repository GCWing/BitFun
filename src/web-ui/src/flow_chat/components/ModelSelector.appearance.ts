import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const modelSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'model-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'name' },
    { id: 'reasoningSummary' },
    { id: 'dropdown' },
    { id: 'level' },
    { id: 'back' },
    { id: 'list' },
    { id: 'option' },
    { id: 'providerOption' },
    { id: 'optionMain' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
  ],
};
