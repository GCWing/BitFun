import { describe, expect, it } from 'vitest';

import {
  FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES,
  FLOWCHAT_COLLAPSE_DURATION_MS,
  FLOWCHAT_COLLAPSE_INTENT_TTL_MS,
} from './flowChatCollapseMotion';

describe('flowChatCollapseMotion', () => {
  it('keeps intent TTL above the animated collapse window', () => {
    expect(FLOWCHAT_COLLAPSE_DURATION_MS).toBe(300);
    expect(FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES).toBeGreaterThan(0);
    expect(FLOWCHAT_COLLAPSE_INTENT_TTL_MS).toBeGreaterThan(FLOWCHAT_COLLAPSE_DURATION_MS);
  });
});
