// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { TerminalOutputFallback } from './LazyTerminalOutputRenderer';

describe('TerminalOutputFallback', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('reserves bounded row height while the xterm renderer chunk loads', () => {
    act(() => {
      root.render(
        <TerminalOutputFallback
          content={['one', 'two', 'three', 'four'].join('\n')}
          maxRows={2}
        />
      );
    });

    const fallback = container.querySelector<HTMLPreElement>('pre.terminal-output-pre');
    expect(fallback).not.toBeNull();
    expect(fallback?.textContent).toBe('three\nfour');
    expect(fallback?.style.height).toBe('34px');
    expect(fallback?.style.overflow).toBe('hidden');
  });
});
