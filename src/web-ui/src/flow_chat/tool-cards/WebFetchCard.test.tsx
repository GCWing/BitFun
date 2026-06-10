import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { WebFetchCard } from './WebFetchCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const openExternalMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return {
    useTranslation: () => ({
      t: createTestI18nT('flow-chat'),
    }),
  };
});

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../infrastructure/api', () => ({
  systemAPI: {
    openExternal: openExternalMock,
  },
}));

const config: ToolCardConfig = {
  toolName: 'WebFetch',
  displayName: 'Read Webpage',
  icon: 'WF',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
  description: 'Fetch webpage content',
  displayMode: 'standard',
};

function buildCompletedToolItem(): FlowToolItem {
  return {
    id: 'tool-webfetch-1',
    type: 'tool',
    toolName: 'WebFetch',
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'call-webfetch-1',
      input: {
        url: 'https://example.com/article',
        format: 'text',
      },
    },
    toolResult: {
      success: true,
      result: {
        url: 'https://example.com/article',
        format: 'text',
        content: 'Fetched body content',
        content_length: 20,
      },
    },
  };
}

describe('WebFetchCard', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);
    vi.stubGlobal('ResizeObserver', class {
      observe = vi.fn();
      disconnect = vi.fn();
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
    openExternalMock.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
    dom.window.close();
  });

  it('renders a compact fetch summary and expands fetched content', () => {
    act(() => {
      root.render(
        <WebFetchCard
          toolItem={buildCompletedToolItem()}
          config={config}
        />,
      );
    });

    expect(container.textContent).toContain('Read Webpage: "https://example.com/article"');
    expect(container.textContent).toContain('20 chars');
    expect(container.textContent).not.toContain('Fetched body content');

    const card = container.querySelector('.compact-tool-card');
    expect(card).not.toBeNull();

    act(() => {
      card?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Fetched body content');
  });

  it('opens the fetched URL when the expanded link row is clicked', () => {
    act(() => {
      root.render(
        <WebFetchCard
          toolItem={buildCompletedToolItem()}
          config={config}
        />,
      );
    });

    const card = container.querySelector('.compact-tool-card');
    act(() => {
      card?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    const linkRow = container.querySelector('.compact-expanded-result-title');
    expect(linkRow).not.toBeNull();

    act(() => {
      linkRow?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(openExternalMock).toHaveBeenCalledWith('https://example.com/article');
  });
});
