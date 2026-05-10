import { describe, expect, it } from 'vitest';
import {
  buildCodeReviewReliabilityNotices,
  reliabilityNoticeMarkdownLine,
} from './reliabilityNotices';
import { DEFAULT_CODE_REVIEW_MARKDOWN_LABELS } from './codeReviewReport';

describe('reliabilityNotices', () => {
  it('normalizes structured notices and keeps markdown label fallback stable', () => {
    const notices = buildCodeReviewReliabilityNotices({
      review_mode: 'deep',
      reliability_signals: [
        {
          kind: 'retry_guidance',
          severity: 'warning',
          source: 'runtime',
          detail: 'Retry one reduced reviewer packet.',
        },
      ],
    });

    expect(notices).toEqual([
      {
        kind: 'retry_guidance',
        severity: 'warning',
        source: 'runtime',
        detail: 'Retry one reduced reviewer packet.',
      },
    ]);
    expect(reliabilityNoticeMarkdownLine(
      notices[0],
      DEFAULT_CODE_REVIEW_MARKDOWN_LABELS,
    )).toBe(
      '- Retry guidance emitted [warning/runtime]: Retry one reduced reviewer packet.',
    );
  });
});
