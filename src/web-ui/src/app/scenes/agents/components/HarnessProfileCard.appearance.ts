import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const harnessProfileCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'harness-profile-card',
  parts: [{ id: 'root', propertyProfile: 'control', visualRole: 'control' }],
  facets: [
    {
      id: 'profile',
      attribute: 'data-bf-profile',
      values: ['minimal', 'balanced', 'ultimate', 'creative'],
    },
  ],
  states: [
    { id: 'connected', selector: { kind: 'self', suffix: '[data-bf-state~="connected"]' } },
    { id: 'comingSoon', selector: { kind: 'self', suffix: '[data-bf-state~="coming-soon"]' } },
  ],
};
