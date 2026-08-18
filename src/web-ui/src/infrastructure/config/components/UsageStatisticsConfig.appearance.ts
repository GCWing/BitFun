import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const usageStatisticsConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'usage-statistics-config',
  parts: [
    { id: 'root' },
    { id: 'filters' },
    { id: 'summary' },
    { id: 'distributions' },
    { id: 'modelHitRate' },
    { id: 'trendPanel' },
    { id: 'empty' },
  ],
};
