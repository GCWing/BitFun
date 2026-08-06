import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const beeColonyMonitorAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'bee-colony-monitor',
  parts: [
    { id: 'root' }, { id: 'backdrop' }, { id: 'trigger' }, { id: 'panel' },
    { id: 'header' }, { id: 'body' },
  ],
};
