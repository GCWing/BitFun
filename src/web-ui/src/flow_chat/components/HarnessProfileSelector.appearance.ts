import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const harnessProfileSelectorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'harness-selector',
  parts: [
    { id: 'root' },
    { id: 'trigger' },
    { id: 'menu' },
    { id: 'profile' },
  ],
  facets: [
    {
      id: 'profile',
      attribute: 'data-bf-profile',
      values: ['minimal', 'balanced', 'ultimate'],
    },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-bf-state~="current"]' } },
    { id: 'compatibility', selector: { kind: 'self', suffix: '[data-bf-state~="compatibility"]' } },
    { id: 'comingSoon', selector: { kind: 'self', suffix: '[data-bf-state~="coming-soon"]' } },
  ],
};
