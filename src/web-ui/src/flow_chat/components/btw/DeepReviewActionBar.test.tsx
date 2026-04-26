import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { useReviewActionBarStore } from '../../store/deepReviewActionBarStore';

const sendMessageMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
    onClick,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  Checkbox: ({
    checked,
    onChange,
  }: {
    checked?: boolean;
    onChange?: () => void;
  }) => (
    <input type="checkbox" checked={checked} readOnly onClick={onChange} />
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../services/FlowChatManager', () => ({
  flowChatManager: {
    sendMessage: sendMessageMock,
  },
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
  }),
}));

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean; url?: string }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describeWithJsdom('DeepReviewActionBar', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });

    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('localStorage', window.localStorage);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    sendMessageMock.mockResolvedValue(undefined);
    useReviewActionBarStore.getState().reset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
    useReviewActionBarStore.getState().reset();
  });

  it('keeps remediation in progress after submitting a fix turn', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: {
          recommended_action: 'request_changes',
        },
        issues: [
          {
            severity: 'high',
            title: 'Incorrect branch',
          },
        ],
        remediation_plan: ['Fix the incorrect branch.'],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const startFixButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Start fixing'));

    expect(startFixButton).toBeTruthy();

    await act(async () => {
      startFixButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(sendMessageMock).toHaveBeenCalledTimes(1);
    expect(useReviewActionBarStore.getState().phase).toBe('fix_running');
  });

  it('uses standard review mode when starting Code Review remediation', async () => {
    const { ReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'review-session',
      parentSessionId: 'parent-session',
      reviewMode: 'standard',
      reviewData: {
        summary: {
          recommended_action: 'request_changes',
        },
        remediation_plan: ['Fix the standard review finding.'],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<ReviewActionBar />);
    });

    const fixAndReviewButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Fix and re-review'));

    expect(fixAndReviewButton).toBeTruthy();

    await act(async () => {
      fixAndReviewButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(sendMessageMock).toHaveBeenCalledTimes(1);
    const [prompt, sessionId, displayMessage, agentType] = sendMessageMock.mock.calls[0];
    expect(prompt).toContain('selected Code Review findings only');
    expect(prompt).toContain('follow-up standard code review');
    expect(sessionId).toBe('review-session');
    expect(displayMessage).toBe('Fix Code Review findings and re-review');
    expect(agentType).toBe('CodeReview');
  });
});
