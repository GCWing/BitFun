import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const basicsConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'basics-config',
  parts: [
    { id: 'root' }, { id: 'content' }, { id: 'launchAtLogin' }, { id: 'autoUpdate' },
    { id: 'logging' }, { id: 'logPath' }, { id: 'terminal' }, { id: 'shellOption' },
    { id: 'windowBehavior' }, { id: 'notifications' },
  ],
};
