import { describe, expect, it } from 'vitest';
import {
  virtualWindowPaddingPx,
  visibleRowRange,
  type FlowChatVirtualRow,
} from './useFlowChatVirtualizer';

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

describe('visibleRowRange', () => {
  /** 21 items of 100px, all rendered — the transcript is barely one screen. */
  const wholeTranscript = Array.from({ length: 21 }, (_, index) => (
    row(index, CONTENT_START + index * 100, 100)
  ));

  it('answers where the reader is, not what is rendered', () => {
    // The defect this exists for: every row is rendered wherever the viewport
    // stands, so the rendered window reports index 0 as present forever.
    expect(visibleRowRange(wholeTranscript, 1200, 500)).toEqual({
      startIndex: 11,
      endIndex: 16,
    });
  });

  it('reports the head only when the first item is on screen', () => {
    expect(visibleRowRange(wholeTranscript, 0, 500)).toEqual({
      startIndex: 0,
      endIndex: 4,
    });
  });

  it('counts an item the viewport edge merely clips', () => {
    // Half of item 4 and a sliver of item 9 are on screen, and both are things
    // the reader can see.
    expect(visibleRowRange(wholeTranscript, CONTENT_START + 450, 500)).toEqual({
      startIndex: 4,
      endIndex: 9,
    });
  });

  it('excludes an item that ends exactly on the viewport top', () => {
    expect(visibleRowRange(wholeTranscript, CONTENT_START + 500, 500)).toEqual({
      startIndex: 5,
      endIndex: 9,
    });
  });

  it('reports no boundary from inside the reserved tail blank', () => {
    // Past every item. The reader is at neither end of the loaded transcript,
    // and answering "the last one" would page on a viewport full of nothing.
    expect(visibleRowRange(wholeTranscript, 5000, 500)).toBeNull();
  });

  it('reports nothing when there is nothing rendered', () => {
    expect(visibleRowRange([], 0, 500)).toBeNull();
  });
});
