// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import {
  computeFlowChatSearchLineBox,
  resolveFlowChatSearchPresentation,
} from './flowChatSearchPresentation';
import type { SearchMatch } from './useFlowChatSearch';

function match(flowItemId: string, occurrenceIndex = 0): SearchMatch {
  return { type: 'model-round', turnId: 'turn-1', virtualItemIndex: 0, flowItemId, occurrenceIndex };
}

describe('FlowChat search text presentation', () => {
  it('highlights every matched source once while keeping the current occurrence distinct', () => {
    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div data-flow-item-id="a">needle one, <em>nee</em>dle two</div>
      <div data-flow-item-id="b">needle three<button>needle control</button></div>
      <div data-flow-item-id="tool">needle in an unrelated tool label</div>
    `;
    const matches = [match('a'), match('a', 1), match('b')];
    const result = resolveFlowChatSearchPresentation(wrapper, 'needle', matches, matches[1]);

    expect(result.currentRange?.toString()).toBe('needle');
    expect(result.currentRange?.startContainer.parentElement?.tagName).toBe('EM');
    expect(result.currentRoot?.dataset.flowItemId).toBe('a');
    expect(result.otherRanges.map(range => range.toString())).toEqual(['needle', 'needle']);
    expect(result.otherRanges[1].startContainer.parentElement?.dataset.flowItemId).toBe('b');
  });

  it('keeps non-current rows highlighted without creating a current line', () => {
    const wrapper = document.createElement('div');
    wrapper.innerHTML = '<div data-flow-item-id="a">D and d</div>';
    const result = resolveFlowChatSearchPresentation(wrapper, 'D', [match('a')]);

    expect(result.currentRange).toBeNull();
    expect(result.currentRoot).toBeNull();
    expect(result.otherRanges.map(range => range.toString())).toEqual(['D', 'd', 'd']);
  });

  it('never replaces an unmounted or collapsed text source with matching tool labels', () => {
    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div data-flow-item-id="hidden" hidden>needle</div>
      <div data-tool-card-id="thinking">needle in collapsed header</div>
      <div>needle in sibling</div>
    `;
    const matches = [match('missing'), match('hidden'), match('thinking')];
    const result = resolveFlowChatSearchPresentation(wrapper, 'needle', matches, matches[0]);

    expect(result).toEqual({ currentRange: null, currentRoot: null, otherRanges: [] });
  });
});

describe('FlowChat current search line geometry', () => {
  const wrapper = new DOMRect(100, 200, 900, 600);
  const source = new DOMRect(148, 230, 804, 480);

  it('makes a single letter easy to locate with exactly one text-height band', () => {
    const result = computeFlowChatSearchLineBox(wrapper, source, [new DOMRect(380, 300, 8, 18)], 26);
    expect(result).toEqual({ left: 48, top: 96, width: 804, height: 26 });
  });

  it('uses the first painted line when a query wraps instead of spanning the paragraph', () => {
    const fragments = [new DOMRect(), new DOMRect(700, 300, 200, 18), new DOMRect(148, 326, 160, 18)];
    expect(computeFlowChatSearchLineBox(wrapper, source, fragments, 26)?.height).toBe(26);
  });

  it('stays attached to the text while the outer viewport scrolls', () => {
    const rect = new DOMRect(380, 300, 8, 18);
    const shift = (box: DOMRect) => new DOMRect(box.x, box.y - 150, box.width, box.height);
    expect(computeFlowChatSearchLineBox(shift(wrapper), shift(source), [shift(rect)], 26))
      .toEqual(computeFlowChatSearchLineBox(wrapper, source, [rect], 26));
  });

  it('hides clipped, collapsed, and offscreen-in-source hits without inventing a marker', () => {
    expect(computeFlowChatSearchLineBox(wrapper, source, [], 26)).toBeNull();
    expect(computeFlowChatSearchLineBox(wrapper, source, [new DOMRect(160, 730, 8, 18)], 26)).toBeNull();
    expect(computeFlowChatSearchLineBox(wrapper, source, [new DOMRect(1000, 300, 8, 18)], 26)).toBeNull();
  });

  it('clips a line band to the visible source without changing the row geometry', () => {
    const clipped = new DOMRect(148, 300, 804, 12);
    expect(computeFlowChatSearchLineBox(wrapper, clipped, [new DOMRect(380, 296, 8, 18)], 26))
      .toEqual({ left: 48, top: 100, width: 804, height: 12 });
  });
});
