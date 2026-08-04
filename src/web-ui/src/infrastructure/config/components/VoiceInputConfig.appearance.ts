import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const voiceInputConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'voice-input-config',
  parts: [{ id: 'root' }, { id: 'modelList' }, { id: 'modelCard' }, { id: 'modelActions' }],
};
