import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const acpModeSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'acp-mode-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'label' },
    { id: 'menu' },
    { id: 'header' },
    { id: 'option' },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
    { id: 'selected', selector: { kind: 'self', suffix: '[data-openbitfun-state~="selected"]' } },
  ],
};
