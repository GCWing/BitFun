import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const reasoningConfigPanelAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'reasoning-config-panel',
  parts: [
    { id: 'root' },
    { id: 'body' },
    { id: 'footer' },
    { id: 'error' },
    { id: 'actions' },
  ],
};
