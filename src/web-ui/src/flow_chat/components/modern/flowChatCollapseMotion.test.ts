import { describe, expect, it } from 'vitest';

import {
  FLOWCHAT_COLLAPSE_DURATION_MS,
  FLOWCHAT_COLLAPSE_EASING,
} from './flowChatCollapseMotion';

describe('flowChatCollapseMotion', () => {
  it('keeps automatic and manual collapse motion aligned', () => {
    expect(FLOWCHAT_COLLAPSE_DURATION_MS).toBe(300);
    expect(FLOWCHAT_COLLAPSE_EASING).toContain('cubic-bezier');
  });
});
