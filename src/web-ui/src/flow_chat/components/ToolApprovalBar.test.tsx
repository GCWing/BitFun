import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import type { FlowToolItem } from '../types/flow-chat';
import { ToolApprovalBar } from './ToolApprovalBar';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const messages: Record<string, string> = {
  'toolCards.approval.ariaLabel': 'Tool approval',
  'toolCards.approval.waiting': 'Waiting for permission',
  'toolCards.acpPermission.allowOnce': 'Allow once',
  'toolCards.acpPermission.allowAlways': 'Always allow',
  'toolCards.acpPermission.reject': 'Reject',
  'toolCards.acpPermission.rejectAlways': 'Always reject',
  'toolCards.acpPermission.selectOption': 'Select option',
};

vi.mock('react-i18next', async () => {
  const actual = await vi.importActual<typeof import('react-i18next')>('react-i18next');
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => messages[key] ?? key,
    }),
  };
});

function toolItem(status: FlowToolItem['status'] = 'pending_confirmation'): FlowToolItem {
  return {
    id: 'tool-1',
    type: 'tool',
    toolName: 'ExternalTool',
    status,
    timestamp: Date.now(),
    toolCall: {
      id: 'call-1',
      input: {},
    },
  };
}

describe('ToolApprovalBar', () => {
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

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.unstubAllGlobals();
  });

  it('does not render the retired confirmation controls without ACP options', () => {
    act(() => root.render(<ToolApprovalBar toolItem={toolItem()} />));

    expect(container.textContent).toBe('');
  });

  it('routes ACP option identities through the approval callbacks', () => {
    const onConfirm = vi.fn();
    const onReject = vi.fn();
    const item = toolItem();
    item.acpPermission = {
      permissionId: 'permission-1',
      requestedAt: Date.now(),
      options: [
        { optionId: 'reject-once', name: 'Reject this request', kind: 'reject_once' },
        { optionId: 'allow-always', name: 'Trust this tool', kind: 'allow_always' },
      ],
    };

    act(() => {
      root.render(
        <ToolApprovalBar toolItem={item} onConfirm={onConfirm} onReject={onReject} />,
      );
    });

    expect(container.textContent).toContain('Waiting for permission');
    const buttons = Array.from(container.querySelectorAll('button'));
    const allowButton = buttons.find((button) => button.textContent === 'Always allow');
    const rejectButton = buttons.find((button) => button.textContent === 'Reject');

    act(() => {
      allowButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      rejectButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onConfirm).toHaveBeenCalledWith('allow-always', true);
    expect(onReject).toHaveBeenCalledWith({ permissionOptionId: 'reject-once' });
  });

  it('does not render ACP options after the tool leaves the pending state', () => {
    const item = toolItem('running');
    item.acpPermission = {
      permissionId: 'permission-1',
      requestedAt: Date.now(),
      options: [{ optionId: 'allow-once', name: 'Allow', kind: 'allow_once' }],
    };

    act(() => root.render(<ToolApprovalBar toolItem={item} />));

    expect(container.textContent).toBe('');
  });
});
