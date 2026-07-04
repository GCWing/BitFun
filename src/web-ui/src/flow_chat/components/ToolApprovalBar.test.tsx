import React from 'react';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ToolApprovalBar } from './ToolApprovalBar';
import type { FlowToolItem } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const messages: Record<string, string> = {
  'toolCards.approval.waiting': 'Waiting for confirmation',
  'toolCards.approval.confirm': 'Allow',
  'toolCards.approval.reject': 'Reject',
  'toolCards.approval.rejectWithInstruction': 'Reject with instruction',
  'toolCards.approval.enableAutoExecute': 'Enable tool auto execute',
  'toolCards.approval.confirmTooltip': 'Allow this tool run',
  'toolCards.approval.enableAutoExecuteTooltip': '开启工具自动执行',
  'toolCards.approval.autoExecuteEnabled': 'Tool auto execute enabled',
  'toolCards.approval.autoExecuteEnableFailed': 'Failed to enable tool auto execute',
  'toolCards.approval.rejectTooltip': 'Reject this tool run',
  'toolCards.approval.rejectWithInstructionTooltip': 'Reject and tell the assistant what to do next',
  'toolCards.approval.rejectInstructionLabel': 'Rejection instruction',
  'toolCards.approval.rejectInstructionPlaceholder': 'Tell the assistant what to do instead...',
  'toolCards.approval.rejectWithInstructionSubmit': 'Reject',
  'toolCards.approval.emptyInputTooltip': 'This tool has no executable input',
  'toolCards.approval.ariaLabel': 'Tool approval',
  'toolCards.approval.remaining': 'remaining {{time}}',
};

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: Record<string, unknown>) => {
        const template = messages[key] ?? (typeof options?.defaultValue === 'string' ? options.defaultValue : key);
        return template.replace(/\{\{(\w+)\}\}/g, (_match, token) => String(options?.[token] ?? `{{${token}}}`));
      },
    }),
  };
});

vi.mock('@/component-library', () => ({
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

const setConfigMock = vi.hoisted(() => vi.fn());
const notificationSuccessMock = vi.hoisted(() => vi.fn());
const notificationErrorMock = vi.hoisted(() => vi.fn());
const eventBusEmitMock = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/config', () => ({
  configManager: {
    setConfig: setConfigMock,
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: notificationSuccessMock,
    error: notificationErrorMock,
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: eventBusEmitMock,
  },
}));

function execCommandItem(cmd: string, status: FlowToolItem['status'] = 'pending_confirmation'): FlowToolItem {
  return {
    id: 'tool-exec-1',
    type: 'tool',
    toolName: 'ExecCommand',
    status,
    timestamp: Date.now(),
    toolCall: {
      id: 'call-exec-1',
      input: { cmd },
    },
  };
}

describe('ToolApprovalBar', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    setConfigMock.mockResolvedValue(undefined);
    notificationSuccessMock.mockClear();
    notificationErrorMock.mockClear();
    eventBusEmitMock.mockClear();

    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    (dom.window.HTMLElement.prototype as any).attachEvent = vi.fn();
    (dom.window.HTMLElement.prototype as any).detachEvent = vi.fn();

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders shared approval actions for pending ExecCommand tools', () => {
    const onConfirm = vi.fn();
    const onReject = vi.fn();
    const input = { cmd: 'npm test' };

    act(() => {
      root.render(
        <ToolApprovalBar
          toolItem={{ ...execCommandItem(input.cmd), toolCall: { id: 'call-exec-1', input } }}
          onConfirm={onConfirm}
          onReject={onReject}
        />,
      );
    });

    expect(container.textContent).toContain('Waiting for confirmation');

    const allowButton = container.querySelector('button[aria-label="Allow"]') as HTMLButtonElement;
    const rejectButton = container.querySelector('button[aria-label="Reject"]') as HTMLButtonElement;

    act(() => {
      allowButton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      rejectButton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onConfirm).toHaveBeenCalledWith(input);
    expect(onReject).toHaveBeenCalledWith();
  });

  it('submits a rejection instruction from the shared approval bar', () => {
    const onReject = vi.fn();

    act(() => {
      root.render(
        <ToolApprovalBar
          toolItem={execCommandItem('npm test')}
          onReject={onReject}
        />,
      );
    });

    const rejectWithInstructionButton = container.querySelector(
      'button[aria-label="Reject with instruction"]',
    ) as HTMLButtonElement;
    act(() => {
      rejectWithInstructionButton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    const input = container.querySelector('input[aria-label="Rejection instruction"]') as HTMLInputElement;
    act(() => {
      input.value = 'Use the status panel instead';
      input.dispatchEvent(new dom.window.Event('change', { bubbles: true }));
    });

    const submitButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent === 'Reject') as HTMLButtonElement;
    act(() => {
      submitButton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onReject).toHaveBeenCalledWith({ instruction: 'Use the status panel instead' });
  });

  it('disables approval for an empty ExecCommand input', () => {
    act(() => {
      root.render(<ToolApprovalBar toolItem={execCommandItem('   ')} />);
    });

    const allowButton = container.querySelector('button[aria-label="Allow"]') as HTMLButtonElement;
    expect(allowButton.disabled).toBe(true);
    expect(allowButton.title).toBe('This tool has no executable input');
  });

  it('enables tool auto execute from the shared approval bar', async () => {
    await act(async () => {
      root.render(<ToolApprovalBar toolItem={execCommandItem('npm test')} />);
    });

    const autoExecuteButton = container.querySelector(
      'button[aria-label="Enable tool auto execute"]',
    ) as HTMLButtonElement;

    expect(autoExecuteButton.title).toBe('开启工具自动执行');

    await act(async () => {
      autoExecuteButton.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(setConfigMock).toHaveBeenCalledWith('ai.skip_tool_confirmation', true);
    expect(notificationSuccessMock).toHaveBeenCalledWith('Tool auto execute enabled', { duration: 2000 });
    expect(eventBusEmitMock).toHaveBeenCalledWith('mode:config:updated');
  });

  it('does not render for non-confirmation statuses', () => {
    act(() => {
      root.render(<ToolApprovalBar toolItem={execCommandItem('npm test', 'running')} />);
    });

    expect(container.textContent).toBe('');
  });

  it('shows remaining confirmation time when confirmation timeout is set', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-27T12:00:00.000Z'));

    try {
      act(() => {
        root.render(
          <ToolApprovalBar
            toolItem={{ ...execCommandItem('npm test'), confirmationTimeoutAt: Date.now() + 65_000 }}
          />,
        );
      });

      expect(container.textContent).toContain('Waiting for confirmation');
      expect(container.textContent).toContain('1m 5s');
    } finally {
      vi.useRealTimers();
    }
  });

  it('hides remaining confirmation time when timeout is longer than ten minutes', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-06-27T12:00:00.000Z'));

    try {
      act(() => {
        root.render(
          <ToolApprovalBar
            toolItem={{ ...execCommandItem('npm test'), confirmationTimeoutAt: Date.now() + 11 * 60 * 1000 }}
          />,
        );
      });

      expect(container.textContent).toContain('Waiting for confirmation');
      expect(container.textContent).not.toContain('remaining');
    } finally {
      vi.useRealTimers();
    }
  });
});
