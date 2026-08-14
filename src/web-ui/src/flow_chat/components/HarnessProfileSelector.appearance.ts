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
    // The gear the composer is set to but the session has not adopted yet.
    { id: 'pending', selector: { kind: 'self', suffix: '[data-bf-state~="pending"]' } },
    { id: 'newSession', selector: { kind: 'self', suffix: '[data-bf-state~="new-session"]' } },
    { id: 'comingSoon', selector: { kind: 'self', suffix: '[data-bf-state~="coming-soon"]' } },
  ],
};
