import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const deepReviewConsentDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'deep-review-consent-dialog',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'heading' }, { id: 'close' },
    { id: 'capacityNote' }, { id: 'summary' }, { id: 'summaryHeader' },
    { id: 'summaryStats' }, { id: 'impactGrid' },
    { id: 'strategy' }, { id: 'reviewerGroup' }, { id: 'footer' }, { id: 'actions' },
  ],
};
