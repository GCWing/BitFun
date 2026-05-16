import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ReadFileDisplay } from './ReadFileDisplay';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    }),
  };
});

vi.mock('../../component-library', () => ({
  ToolProcessingDots: () => <span data-testid="tool-processing-dots" />,
  IconButton: ({
    children,
    tooltip,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { tooltip?: React.ReactNode }) => (
    <button
      type="button"
      aria-label={typeof tooltip === 'string' ? tooltip : undefined}
      {...props}
    >
      {children}
    </button>
  ),
}));

vi.mock('./ToolCardHeaderActions', () => ({
  ToolCardHeaderActions: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
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

  it('renders ACP permission actions for pending read confirmation', () => {
    const onConfirm = vi.fn();
    const onReject = vi.fn();

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
          onConfirm={onConfirm}
          onReject={onReject}
        />
      );
    });

    expect(container.textContent).toContain('Requesting read permission:');
    expect(container.textContent).toContain('/');

    const actionButtons = container.querySelectorAll('button');
    expect(actionButtons).toHaveLength(2);

    act(() => {
      actionButtons[0]?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(onConfirm).toHaveBeenCalledWith(toolItem.toolCall.input, 'once', true);

    act(() => {
      actionButtons[1]?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(onReject).toHaveBeenCalledWith('reject');
  });
});
