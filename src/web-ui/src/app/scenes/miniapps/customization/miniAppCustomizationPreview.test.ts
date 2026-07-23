import { describe, expect, it } from 'vitest';
import { getNextMiniAppPreviewOpenState } from './miniAppCustomizationPreview';

describe('getNextMiniAppPreviewOpenState', () => {
  it('keeps preview closed until a preview copy exists', () => {
    expect(getNextMiniAppPreviewOpenState({ hasPreview: false, isOpen: false })).toBe(false);
  });

  it('toggles preview visibility when a preview copy exists', () => {
    expect(getNextMiniAppPreviewOpenState({ hasPreview: true, isOpen: false })).toBe(true);
    expect(getNextMiniAppPreviewOpenState({ hasPreview: true, isOpen: true })).toBe(false);
  });
});
