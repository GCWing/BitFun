import { describe, expect, it } from 'vitest';
import {
  applyCurrentClientVersionDefault,
  createEmptyMarketSubmissionDraft,
} from './miniAppSubmissionDraft';

describe('MiniApp market submission defaults', () => {
  it('uses the current client version for a new draft', () => {
    const draft = applyCurrentClientVersionDefault(
      createEmptyMarketSubmissionDraft(),
      '1.0.0',
    );

    expect(draft.minOpenBitFunVersion).toBe('1.0.0');
  });

  it('preserves a minimum version chosen by the user', () => {
    const draft = {
      ...createEmptyMarketSubmissionDraft(),
      minOpenBitFunVersion: '1.1.0',
    };

    expect(applyCurrentClientVersionDefault(draft, '1.2.0')).toBe(draft);
  });
});
