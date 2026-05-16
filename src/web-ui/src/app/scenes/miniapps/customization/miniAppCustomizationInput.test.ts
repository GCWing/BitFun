import { describe, expect, it } from 'vitest';
import { shouldSubmitMiniAppCustomizationRequest } from './miniAppCustomizationInput';

describe('shouldSubmitMiniAppCustomizationRequest', () => {
  it('submits on Enter without Shift', () => {
    expect(shouldSubmitMiniAppCustomizationRequest({
      key: 'Enter',
      shiftKey: false,
      isComposing: false,
    })).toBe(true);
  });

  it('keeps Shift+Enter as a newline', () => {
    expect(shouldSubmitMiniAppCustomizationRequest({
      key: 'Enter',
      shiftKey: true,
      isComposing: false,
    })).toBe(false);
  });

  it('does not submit while IME composition is active', () => {
    expect(shouldSubmitMiniAppCustomizationRequest({
      key: 'Enter',
      shiftKey: false,
      isComposing: true,
    })).toBe(false);
  });
});
