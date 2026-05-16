import type { MouseEvent as ReactMouseEvent } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { JSDOM } from 'jsdom';
import { shouldIgnoreCardToggleClick } from './textSelection';

function createClickEvent(target: Element, currentTarget: Element = target): ReactMouseEvent<Element> {
  return {
    button: 0,
    currentTarget,
    defaultPrevented: false,
    target,
  } as unknown as ReactMouseEvent<Element>;
}

describe('shouldIgnoreCardToggleClick', () => {
  let dom: JSDOM;
  let root: HTMLDivElement;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"><span id="text">selectable text</span><button id="button">Action</button></div></body></html>');
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('Element', dom.window.Element);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);

    root = dom.window.document.getElementById('root') as HTMLDivElement;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('allows a normal left click with no active text selection', () => {
    expect(shouldIgnoreCardToggleClick(createClickEvent(root), root)).toBe(false);
  });

  it('ignores clicks when selected text belongs to the card', () => {
    const text = dom.window.document.getElementById('text')!;
    const range = dom.window.document.createRange();
    range.selectNodeContents(text);
    dom.window.getSelection()?.addRange(range);

    expect(shouldIgnoreCardToggleClick(createClickEvent(text, root), root)).toBe(true);
  });

  it('ignores interactive child clicks', () => {
    const button = dom.window.document.getElementById('button')!;

    expect(shouldIgnoreCardToggleClick(createClickEvent(button, root), root)).toBe(true);
  });
});
