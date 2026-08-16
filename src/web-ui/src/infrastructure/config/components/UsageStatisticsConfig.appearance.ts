import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const usageStatisticsConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'usage-statistics-config',
  parts: [
    { id: 'root' },
    { id: 'filters' },
    { id: 'summary' },
    { id: 'distributions' },
    { id: 'donut' },
    { id: 'trend' },
    { id: 'trendPanel' },
    { id: 'empty' },
  ],
};
