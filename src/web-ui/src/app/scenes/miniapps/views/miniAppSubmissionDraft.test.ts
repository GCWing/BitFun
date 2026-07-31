import { describe, expect, it } from 'vitest';
import {
  applyCurrentClientVersionDefault,
  createEmptyMarketSubmissionDraft,
} from './miniAppSubmissionDraft';

describe('MiniApp market submission defaults', () => {
  it('uses the current client version for a new draft', () => {
    const draft = applyCurrentClientVersionDefault(
      createEmptyMarketSubmissionDraft(),
      '0.2.15',
    );

    expect(draft.minBitfunVersion).toBe('0.2.15');
  });

  it('preserves a minimum version chosen by the user', () => {
    const draft = {
      ...createEmptyMarketSubmissionDraft(),
      minBitfunVersion: '0.2.10',
    };

    expect(applyCurrentClientVersionDefault(draft, '0.2.15')).toBe(draft);
  });
});
