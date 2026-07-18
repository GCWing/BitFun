import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { GetToolSpecCard } from './GetToolSpecCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

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

const config: ToolCardConfig = {
  toolName: 'GetToolSpec',
  displayName: 'Read Tool Spec',
  icon: 'SPEC',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
  description: 'Read usage instructions and schema for a deferred tool',
  displayMode: 'compact',
};

function buildDetailItem(): FlowToolItem {
  return {
    id: 'tool-spec-1',
    type: 'tool',
    toolName: 'GetToolSpec',
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'call-spec-1',
      input: {
        tool_name: 'Git',
      },
    },
    toolResult: {
      success: true,
      result: {
        tool_name: 'Git',
        description: 'Inspect and operate on the Git repository.',
        input_schema: {
          type: 'object',
          properties: {
            command: {
              type: 'string',
            },
          },
        },
      },
    },
  };
}

function buildAlreadyLoadedItem(): FlowToolItem {
  return {
    id: 'tool-spec-2',
    type: 'tool',
    toolName: 'GetToolSpec',
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'call-spec-2',
      input: {
        tool_name: 'WebFetch',
      },
    },
    toolResult: {
      success: true,
      result: {
        tool_name: 'WebFetch',
        already_loaded: true,
      },
    },
  };
}

describe('GetToolSpecCard', () => {
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
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
    dom.window.close();
  });

  it('renders tool spec detail and expands description plus schema', () => {
    act(() => {
      root.render(
        <GetToolSpecCard
          toolItem={buildDetailItem()}
          config={config}
        />,
      );
    });

    expect(container.textContent).toContain('Tool Spec');
    expect(container.textContent).toContain('Loaded spec for Git');
    expect(container.textContent).not.toContain('Inspect and operate on the Git repository.');

    const card = container.querySelector('.compact-tool-card');
    expect(card).not.toBeNull();

    act(() => {
      card?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Description');
    expect(container.textContent).toContain('Inspect and operate on the Git repository.');
    expect(container.textContent).toContain('"command"');
  });

  it('shows already-loaded summary without expanded detail affordance', () => {
    act(() => {
      root.render(
        <GetToolSpecCard
          toolItem={buildAlreadyLoadedItem()}
          config={config}
        />,
      );
    });

    expect(container.textContent).toContain('WebFetch is already loaded');
    const card = container.querySelector('.compact-tool-card');
    expect(card?.className).not.toContain('clickable');
  });
});
