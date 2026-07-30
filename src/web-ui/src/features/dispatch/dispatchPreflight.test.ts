import { describe, expect, it } from 'vitest';
import { shouldConfirmDispatchAutoApproval } from './dispatchPreflight';

describe('dispatch preflight', () => {
  it('confirms auto approval only for submit and ambiguous submit retry', () => {
    expect(shouldConfirmDispatchAutoApproval('auto', 'submitting')).toBe(true);
    expect(shouldConfirmDispatchAutoApproval('auto', 'submission_unknown')).toBe(true);
    expect(shouldConfirmDispatchAutoApproval('auto', 'queued')).toBe(false);
    expect(shouldConfirmDispatchAutoApproval('auto', 'running')).toBe(false);
    expect(shouldConfirmDispatchAutoApproval('remote', 'submitting')).toBe(false);
  });
});
