import { describe, expect, it } from 'vitest';
import { latestReasoningSummaryPreview } from './reasoningSummaryPresentation';

describe('latestReasoningSummaryPreview', () => {
  it('shows only the latest summary part without Markdown markers', () => {
    expect(latestReasoningSummaryPreview(
      '**Inspecting the Responses stream**\n\n**Preparing the focused repair**',
    )).toBe('Preparing the focused repair');
  });

  it('collapses multiline Markdown in the latest part to one line', () => {
    expect(latestReasoningSummaryPreview(
      '**Earlier**\n\n### Latest\n- first detail\n- second detail',
    )).toBe('Latest first detail second detail');
  });
});
