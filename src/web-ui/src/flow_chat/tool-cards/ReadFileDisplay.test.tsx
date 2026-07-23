import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ReadFileDisplay } from './ReadFileDisplay';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const messages: Record<string, string> = {
  'toolCards.readFile.permissionRequest': 'Requesting read permission:',
};

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: { defaultValue?: string }) => messages[key] ?? options?.defaultValue ?? key,
    }),
  };
});

vi.mock('../../component-library', () => ({
  ToolProcessingDots: () => <span data-testid="tool-processing-dots" />,
}));

describe('ReadFileDisplay', () => {
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

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders pending read confirmation copy without inline approval actions', () => {
    const toolItem: FlowToolItem = {
      id: 'tool-read-1',
      type: 'tool',
      toolName: 'Read',
      status: 'pending_confirmation',
      timestamp: Date.now(),
      requiresConfirmation: true,
      userConfirmed: false,
      toolCall: {
        id: 'call-read-1',
        input: {
          file_path: '/',
        },
      },
      acpPermission: {
        permissionId: 'perm-1',
        requestedAt: Date.now(),
        options: [
          {
            optionId: 'once',
            name: 'Allow once',
            kind: 'allow_once',
          },
          {
            optionId: 'reject',
            name: 'Reject',
            kind: 'reject_once',
          },
        ],
      },
    };

    const config: ToolCardConfig = {
      toolName: 'Read',
      displayName: 'Read File',
      icon: 'R',
      requiresConfirmation: false,
      resultDisplayType: 'summary',
      description: 'Read file contents',
      displayMode: 'compact',
    };

    act(() => {
      root.render(
        <ReadFileDisplay
          toolItem={toolItem}
          config={config}
        />
      );
    });

    expect(container.textContent).toContain('Requesting read permission:');
    expect(container.textContent).toContain('/');
    expect(container.querySelectorAll('button')).toHaveLength(0);
  });

  it('does not report a file size for session preview truncation markers', () => {
    const toolItem: FlowToolItem = {
      id: 'tool-read-2',
      type: 'tool',
      toolName: 'Read',
      status: 'completed',
      timestamp: Date.now(),
      toolCall: {
        id: 'call-read-2',
        input: {
          file_path: 'src/main.rs',
        },
      },
      toolResult: {
        id: 'result-read-2',
        result: {
          content: '[truncated for session view]',
        },
        timestamp: Date.now(),
      },
    };

    const config: ToolCardConfig = {
      toolName: 'Read',
      displayName: 'Read File',
      icon: 'R',
      requiresConfirmation: false,
      resultDisplayType: 'summary',
      description: 'Read file contents',
      displayMode: 'compact',
    };

    act(() => {
      root.render(
        <ReadFileDisplay
          toolItem={toolItem}
          config={config}
        />
      );
    });

    expect(container.textContent).toContain('main.rs');
    expect(container.textContent).not.toMatch(/\(\d+B\)/);
  });
});
