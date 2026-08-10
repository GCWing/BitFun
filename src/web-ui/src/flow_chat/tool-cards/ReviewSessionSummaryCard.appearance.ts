import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const reviewSessionSummaryCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'review-session-summary-card',
  parts: [
    { id: 'fileCount' }, { id: 'details' }, { id: 'summary' },
    { id: 'files' }, { id: 'open' },
  ],
};
