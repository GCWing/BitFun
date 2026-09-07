// @vitest-environment jsdom

import React, { act } from 'react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { compileString } from 'sass';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PermissionRequest } from '@/infrastructure/api/service-api/AgentAPI';
import { ChatInputApprovalBand } from './ChatInputApprovalBand';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const TRANSLATIONS: Record<string, string> = {
  'permission.actions.edit': 'Edit files',
  'permission.actions.bash': 'Run command',
  'permission.actions.other': 'Other action',
  'permission.allowOnce': 'Allow once',
  'permission.allowAlways': 'Always allow',
  'permission.allowCurrentAndFollowing': 'Allow all',
  'permission.reject': 'Reject',
  'permission.rejectCurrentAndFollowing': 'Reject all',
  'permission.rejectWithReason': 'Reject with reason',
  'permission.responseFailed': 'The reply could not be delivered.',
  'permission.scopeThis': 'This one',
  'permission.scopeAll': 'All',
};

vi.mock('react-i18next', async importOriginal => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({
    t: (key: string, values?: Record<string, string>) => {
      if (key === 'permission.subagentOwner') return `${values?.subagent} subagent`;
      if (key === 'permission.allowAlwaysTooltip') {
        return `Always allow saves matching access for ${values?.projectPath}`;
      }
      if (key === 'permission.risks.pageSave') {
        return `Save ${values?.slug} as ${values?.visibility} without deploying.`;
      }
      return TRANSLATIONS[key] ?? key;
    },
  }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  Tooltip: ({ children }: { children: React.ReactElement }) => <>{children}</>,
}));


function request(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    requestId: 'request-1',
    roundId: 'round-1',
    order: 0,
    sessionId: 'session-1',
    toolCallId: 'tool-1',
    projectPath: '/workspace/OpenBitFun',
    projectId: 'project-1',
    agentId: 'agentic',
    action: 'edit',
    resources: ['src/main.rs'],
    saveResources: ['src/main.rs'],
    source: { kind: 'tool_call', identity: 'Write' },
    ...overrides,
  };
}

