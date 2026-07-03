import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { CanvasToolCard } from './CanvasToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      sessions: new Map(),
    }),
  },
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  createTab: vi.fn(),
}));

vi.mock('../../component-library', () => ({
  ToolProcessingDots: () => <span data-testid="tool-processing-dots" />,
}));

function canvasToolItem(toolName: string): FlowToolItem {
  return {
    id: `tool-${toolName}`,
    type: 'tool',
    toolName,
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: `call-${toolName}`,
      input: {
        title: 'Architecture Map',
      },
    },
    toolResult: {
      success: true,
      result: {
        action: toolName,
        artifactReference: 'bitfun-canvas://session/test/canvas/canvas_123',
        compiled: true,
        canvas: {
          status: 'compiled',
          artifact: {
            title: 'Architecture Map',
            status: 'compiled',
          },
        },
      },
    },
  };
}

describe('CanvasToolCard', () => {
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
  });

  it('uses the specific Canvas tool display name in the header', () => {
    act(() => {
      root.render(
        <CanvasToolCard
          toolItem={canvasToolItem('PatchCanvas')}
          config={{} as ToolCardConfig}
        />
      );
    });

    expect(container.textContent).toContain('Patch Canvas');
    expect(container.textContent).not.toContain('Create Canvas');
    expect(container.textContent).toContain('Architecture Map');
  });
});
