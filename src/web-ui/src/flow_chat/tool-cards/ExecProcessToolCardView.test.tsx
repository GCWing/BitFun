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

vi.mock('@/tools/terminal/components/LazyTerminalOutputRenderer', () => ({
  LazyTerminalOutputRenderer: React.forwardRef<
    { getVisibleText: () => string },
    { content: string; className?: string; maxRows?: number }
  >(({ content, className, maxRows }, ref) => {
    React.useImperativeHandle(ref, () => ({ getVisibleText: () => content }), [content]);
    return <pre className={className} data-max-rows={maxRows}>{content}</pre>;
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
    vi.useRealTimers();
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

    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="ambient"]')).toBeNull();
    expect(container.textContent).toContain('Waiting for confirmation');
    expect(container.textContent).not.toContain('Receiving parameters...');
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="outputFrame"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="footer"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="output"] pre')).toBeNull();
  });

  it('retains a just-completed tail result during the grace period', () => {
    const resultModel: ExecProcessCardModel = {
      ...model,
      resultOutput: 'All tests passed',
    };

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('running')}
          model={resultModel}
          isLastItem
        />,
      );
    });

    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="ambient"]')).toBeNull();

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={resultModel}
          isLastItem
        />,
      );
    });

    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="ambient"]')).toBeNull();
    expect(container.textContent).toContain('All tests passed');
    expect(container.querySelector('[data-bf-part="output"] pre')?.getAttribute('data-max-rows')).toBe('4');

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={resultModel}
          isLastItem={false}
        />,
      );
    });

    // Collapsed cards keep the prominent framework shell and animate height closed.
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"][data-bf-state~="expanded"]')).toBeNull();
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="ambient"]')).toBeNull();
    expect(container.querySelector('[data-bf-part="output"] pre')?.getAttribute('data-max-rows')).toBe('4');
  });

  it('uses the expanded output preview after a completed card is manually expanded', () => {
    const resultModel: ExecProcessCardModel = {
      ...model,
      resultOutput: 'All tests passed',
    };

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={resultModel}
        />,
      );
    });

    act(() => {
      container
        .querySelector<HTMLElement>('[data-bf-part="surface"][data-bf-attention="prominent"]')
        ?.click();
    });

    expect(container.querySelector('[data-bf-part="output"] pre')?.getAttribute('data-max-rows')).toBe('15');
    expect(container.querySelector('[data-bf-part="outputFrame"]')?.getAttribute('data-density')).toBe('expanded');
    expect(container.querySelector('[data-bf-part="outputFrame"]')?.getAttribute('data-sizing')).toBe('content');
  });

  it('content-sizes a manually expanded completed card with no output', () => {
    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={model}
        />,
      );
    });

    act(() => {
      container
        .querySelector<HTMLElement>('[data-bf-part="surface"][data-bf-attention="prominent"]')
        ?.click();
    });

    expect(container.textContent).toContain('No output');
    expect(container.querySelector('[data-bf-part="outputFrame"]')?.getAttribute('data-density')).toBe('expanded');
    expect(container.querySelector('[data-bf-part="outputFrame"]')?.getAttribute('data-sizing')).toBe('content');
  });

  it('pushes WriteStdin session and execution metadata to the footer end', () => {
    const stdinModel: ExecProcessCardModel = {
      ...model,
      kind: 'stdin',
      sessionId: 42,
      exitCode: 0,
      wallTimeSeconds: 1.25,
    };

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={{ ...toolItem('completed'), toolName: 'WriteStdin' }}
          model={stdinModel}
        />,
      );
    });

    act(() => {
      container
        .querySelector<HTMLElement>('[data-bf-part="surface"][data-bf-attention="prominent"]')
        ?.click();
    });

    const footerItems = Array.from(container.querySelectorAll('[data-bf-part="footer"] > span'));
    expect(footerItems).toHaveLength(3);
    expect(footerItems[0]?.getAttribute('data-push-to-end')).toBe('true');
    expect(footerItems[0]?.textContent).toContain('#42');
    expect(footerItems[1]?.getAttribute('data-push-to-end')).toBe('false');
    expect(footerItems[1]?.textContent).toContain('toolCards.execProcess.wallTime');
    expect(footerItems[2]?.getAttribute('data-push-to-end')).toBe('false');
    expect(footerItems[2]?.textContent).toContain('Exit code: 0');
  });

  it('keeps the output frame and footer mounted while content changes', () => {
    const streamingItem = {
      ...toolItem('running'),
      _progressLogs: ['line 1\nline 2\nline 3\nline 4'],
    } as FlowToolItem;
    const completedModel: ExecProcessCardModel = {
      ...model,
      resultOutput: 'line 1\nline 2\nline 3\nline 4',
      workdir: 'E:/workspace',
      exitCode: 0,
      wallTimeSeconds: 1.25,
    };

    act(() => {
      root.render(<ExecProcessToolCardView toolItem={toolItem('running')} model={model} />);
    });
    const frameBeforeOutput = container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="outputFrame"]');
    const footerBeforeOutput = container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="footer"]');
    expect(frameBeforeOutput?.getAttribute('data-density')).toBe('compact');
    expect(frameBeforeOutput?.getAttribute('data-sizing')).toBe('fixed');
    expect(footerBeforeOutput?.textContent).toBe('');
    expect(container.querySelector('[data-bf-part="output"] pre')).toBeNull();

    act(() => {
      root.render(<ExecProcessToolCardView toolItem={streamingItem} model={model} />);
    });
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="outputFrame"]')).toBe(frameBeforeOutput);
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="footer"]')).toBe(footerBeforeOutput);
    expect(container.querySelector('[data-bf-part="output"] pre')).not.toBeNull();

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={completedModel}
          isLastItem
        />,
      );
    });
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="outputFrame"]')).toBe(frameBeforeOutput);
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="footer"]')).toBe(footerBeforeOutput);
    expect(footerBeforeOutput?.textContent).toContain('E:/workspace');
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="outputFrame"]')?.getAttribute('data-density')).toBe('compact');
  });

  it('collapses a completed tail result when the grace period expires', () => {
    vi.useFakeTimers();
    const resultModel: ExecProcessCardModel = {
      ...model,
      resultOutput: 'All tests passed',
    };

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('running')}
          model={resultModel}
          isLastItem
        />,
      );
    });

    act(() => {
      root.render(
        <ExecProcessToolCardView
          toolItem={toolItem('completed')}
          model={resultModel}
          isLastItem
        />,
      );
    });
    expect(vi.getTimerCount()).toBeGreaterThan(0);

    act(() => {
      vi.advanceTimersByTime(799);
    });
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"][data-bf-state~="expanded"]')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(container.querySelector('[data-bf-part="surface"][data-bf-attention="prominent"][data-bf-state~="expanded"]')).toBeNull();
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="details"]')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(299);
    });
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="details"]')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(container.querySelector('[data-bf-component="command-tool-card"] [data-bf-part="details"]')).toBeNull();
  });
});
