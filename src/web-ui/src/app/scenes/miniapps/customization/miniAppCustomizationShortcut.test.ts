import { describe, expect, it } from 'vitest';
import { shouldHandleCustomizeShortcut } from './useMiniAppCustomizeHotspot';

describe('shouldHandleCustomizeShortcut', () => {
  it('accepts ctrl shift e outside editable elements', () => {
    const target = { tagName: 'button', isContentEditable: false } as unknown as EventTarget;

    expect(
      shouldHandleCustomizeShortcut({
        key: 'E',
        ctrlKey: true,
        metaKey: false,
        shiftKey: true,
        target,
      }),
    ).toBe(true);
  });

  it('ignores shortcut events from inputs', () => {
    const target = { tagName: 'input', isContentEditable: false } as unknown as EventTarget;

    expect(
      shouldHandleCustomizeShortcut({
        key: 'e',
        ctrlKey: true,
        metaKey: false,
        shiftKey: true,
        target,
      }),
    ).toBe(false);
  });
});
