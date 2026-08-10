import { describe, expect, it } from 'vitest';
import { virtualWindowPaddingPx, type FlowChatVirtualRow } from './useFlowChatVirtualizer';

/** Header above the items, which every offset below is measured past. */
const CONTENT_START = 24;

function row(index: number, startPx: number, sizePx: number): FlowChatVirtualRow {
  return { index, key: `item-${index}`, startPx, endPx: startPx + sizePx };
}

describe('virtualWindowPaddingPx', () => {
  it('stands in for the transcript on both sides of the window', () => {
    // 40 items of 100px: the window holds 10..14, so 10 are above and 25 below.
    const rows = [10, 11, 12, 13, 14].map(index => row(index, CONTENT_START + index * 100, 100));
    expect(virtualWindowPaddingPx(rows, 4000, CONTENT_START)).toEqual({
      paddingTopPx: 1000,
      paddingBottomPx: 2500,
    });
  });

  it('reserves nothing above a window that starts at the first item', () => {
    const rows = [row(0, CONTENT_START, 100), row(1, CONTENT_START + 100, 100)];
    expect(virtualWindowPaddingPx(rows, 4000, CONTENT_START)).toEqual({
      paddingTopPx: 0,
      paddingBottomPx: 3800,
    });
  });

  it('reserves nothing below a window that ends at the last item', () => {
    const rows = [row(38, CONTENT_START + 3800, 100), row(39, CONTENT_START + 3900, 100)];
    expect(virtualWindowPaddingPx(rows, 4000, CONTENT_START)).toEqual({
      paddingTopPx: 3800,
      paddingBottomPx: 0,
    });
  });

  it('reserves nothing at all for an empty window', () => {
    // Nothing to be outside of. Deriving padding from a row that is not there
    // would reserve the whole transcript on one side and nothing on the other.
    expect(virtualWindowPaddingPx([], 4000, CONTENT_START)).toEqual({
      paddingTopPx: 0,
      paddingBottomPx: 0,
    });
  });

  it('never reserves negative space while the header height is settling', () => {
    // The header is measured, so for a frame its height can exceed the offsets
    // computed against the previous one.
    const rows = [row(0, 8, 100)];
    expect(virtualWindowPaddingPx(rows, 4000, CONTENT_START)).toEqual({
      paddingTopPx: 0,
      paddingBottomPx: 3916,
    });
  });
});
