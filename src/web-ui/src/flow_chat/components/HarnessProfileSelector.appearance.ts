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
      attribute: 'data-bf-presentation',
      values: ['standalone', 'menu-item'],
    },
    {
      id: 'profile',
      attribute: 'data-bf-profile',
      values: ['minimal', 'balanced', 'ultimate', 'creative', 'other'],
    },
  ],
  states: [
    { id: 'open', selector: { kind: 'self', suffix: '[data-bf-state~="open"]' } },
    { id: 'fixed', selector: { kind: 'self', suffix: '[data-bf-state~="fixed"]' } },
    { id: 'current', selector: { kind: 'self', suffix: '[data-bf-state~="current"]' } },
    { id: 'available', selector: { kind: 'self', suffix: '[data-bf-state~="available"]' } },
    { id: 'unavailable', selector: { kind: 'self', suffix: '[data-bf-state~="unavailable"]' } },
    { id: 'comingSoon', selector: { kind: 'self', suffix: '[data-bf-state~="coming-soon"]' } },
  ],
};
