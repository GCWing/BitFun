import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ExecProcessToolCardView, type ExecProcessCardModel } from './ExecProcessToolCardView';
import type { FlowToolItem } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const messages: Record<string, string> = {
  'toolCards.terminal.cancelled': 'Cancelled',
  'toolCards.terminal.rejected': 'Rejected',
  'toolCards.terminal.receivingParams': 'Receiving parameters...',
  'toolCards.terminal.exitCode': 'Exit code: {{code}}',
  'toolCards.approval.waiting': 'Waiting for confirmation',
  'toolCards.execProcess.copyPrimary': 'Copy',
  'toolCards.execProcess.primaryCopied': 'Copied',
  'toolCards.execProcess.copyPrimaryFailed': 'Failed to copy',
};

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: Record<string, unknown>) => {
        const template = messages[key] ?? key;
        return template.replace(/{{(\w+)}}/g, (_, name) => String(options?.[name] ?? ''));
      },
    }),
  };
});

vi.mock('../../component-library', () => ({
  DotMatrixLoader: () => <span data-testid="dot-matrix-loader" />,
  ToolProcessingDots: () => <span data-testid="tool-processing-dots" />,
  IconButton: ({
    children,
    tooltip,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { tooltip?: React.ReactNode }) => (
    <button
      type="button"
      title={typeof tooltip === 'string' ? tooltip : undefined}
      {...props}
    >
      {children}
    </button>
  ),
}));

vi.mock('@/tools/terminal/components/LazyTerminalOutputRenderer', () => ({
  LazyTerminalOutputRenderer: React.forwardRef<
    { getVisibleText: () => string },
    { content: string; className?: string }
  >(({ content, className }, ref) => {
    React.useImperativeHandle(ref, () => ({ getVisibleText: () => content }), [content]);
    return <pre className={className}>{content}</pre>;
  }),
}));

const model: ExecProcessCardModel = {
  kind: 'command',
  actionLabel: 'Run command:',
  primaryText: 'npm test',
  emptyText: '[No command]',
  copyText: 'npm test',
  waitingText: 'Running command...',
  noOutputText: 'No output',
  resultOutput: '',
};

function toolItem(status: FlowToolItem['status'], isParamsStreaming = false): FlowToolItem {
  return {
    id: 'tool-exec-1',
    type: 'tool',
    toolName: 'ExecCommand',
    status,
    timestamp: Date.now(),
    isParamsStreaming,
    toolCall: {
      id: 'call-exec-1',
      input: { cmd: 'npm test' },
    },
  };
}

describe('ExecProcessToolCardView', () => {
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

  it('shows cancelled state instead of receiving params when a stale streaming flag remains', () => {
    act(() => {
      root.render(<ExecProcessToolCardView toolItem={toolItem('running', true)} model={model} />);
    });

    act(() => {
      root.render(<ExecProcessToolCardView toolItem={toolItem('cancelled', true)} model={model} />);
    });

    expect(container.textContent).toContain('Cancelled');
    expect(container.textContent).not.toContain('Receiving parameters...');
  });

  it('shows rejected state for user-rejected command confirmation', () => {
    act(() => {
      root.render(<ExecProcessToolCardView toolItem={toolItem('rejected', true)} model={model} />);
    });

    expect(container.textContent).toContain('Rejected');
    expect(container.textContent).not.toContain('Receiving parameters...');
  });

  it('keeps legacy cancelled rejection state labeled as rejected', () => {
    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={{
            ...toolItem('cancelled', true),
            userConfirmed: false,
          }}
          model={model}
        />,
      );
    });

    expect(container.textContent).toContain('Rejected');
    expect(container.textContent).not.toContain('Receiving parameters...');
  });

  it('shows waiting confirmation instead of receiving params while confirmation is pending', () => {
    act(() => {
      root.render(<ExecProcessToolCardView toolItem={toolItem('pending_confirmation', true)} model={model} />);
    });

    expect(container.querySelector('.base-tool-card')).not.toBeNull();
    expect(container.querySelector('.compact-tool-card')).toBeNull();
    expect(container.textContent).toContain('Waiting for confirmation');
    expect(container.textContent).not.toContain('Receiving parameters...');
  });
});
