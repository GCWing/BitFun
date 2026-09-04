import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ContextCompressionDisplay } from './ContextCompressionDisplay';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return {
    initReactI18next: {
      type: '3rdParty',
      init: vi.fn(),
    },
    useTranslation: () => ({
      t: createTestI18nT('flow-chat'),
    }),
  };
});

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    formatNumber: (value: number, options?: Intl.NumberFormatOptions) =>
      new Intl.NumberFormat('en-US', options).format(value),
  },
}));

describe('ContextCompressionDisplay', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
    dom.window.close();
  });

  it('shows only the compressed length and reduction ratio in the result summary', () => {
    act(() => {
      root.render(
        <ContextCompressionDisplay
          compressionData={{
            session_id: 'session-1',
            compression_count: 3,
            has_summary: true,
            summary_source: 'model',
            tokens_before: 124_000,
            tokens_after: 31_000,
            compression_ratio: 0.25,
            trigger: 'manual',
          }}
        />,
      );
    });

    expect(container.querySelector('[data-openbitfun-part="action"]')?.textContent).toBe('Context compression:');
    expect(container.querySelector('[data-openbitfun-part="content"]')?.textContent).toBe(
      'Compressed context length 31,000 (compression ratio 75%)',
    );
    expect(container.querySelector('[data-openbitfun-part="tokenChange"]')).toBeNull();
    expect(container.querySelector('[data-openbitfun-part="savings"]')).toBeNull();
    expect(container.querySelector('[data-openbitfun-part="meta"]')).toBeNull();
    expect(container.textContent).not.toContain('124,000');
    expect(container.textContent).not.toContain('Compression #3');
  });
});
