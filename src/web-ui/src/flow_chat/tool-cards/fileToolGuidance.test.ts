import { describe, expect, it } from 'vitest';

import {
  FILE_TOOL_GUIDANCE_PREFIX,
  displayFileToolGuidanceMessage,
  isFileToolGuidanceMessage,
} from './fileToolGuidance';

describe('fileToolGuidance', () => {
  it('detects guidance-prefixed messages', () => {
    const message = `${FILE_TOOL_GUIDANCE_PREFIX}Use Read first.`;
    expect(isFileToolGuidanceMessage(message)).toBe(true);
    expect(displayFileToolGuidanceMessage(message)).toBe('Use Read first.');
  });

  it('leaves non-guidance messages unchanged', () => {
    expect(isFileToolGuidanceMessage('Permission denied')).toBe(false);
    expect(displayFileToolGuidanceMessage('Permission denied')).toBe('Permission denied');
  });
});
