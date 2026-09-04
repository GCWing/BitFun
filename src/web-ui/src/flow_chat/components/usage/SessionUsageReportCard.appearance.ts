import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const sessionUsageReportCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'session-usage-report-card',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'title' }, { id: 'actions' },
    { id: 'metrics' }, { id: 'metric' }, { id: 'lists' }, { id: 'list' },
    { id: 'listRow' }, { id: 'fileStat' }, { id: 'loading' }, { id: 'fallback' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'fallback', selector: { kind: 'self', suffix: '[data-openbitfun-state~="fallback"]' } },
  ],
};
