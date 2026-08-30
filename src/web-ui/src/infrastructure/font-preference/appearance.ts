import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const fontPreferenceAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'font-preference',
  parts: [
    { id: 'root' },
    { id: 'customControls' },
    { id: 'preview' },
  ],
};
