import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const harnessProfileStepAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'harness-profile-step',
  parts: [{ id: 'root', visualRole: 'content' }],
  facets: [
    {
      id: 'profile',
      attribute: 'data-bf-profile',
      values: ['minimal', 'balanced', 'ultimate', 'creative'],
    },
  ],
};