describe('ChatInputApprovalBand', () => {
  let container: HTMLDivElement;
  let root: Root;
  let stylesheet: HTMLStyleElement;

  beforeEach(() => {
    stylesheet = document.createElement('style');
    const filename = 'ChatInputApprovalBand.scss';
    stylesheet.textContent = compileString(readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), filename), 'utf8',
    )).css;
    document.head.appendChild(stylesheet);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    stylesheet.remove();
    vi.clearAllMocks();
  });

  const click = async (testId: string) => {
    await act(async () => {
      const button = container.querySelector<HTMLButtonElement>(`[data-testid="${testId}"]`);
      expect(button).not.toBeNull();
      expect(button!.disabled).toBe(false);
      button!.click();
    });
  };

  it('says what is being asked for and who is asking', async () => {
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request({
            delegation: {
              parentSessionId: 'session-1',
              parentDialogTurnId: 'turn-1',
              parentToolCallId: 'task-1',
              subagentType: 'Explore',
            },
          })]}
          onRespond={vi.fn(async () => undefined)}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });

    const band = container.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]');
    expect(band?.textContent).toContain('Edit files');
    expect(band?.textContent).toContain('src/main.rs');
    expect(band?.textContent).toContain('Explore subagent');
  });

  it('keeps the risk on its own line so it cannot be answered unread', async () => {
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request({
            action: 'page_publish',
            displayMetadata: {
              pageOperation: 'save',
              pageSlug: 'status',
              pageVisibility: 'public',
            },
          })]}
          onRespond={vi.fn(async () => undefined)}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });

    expect(container.querySelector('[data-openbitfun-part="risk"]')?.textContent).toBe(
      'Save status as permission.visibility.public without deploying.',
    );
  });

  it('answers only the request in front of the reader by default', async () => {
    const onRespond = vi.fn(async () => undefined);
    const onRespondBatch = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request()]}
          onRespond={onRespond}
          onRespondBatch={onRespondBatch}
        />
      );
    });

    // A lone request has nothing to scope, so the toggle stays out of the way.
    expect(container.querySelector('[role="radio"][data-openbitfun-value="all"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-input-approval-pending-count"]')).toBeNull();

    await click('chat-input-approval-allow');
    expect(onRespond).toHaveBeenCalledWith('request-1', 'once', undefined);
    expect(onRespondBatch).not.toHaveBeenCalled();
  });

  it('extends an answer to the queue only when the reader picks that scope', async () => {
    const onRespond = vi.fn(async () => undefined);
    const onRespondBatch = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request(), request({ requestId: 'request-2', toolCallId: 'tool-2' })]}
          totalPendingCount={3}
          onRespond={onRespond}
          onRespondBatch={onRespondBatch}
        />
      );
    });

    expect(
      container.querySelector('[data-testid="chat-input-approval-pending-count"]')?.textContent,
    ).toBe('+2');
    expect(
      container.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]')
        ?.dataset.approvalScope,
    ).toBe('this');

    const currentScope = container.querySelector<HTMLButtonElement>(
      '[role="radio"][data-openbitfun-value="this"]',
    );
    expect(currentScope).not.toBeNull();
    await act(async () => {
      currentScope!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });
    expect(container.querySelector('[role="radio"][data-openbitfun-value="all"]')
      ?.getAttribute('aria-checked')).toBe('true');
    const band = container.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]');
    expect(band?.dataset.approvalScope).toBe('all');
    expect(band?.textContent).toContain('Reject all');
    // A saved grant is never applied to requests the reader has not seen.
    expect(container.querySelector('[data-testid="chat-input-approval-allow-always"]')).toBeNull();

    await click('chat-input-approval-reject');
    expect(onRespondBatch).toHaveBeenCalledWith('request-1', 'reject', undefined);
    expect(onRespond).not.toHaveBeenCalled();
  });

  it('offers the composer text as a rejection reason instead of assuming it is one', async () => {
    const onRespond = vi.fn(async () => undefined);
    const onRejectReasonConsumed = vi.fn();

    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request()]}
          onRespond={onRespond}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });
    // With an empty composer there is no reason to offer, and allowing is not
    // gated on the composer being empty either.
    expect(
      container.querySelector('[data-testid="chat-input-approval-reject-with-reason"]'),
    ).toBeNull();
    expect(
      container.querySelector<HTMLButtonElement>('[data-testid="chat-input-approval-allow"]')?.disabled,
    ).toBe(false);

    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request()]}
          rejectReason="  use rg instead  "
          onRejectReasonConsumed={onRejectReasonConsumed}
          onRespond={onRespond}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });

    // A draft in the composer never disables allowing; it only adds a way to
    // reject that carries the draft as the reason.
    expect(
      container.querySelector<HTMLButtonElement>('[data-testid="chat-input-approval-allow"]')?.disabled,
    ).toBe(false);

    await click('chat-input-approval-reject');
    expect(onRespond).toHaveBeenLastCalledWith('request-1', 'reject', undefined);
    expect(onRejectReasonConsumed).not.toHaveBeenCalled();

    await click('chat-input-approval-reject-with-reason');
    expect(onRespond).toHaveBeenLastCalledWith('request-1', 'reject', 'use rg instead');
    expect(onRejectReasonConsumed).toHaveBeenCalledTimes(1);
  });

  it('reports a reply that never landed and leaves the request answerable', async () => {
    const onRespond = vi.fn(async () => {
      throw new Error('offline');
    });
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request()]}
          onRespond={onRespond}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });

    await click('chat-input-approval-allow');
    expect(container.querySelector('[data-openbitfun-part="error"]')?.textContent).toBe(
      'The reply could not be delivered.',
    );
    expect(
      container.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]')
        ?.dataset.openbitfunState,
    ).toBe('error');
    expect(
      container.querySelector<HTMLButtonElement>('[data-testid="chat-input-approval-allow"]')?.disabled,
    ).toBe(false);
  });

  it.each([
    ['unbroken command', `printf '${'x'.repeat(40000)}'`],
    ['multiline command', Array.from({ length: 300 }, (_, index) => `printf 'line ${index}\n'`).join('\n')],
  ])('keeps the full %s separate from answer controls and copies it intact', async (_, command) => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    const onRespond = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[request({ action: 'bash', resources: [command] })]}
          onRespond={onRespond}
          onRespondBatch={vi.fn(async () => undefined)}
        />,
      );
    });

    const band = container.querySelector<HTMLElement>('[data-testid="chat-input-approval-band"]')!;
    const resource = band.querySelector<HTMLElement>('[role="region"]')!;
    const actions = band.querySelector<HTMLElement>('[data-openbitfun-part="actions"]')!;
    expect(resource.textContent).toBe(command);
    expect(resource.tabIndex).toBe(0);
    expect(resource.contains(actions)).toBe(false);
    expect(band.querySelector('[data-openbitfun-component="card"]')?.getAttribute('data-appearance'))
      .toBe('subtle');

    // jsdom checks the compiled containment rules; real viewport layout is a
    // separate manual check, since jsdom does not calculate element geometry.
    const bandStyle = getComputedStyle(band);
    expect(bandStyle.width).toBe('100%');
    expect(bandStyle.maxWidth).toBe('100%');
    expect(bandStyle.minWidth).toBe('0px');
    const resourceStyle = getComputedStyle(resource);
    expect(resourceStyle.whiteSpace).toBe('pre-wrap');
    expect(resourceStyle.overflowWrap).toBe('anywhere');
    expect(resourceStyle.overflow).toBe('auto');
    expect(resourceStyle.maxHeight).toBe('min(10rem, 25dvh)');
    expect(getComputedStyle(actions).flexWrap).toBe('wrap');

    await click('chat-input-approval-copy');
    expect(writeText).toHaveBeenCalledWith(command);
    await click('chat-input-approval-allow');
    expect(onRespond).toHaveBeenLastCalledWith('request-1', 'once', undefined);
    await click('chat-input-approval-reject');
    expect(onRespond).toHaveBeenLastCalledWith('request-1', 'reject', undefined);
  });

  it('renders nothing when there is nothing to approve', async () => {
    await act(async () => {
      root.render(
        <ChatInputApprovalBand
          requests={[]}
          onRespond={vi.fn(async () => undefined)}
          onRespondBatch={vi.fn(async () => undefined)}
        />
      );
    });
    expect(container.querySelector('[data-testid="chat-input-approval-band"]')).toBeNull();
  });
});
