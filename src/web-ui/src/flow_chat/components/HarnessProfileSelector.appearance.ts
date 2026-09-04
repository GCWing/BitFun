import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const harnessProfileSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'harness-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'menu' },
    { id: 'profile' },
    { id: 'agent' },
  ],
  facets: [
    {
      id: 'presentation',
      attribute: 'data-openbitfun-presentation',
      values: ['standalone', 'menu-item'],
    },
    {
      id: 'profile',
      attribute: 'data-openbitfun-profile',
      values: ['minimal', 'balanced', 'ultimate', 'creative', 'other'],
    },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-openbitfun-state~="open"]' } },
    { id: 'fixed', selector: { kind: 'self', suffix: '[data-openbitfun-state~="fixed"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-openbitfun-state~="current"]' } },
    { id: 'available', selector: { kind: 'self', suffix: '[data-openbitfun-state~="available"]' } },
    { id: 'unavailable', selector: { kind: 'self', suffix: '[data-openbitfun-state~="unavailable"]' } },
    { id: 'comingSoon', selector: { kind: 'self', suffix: '[data-openbitfun-state~="coming-soon"]' } },
  ],
};
