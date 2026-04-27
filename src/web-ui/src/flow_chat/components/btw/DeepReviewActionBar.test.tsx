import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { useReviewActionBarStore } from '../../store/deepReviewActionBarStore';

const sendMessageMock = vi.hoisted(() => vi.fn());
const eventBusEmitMock = vi.hoisted(() => vi.fn());
const confirmWarningMock = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
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
    emit: eventBusEmitMock,
  },
}));

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmWarning: confirmWarningMock,
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
    info: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      sessions: new Map(),
      activeSessionId: null,
    }),
    subscribe: () => () => {},
  },
}));

vi.mock('../../utils/deepReviewExperience', () => ({
  aggregateReviewerProgress: () => [],
  buildReviewerProgressSummary: () => null,
  extractPartialReviewData: () => null,
  buildErrorAttribution: () => null,
  buildRecoveryPlan: () => null,
  evaluateDegradationOptions: () => [],
}));

vi.mock('../../services/DeepReviewContinuationService', () => ({
  continueDeepReviewSession: vi.fn(),
}));

vi.mock('@/shared/ai-errors/aiErrorPresenter', () => ({
  getAiErrorPresentation: () => ({
    category: 'network',
    titleKey: 'test',
    messageKey: 'test',
    diagnostics: 'test diagnostics',
    actions: [],
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
    confirmWarningMock.mockResolvedValue(true);
    eventBusEmitMock.mockReturnValue(false);
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

  it('asks for confirmation before replacing existing chat input text', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    eventBusEmitMock.mockImplementation((event: string, payload: { getValue?: () => string }) => {
      if (event === 'chat-input:get-state') {
        payload.getValue = () => 'existing draft';
      }
      return true;
    });
    confirmWarningMock.mockResolvedValue(false);

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const fillButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Fill to input'));
    expect(fillButton).toBeTruthy();

    await act(async () => {
      fillButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(confirmWarningMock).toHaveBeenCalledTimes(1);
    expect(eventBusEmitMock).not.toHaveBeenCalledWith('fill-chat-input', expect.anything());
    expect(useReviewActionBarStore.getState().dismissed).toBe(false);
  });

  it('fills chat input without confirmation when current input is empty', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    eventBusEmitMock.mockImplementation((event: string, payload: { getValue?: () => string }) => {
      if (event === 'chat-input:get-state') {
        payload.getValue = () => '  ';
      }
      return true;
    });

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const fillButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Fill to input'));
    expect(fillButton).toBeTruthy();

    await act(async () => {
      fillButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(confirmWarningMock).not.toHaveBeenCalled();
    expect(eventBusEmitMock).toHaveBeenCalledWith('fill-chat-input', expect.objectContaining({
      mode: 'replace',
    }));
    expect(useReviewActionBarStore.getState().dismissed).toBe(true);
  });

  it('dismisses action bar when close button is clicked', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const closeButton = container.querySelector('.deep-review-action-bar__close');
    expect(closeButton).toBeTruthy();

    await act(async () => {
      closeButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(useReviewActionBarStore.getState().dismissed).toBe(true);
  });

  it('marks completed remediation items when fix completes', async () => {
    const store = useReviewActionBarStore.getState();
    store.showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      phase: 'review_completed',
    });

    // Select all items
    const items = store.remediationItems;
    for (const item of items) {
      store.toggleRemediation(item.id);
    }

    store.setActiveAction('fix');
    store.updatePhase('fix_running');

    // Simulate fix completion
    store.updatePhase('fix_completed');

    const state = useReviewActionBarStore.getState();
    expect(state.completedRemediationIds.size).toBe(2);
    expect(state.phase).toBe('fix_completed');
    expect(state.fixingRemediationIds.size).toBe(0);
  });

  it('shows completed items as disabled and strikethrough', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      phase: 'review_completed',
      completedRemediationIds: new Set(['remediation-0']),
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const completedItem = container.querySelector('.deep-review-action-bar__remediation-item--completed');
    expect(completedItem).toBeTruthy();

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    expect(checkboxes.length).toBeGreaterThanOrEqual(2);
  });

  it('shows continue fix UI when phase is fix_interrupted', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      phase: 'fix_interrupted',
    });

    // Set remaining fix IDs directly on state
    const store = useReviewActionBarStore.getState();
    (store as unknown as { remainingFixIds: string[] }).remainingFixIds = ['remediation-0'];

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const continueButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Continue fixing'));
    expect(continueButton).toBeTruthy();

    const skipButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Skip remaining'));
    expect(skipButton).toBeTruthy();
  });

  it('skips remaining fixes and returns to review_completed', async () => {
    const store = useReviewActionBarStore.getState();
    store.showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      phase: 'fix_interrupted',
    });

    store.skipRemainingFixes();

    const state = useReviewActionBarStore.getState();
    expect(state.phase).toBe('review_completed');
    expect(state.remainingFixIds).toEqual([]);
    expect(state.activeAction).toBeNull();
  });
});
