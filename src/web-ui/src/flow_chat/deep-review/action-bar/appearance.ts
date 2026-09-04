import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const deepReviewActionBarAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'deep-review-action-bar',
  parts: [
    { id: 'root' },
    { id: 'progress' },
    { id: 'attribution' },
    { id: 'degradation' },
    { id: 'degradationOption' },
    { id: 'noIssues' },
    { id: 'fixDone' },
    { id: 'custom' },
    { id: 'customInput' },
  ],
  facets: [
    {
      id: 'phase',
      attribute: 'data-openbitfun-phase',
      values: [
        'review_running',
        'review_completed',
        'fix_running',
        'fix_completed',
        'fix_failed',
        'fix_timeout',
        'fix_interrupted',
        'review_waiting_capacity',
        'review_interrupted',
        'resume_blocked',
        'resume_running',
        'resume_failed',
        'review_error',
      ],
    },
    { id: 'variant', attribute: 'data-openbitfun-variant', values: ['success', 'warning', 'error', 'info', 'loading'] },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
