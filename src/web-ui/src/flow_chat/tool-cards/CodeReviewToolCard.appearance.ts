import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const codeReviewToolCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'code-review-tool-card',
  parts: [
    { id: 'root' },
    { id: 'details' },
    { id: 'reliability' },
    { id: 'reliabilityItem' },
    { id: 'summary' },
    { id: 'issues' },
    { id: 'issue' },
    { id: 'remediation' },
    { id: 'groups' },
    { id: 'group' },
    { id: 'decision' },
    { id: 'remediationList' },
    { id: 'remediationItem' },
    { id: 'positive' },
  ],
  states: [
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
