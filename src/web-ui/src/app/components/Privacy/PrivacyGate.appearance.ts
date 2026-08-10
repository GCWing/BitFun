import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const privacyGateAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'privacy-gate',
  parts: [
    { id: 'root', visualRole: 'dialog' },
    { id: 'metadata', visualRole: 'content' },
    { id: 'document', visualRole: 'content' },
    { id: 'footer', visualRole: 'toolbar' },
    { id: 'actions', visualRole: 'toolbar' },
  ],
};
