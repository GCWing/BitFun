import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const agentsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'agents',
  parts: [
    { id: 'root' },
    { id: 'anchorBar' },
    { id: 'zones' },
    { id: 'harnessGrid' },
    { id: 'coreGrid' },
    { id: 'filters' },
    { id: 'detailSection' },
  ],
};
