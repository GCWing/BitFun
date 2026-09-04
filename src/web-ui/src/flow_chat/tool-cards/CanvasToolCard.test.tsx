import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { CanvasToolCard } from './CanvasToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  openCanvasArtifactTab: vi.fn(),
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      sessions: new Map(),
    }),
  },
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  openCanvasArtifactTab: (...args: unknown[]) => mocks.openCanvasArtifactTab(...args),
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
        artifactReference: 'openbitfun-canvas://session/test/canvas/canvas_123',
        compiled: true,
        canvas: {
          status: 'compiled',
          artifact: {
            title: 'Architecture Map',
            status: 'compiled',
            sourceRevision: 'rev_1',
            lastKnownGoodRevision: 'rev_1',
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
    mocks.openCanvasArtifactTab.mockReset();
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

  it('opens the same Canvas artifact tab path used by markdown links', () => {
    act(() => {
      root.render(
        <CanvasToolCard
          toolItem={canvasToolItem('CreateCanvas')}
          sessionId="test"
          config={{} as ToolCardConfig}
        />
      );
    });

    const card = container.querySelector<HTMLElement>(
      '[data-openbitfun-component="flow-chat-tool-card"][data-openbitfun-part="surface"]',
    );
    act(() => card?.click());

    expect(mocks.openCanvasArtifactTab).toHaveBeenCalledWith(expect.objectContaining({
      artifactReference: 'openbitfun-canvas://session/test/canvas/canvas_123',
      title: 'Architecture Map',
      sourceMetadata: expect.objectContaining({
        type: 'tool-call',
        sessionId: 'test',
      }),
      metadata: expect.objectContaining({ fromTool: true }),
    }));
  });
});
