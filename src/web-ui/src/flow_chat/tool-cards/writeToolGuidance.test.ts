import { describe, expect, it } from 'vitest';

import {
  WRITE_TOOL_GUIDANCE_PREFIX,
  displayWriteToolGuidanceMessage,
  isWriteToolGuidanceMessage,
} from './writeToolGuidance';

describe('writeToolGuidance', () => {
  it('detects guidance-prefixed messages', () => {
    const message = `${WRITE_TOOL_GUIDANCE_PREFIX}Use Read first.`;
    expect(isWriteToolGuidanceMessage(message)).toBe(true);
    expect(displayWriteToolGuidanceMessage(message)).toBe('Use Read first.');
  });

  it('leaves non-guidance messages unchanged', () => {
    expect(isWriteToolGuidanceMessage('Permission denied')).toBe(false);
    expect(displayWriteToolGuidanceMessage('Permission denied')).toBe('Permission denied');
  });
});
