import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ViewImageToolCard } from './ViewImageToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const messages: Record<string, string> = {
  'toolCards.viewImage.loading': 'Loading image...',
  'toolCards.viewImage.loadFailed': 'Failed to load image',
  'toolCards.viewImage.runtimeUriUnsupported': 'Preview is not available for this runtime artifact path',
  'copyOutput.openInEditor': 'Open in editor',
};

const { readFileContent } = vi.hoisted(() => ({
  readFileContent: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  workspaceAPI: {
    readFileContent,
  },
}));

vi.mock('./DefaultToolCard', () => ({
  DefaultToolCard: () => <div data-testid="default-tool-card">default-card</div>,
}));

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => messages[key] ?? key,
    }),
  };
});

vi.mock('../../component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

const config: ToolCardConfig = {
  toolName: 'view_image',
  displayName: 'View Image',
  icon: 'IMG',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
  description: 'Attach an image file for model vision',
  displayMode: 'standard',
  primaryColor: 'var(--color-accent-600)',
};

function buildToolItem(overrides: Partial<FlowToolItem> = {}): FlowToolItem {
  return {
    id: 'tool-1',
    type: 'tool',
    toolName: 'view_image',
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'tool-1',
      input: { path: '/workspace/screenshots/pixel.png' },
    },
    toolResult: {
      success: true,
      result: {
        path: '/workspace/screenshots/pixel.png',
        mime_type: 'image/png',
        width: 1,
        height: 1,
        size: 128,
        summary: 'Attached image: /workspace/screenshots/pixel.png',
      },
    },
    ...overrides,
  };
}

describe('ViewImageToolCard', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    readFileContent.mockReset();
    readFileContent.mockResolvedValue('aGVsbG8=');

    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders the default tool card header and requests image bytes', () => {
    act(() => {
      root.render(
        <ViewImageToolCard
          toolItem={buildToolItem()}
          config={config}
        />,
      );
    });

    expect(container.querySelector('[data-testid="default-tool-card"]')).not.toBeNull();
    expect(readFileContent).toHaveBeenCalledWith('/workspace/screenshots/pixel.png');
    expect(container.querySelector('.view-image-tool-card__inline-preview-row')).not.toBeNull();
  });

  it('shows an open-in-editor overlay when a handler is provided', () => {
    const onOpenInEditor = vi.fn();

    act(() => {
      root.render(
        <ViewImageToolCard
          toolItem={buildToolItem()}
          config={config}
          onOpenInEditor={onOpenInEditor}
        />,
      );
    });

    const overlay = container.querySelector('.view-image-tool-card__open-overlay');
    expect(overlay).not.toBeNull();
    expect(overlay?.textContent).toContain('Open in editor');

    act(() => {
      overlay?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onOpenInEditor).toHaveBeenCalledWith('/workspace/screenshots/pixel.png');
  });

  it('shows a runtime-uri error without calling readFileContent', async () => {
    act(() => {
      root.render(
        <ViewImageToolCard
          toolItem={buildToolItem({
            toolResult: {
              success: true,
              result: {
                path: 'bitfun://runtime/workspace-1/sessions/session-1/tool-previews/tool-1.webp',
                mime_type: 'image/webp',
                width: 10,
                height: 10,
                size: 100,
                summary: 'Attached image',
              },
            },
          })}
          config={config}
        />,
      );
    });

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(readFileContent).not.toHaveBeenCalled();
    expect(container.textContent).toContain('Preview is not available for this runtime artifact path');
  });
});
