import { describe, expect, it } from 'vitest';
import { submissionDisplayStatus } from './SubmissionsPage';
import type { AppearanceSubmission } from './types';

const approvedSubmission = {
  submissionId: 'submission-1',
  listingId: 'listing-1',
  slug: 'ocean-night',
  releaseNumber: 1,
  minOpenBitFunVersion: '1.0.0',
  requiredCapabilities: [],
  changelog: 'Initial release',
  license: { spdxExpression: 'MIT' },
  status: 'approved',
  createdAt: 1,
  updatedAt: 2,
} satisfies AppearanceSubmission;

describe('submissionDisplayStatus', () => {
  it('keeps a currently published approval as approved', () => {
    expect(submissionDisplayStatus({
      ...approvedSubmission,
      publicationStatus: 'published',
    })).toBe('approved');
  });

  it('surfaces release and listing moderation over the historical approval', () => {
    expect(submissionDisplayStatus({
      ...approvedSubmission,
      publicationStatus: 'yanked',
    })).toBe('yanked');
    expect(submissionDisplayStatus({
      ...approvedSubmission,
      publicationStatus: 'unpublished',
    })).toBe('unpublished');
  });
});
