// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AsyncPrismSyntaxHighlighter } from './AsyncPrismSyntaxHighlighter';

vi.mock('@/shared/utils/syntaxHighlighterLoader', () => ({
  getLoadedPrismSyntaxHighlighter: () => null,
  loadPrismSyntaxHighlighter: () => new Promise(() => {}),
}));

vi.mock('@/shared/utils/startupTaskScheduling', () => ({
  scheduleAfterStartupPaint: () => () => {},
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  isStartupRenderTraceEnabled: () => false,
  recordReactRenderProfile: vi.fn(),
  startupTrace: {},
}));

describe('AsyncPrismSyntaxHighlighter appearance contract', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('marks the real pre and code elements as skin-addressable parts', async () => {
    await act(async () => {
      root.render(
        <AsyncPrismSyntaxHighlighter language="ts" style={{}}>
          {'const value = 1;'}
        </AsyncPrismSyntaxHighlighter>,
      );
    });

    expect(container.querySelector('pre[data-bf-part="codePre"]')).not.toBeNull();
    expect(container.querySelector('code[data-bf-part="codeContent"]')).not.toBeNull();
  });
});
