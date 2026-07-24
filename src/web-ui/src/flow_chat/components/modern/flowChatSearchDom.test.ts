// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import {
  findElementWithDataValue,
  findFlowChatSearchTextRange,
  getFlowChatSearchTextRoot,
} from './flowChatSearchDom';

describe('FlowChat search DOM navigation', () => {
  it('finds a query split across Markdown text nodes', () => {
    const root = document.createElement('div');
    root.innerHTML = '<p>Before <span>key</span><strong>word</strong> after</p>';

    const range = findFlowChatSearchTextRange(root, 'KEYWORD');

    expect(range?.toString()).toBe('keyword');
  });

  it('maps a folded match back to original offsets when lowercasing expands a character', () => {
    const root = document.createElement('div');
    root.textContent = 'İstanbul';

    const range = findFlowChatSearchTextRange(root, 'stanbul');

    expect(range?.toString()).toBe('stanbul');
  });

  it('targets an exact flow item id without interpolating it into a selector', () => {
    const wrapper = document.createElement('div');
    wrapper.innerHTML = `
      <div data-flow-item-id="first">wrong</div>
      <div data-flow-item-id="item&quot;with-special">right needle</div>
    `;

    const target = findElementWithDataValue(
      wrapper,
      'data-flow-item-id',
      'item"with-special',
    );

    expect(target?.textContent).toBe('right needle');
    expect(getFlowChatSearchTextRoot(wrapper, 'item"with-special')).toBe(target);
  });

  it('ignores text hidden by a collapsed accessible container', () => {
    const root = document.createElement('div');
    root.innerHTML = '<div aria-hidden="true">hidden needle</div><div>visible needle</div>';

    const range = findFlowChatSearchTextRange(root, 'needle');

    expect(range?.startContainer.parentElement?.textContent).toBe('visible needle');
  });
});
