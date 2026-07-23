import { describe, expect, it } from 'vitest';
import {
  buildCodeReviewReliabilityNotices,
  reliabilityNoticeMarkdownLine,
} from './reliabilityNotices';
import { DEFAULT_CODE_REVIEW_MARKDOWN_LABELS } from './codeReviewReport';

describe('reliabilityNotices', () => {
  it('turns a non-complete evidence status into a localized notice without runtime prose', () => {
    const notices = buildCodeReviewReliabilityNotices({
      evidence_status: 'limited',
      reliability_signals: [{
        kind: 'target_evidence_limited',
        severity: 'warning',
        source: 'runtime',
        detail: 'Backend-only English diagnostic.',
      }],
    });

    expect(notices).toEqual([{
      kind: 'target_evidence_limited',
      severity: 'warning',
      source: 'runtime',
    }]);
  });

  it('normalizes structured notices and keeps markdown label fallback stable', () => {
    const notices = buildCodeReviewReliabilityNotices({
      review_mode: 'deep',
      reliability_signals: [
        {
          kind: 'retry_guidance',
          severity: 'warning',
          source: 'runtime',
          detail: 'Retry one optional check outside this run.',
        },
      ],
    });

    expect(notices).toEqual([
      {
        kind: 'retry_guidance',
        severity: 'warning',
        source: 'runtime',
        detail: 'Retry one optional check outside this run.',
      },
    ]);
    expect(reliabilityNoticeMarkdownLine(
      notices[0],
      DEFAULT_CODE_REVIEW_MARKDOWN_LABELS,
    )).toBe(
      '- Retry guidance emitted [warning/runtime]: Retry one optional check outside this run.',
    );
  });
});
