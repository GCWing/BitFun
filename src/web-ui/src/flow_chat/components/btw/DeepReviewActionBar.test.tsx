import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { useReviewActionBarStore } from '../../store/deepReviewActionBarStore';

const sendMessageMock = vi.hoisted(() => vi.fn());
const eventBusEmitMock = vi.hoisted(() => vi.fn());
const confirmWarningMock = vi.hoisted(() => vi.fn());
const continueDeepReviewSessionMock = vi.hoisted(() => vi.fn());
const buildErrorAttributionMock = vi.hoisted(() => vi.fn(() => null));
const buildRecoveryPlanMock = vi.hoisted(() => vi.fn(() => ({
  willPreserve: ['ReviewSecurity'],
  willRerun: ['ReviewPerformance'],
  willSkip: [],
  summaryText: '1 completed reviewer will be preserved; 1 reviewer will be rerun',
})));
const controlDeepReviewQueueMock = vi.hoisted(() => vi.fn());
const flowChatSessionsMock = vi.hoisted(() => new Map<string, unknown>());

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
      const template = options?.defaultValue ?? _key;
      return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
    },
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
    disabled,
    indeterminate,
    label,
    onChange,
  }: {
    checked?: boolean;
    disabled?: boolean;
    indeterminate?: boolean;
    label?: React.ReactNode;
    onChange?: () => void;
  }) => (
    <label>
      <input
        type="checkbox"
        aria-checked={indeterminate ? 'mixed' : checked ? 'true' : 'false'}
        checked={checked}
        disabled={disabled}
        readOnly
        onClick={() => {
          if (!disabled) {
            onChange?.();
          }
        }}
      />
      {label}
    </label>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../services/FlowChatManager', () => ({
  flowChatManager: {
    sendMessage: sendMessageMock,
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    controlDeepReviewQueue: controlDeepReviewQueueMock,
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
      sessions: flowChatSessionsMock,
      activeSessionId: null,
    }),
    subscribe: () => () => {},
  },
}));

vi.mock('../../utils/deepReviewExperience', () => ({
  aggregateReviewerProgress: () => [],
  buildReviewerProgressSummary: () => null,
  extractPartialReviewData: () => null,
  buildErrorAttribution: buildErrorAttributionMock,
  buildRecoveryPlan: buildRecoveryPlanMock,
  evaluateDegradationOptions: () => [],
}));

vi.mock('../../services/DeepReviewContinuationService', () => ({
  continueDeepReviewSession: continueDeepReviewSessionMock,
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
    continueDeepReviewSessionMock.mockResolvedValue(undefined);
    buildErrorAttributionMock.mockReturnValue(null);
    flowChatSessionsMock.clear();
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
    flowChatSessionsMock.clear();
    useReviewActionBarStore.getState().reset();
  });

  it('keeps remediation in progress after submitting a fix turn', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    flowChatSessionsMock.set('child-session', {
      sessionId: 'child-session',
      sessionKind: 'review',
      dialogTurns: [
        {
          id: 'review-turn-1',
          status: 'completed',
          modelRounds: [],
        },
      ],
    });

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
    const state = useReviewActionBarStore.getState();
    expect(state.phase).toBe('fix_running');
    expect(state.minimized).toBe(true);
    expect(state.fixingBaselineTurnId).toBe('review-turn-1');
    expect(container.textContent).toContain('Fix the incorrect branch.');
    expect(container.textContent).toContain('Fixing');
    const itemCheckbox = container.querySelector<HTMLInputElement>(
      '.deep-review-action-bar__remediation-item input[type="checkbox"]',
    );
    expect(itemCheckbox?.disabled).toBe(true);
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
    expect(useReviewActionBarStore.getState().minimized).toBe(false);
  });

  it('fills chat input and minimizes the action bar when current input is empty', async () => {
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
    expect(useReviewActionBarStore.getState().minimized).toBe(true);
  });

  it('minimizes action bar when close button is clicked', async () => {
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

    const closeButton = container.querySelector('.deep-review-action-bar__controls-btn');
    expect(closeButton).toBeTruthy();

    await act(async () => {
      closeButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const state = useReviewActionBarStore.getState();
    expect(state.minimized).toBe(true);
  });

  it('does not show capacity queue controls when there is no queue state', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

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

    expect(container.textContent).not.toContain('Reviewers waiting for capacity');
    expect(Array.from(container.querySelectorAll('button')).some((button) => (
      button.textContent?.includes('Pause queue')
    ))).toBe(false);
  });

  it('shows compact capacity queue controls and keeps them locally adjustable', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      phase: 'review_completed',
    });
    useReviewActionBarStore.setState({
      capacityQueueState: {
        status: 'queued_for_capacity',
        reason: 'provider_concurrency_limit',
        queuedReviewerCount: 2,
        activeReviewerCount: 1,
        optionalReviewerCount: 1,
        queueElapsedMs: 12_000,
        maxQueueWaitSeconds: 60,
        sessionConcurrencyHigh: true,
      },
    } as Partial<ReturnType<typeof useReviewActionBarStore.getState>>);

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    expect(container.textContent).toContain('Waiting for model capacity');
    expect(container.textContent).toContain('BitFun is waiting for temporary model capacity.');
    expect(container.textContent).toContain('Reason: provider concurrency limit');
    expect(container.textContent).toContain('Waited 12s of 1m 0s');
    expect(container.textContent).toContain('Your active session is busy.');
    expect(container.textContent).not.toContain('Run slower next time');
    expect(container.textContent).toContain('Open Review settings');

    const pauseButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Pause queue'));
    expect(pauseButton).toBeTruthy();

    await act(async () => {
      pauseButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect((useReviewActionBarStore.getState() as unknown as {
      capacityQueueState: { status: string };
    }).capacityQueueState.status).toBe('paused_by_user');
    expect(container.textContent).toContain('Queue paused');

    const openSettingsButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Open Review settings'));
    expect(openSettingsButton).toBeTruthy();

    await act(async () => {
      openSettingsButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const { useSettingsStore } = await import('@/app/scenes/settings/settingsStore');
    expect(useSettingsStore.getState().activeTab).toBe('review');
  });

  it('sends backend queue control actions for event-driven capacity waits', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');
    controlDeepReviewQueueMock.mockResolvedValue(undefined);

    useReviewActionBarStore.getState().showCapacityQueueBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      capacityQueueState: {
        toolId: 'task-queue-1',
        subagentType: 'ReviewSecurity',
        dialogTurnId: 'turn-queue-1',
        status: 'queued_for_capacity',
        queuedReviewerCount: 2,
        activeReviewerCount: 1,
        optionalReviewerCount: 1,
        controlMode: 'backend',
        waitingReviewers: [
          {
            toolId: 'task-queue-1',
            subagentType: 'ReviewSecurity',
            displayName: 'Security reviewer',
            status: 'queued_for_capacity',
          },
          {
            toolId: 'task-queue-2',
            subagentType: 'ReviewFrontend',
            displayName: 'Frontend reviewer',
            status: 'queued_for_capacity',
          },
        ],
      },
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const pauseButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Pause queue'));
    expect(pauseButton).toBeTruthy();

    await act(async () => {
      pauseButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(controlDeepReviewQueueMock).toHaveBeenCalledTimes(2);
    expect(controlDeepReviewQueueMock).toHaveBeenCalledWith({
      sessionId: 'child-session',
      dialogTurnId: 'turn-queue-1',
      toolId: 'task-queue-1',
      action: 'pause',
    });
    expect(controlDeepReviewQueueMock).toHaveBeenCalledWith({
      sessionId: 'child-session',
      dialogTurnId: 'turn-queue-1',
      toolId: 'task-queue-2',
      action: 'pause',
    });
    expect((useReviewActionBarStore.getState() as unknown as {
      capacityQueueState: { status: string };
    }).capacityQueueState.status).toBe('paused_by_user');
  });

  it('shows the backend reason when queue control fails', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');
    const { notificationService } = await import('@/shared/notification-system');
    controlDeepReviewQueueMock.mockRejectedValueOnce(new Error('backend queue already closed'));

    useReviewActionBarStore.getState().showCapacityQueueBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      capacityQueueState: {
        toolId: 'task-queue-1',
        subagentType: 'ReviewSecurity',
        dialogTurnId: 'turn-queue-1',
        status: 'queued_for_capacity',
        queuedReviewerCount: 1,
        controlMode: 'backend',
      },
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const pauseButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Pause queue'));
    expect(pauseButton).toBeTruthy();

    await act(async () => {
      pauseButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(notificationService.error).toHaveBeenCalledWith(
      expect.stringContaining('backend queue already closed'),
    );
    expect(notificationService.error).toHaveBeenCalledWith(
      expect.stringContaining('use Stop to interrupt the review'),
    );
  });

  it('reports partial backend queue control failures without claiming full success', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');
    const { notificationService } = await import('@/shared/notification-system');
    controlDeepReviewQueueMock
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error('tool already running'));

    useReviewActionBarStore.getState().showCapacityQueueBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      capacityQueueState: {
        toolId: 'task-queue-1',
        subagentType: 'ReviewSecurity',
        dialogTurnId: 'turn-queue-1',
        status: 'queued_for_capacity',
        queuedReviewerCount: 2,
        controlMode: 'backend',
        waitingReviewers: [
          {
            toolId: 'task-queue-1',
            subagentType: 'ReviewSecurity',
            displayName: 'Security reviewer',
            status: 'queued_for_capacity',
          },
          {
            toolId: 'task-queue-2',
            subagentType: 'ReviewFrontend',
            displayName: 'Frontend reviewer',
            status: 'queued_for_capacity',
          },
        ],
      },
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const pauseButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Pause queue'));
    expect(pauseButton).toBeTruthy();

    await act(async () => {
      pauseButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(controlDeepReviewQueueMock).toHaveBeenCalledTimes(2);
    expect(notificationService.error).toHaveBeenCalledWith(
      expect.stringContaining('1 of 2 reviewers failed'),
    );
    expect(notificationService.error).toHaveBeenCalledWith(
      expect.stringContaining('tool already running'),
    );
    expect((useReviewActionBarStore.getState() as unknown as {
      capacityQueueState: { status: string };
    }).capacityQueueState.status).toBe('queued_for_capacity');
  });

  it('starts a structured retry turn for explicit incomplete Deep Review slices', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    flowChatSessionsMock.set('deep-review-session', {
      sessionId: 'deep-review-session',
      sessionKind: 'deep_review',
      deepReviewRunManifest: {
        reviewMode: 'deep',
        workPackets: [
          {
            packetId: 'reviewer:ReviewSecurity:group-1-of-2',
            phase: 'reviewer',
            launchBatch: 0,
            subagentId: 'ReviewSecurity',
            displayName: 'Security reviewer',
            roleName: 'Security reviewer',
            assignedScope: {
              kind: 'review_target',
              targetSource: 'session_files',
              targetResolution: 'resolved',
              targetTags: ['security'],
              fileCount: 2,
              files: ['src/auth.ts', 'src/session.ts'],
              excludedFileCount: 0,
              groupIndex: 1,
              groupCount: 2,
            },
            allowedTools: ['Read', 'GetFileDiff'],
            timeoutSeconds: 300,
            requiredOutputFields: ['summary', 'findings'],
            strategyLevel: 'deep',
            strategyDirective: 'Review security-sensitive changes.',
            model: 'fast-model',
          },
        ],
      },
    });

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'deep-review-session',
      parentSessionId: 'parent-session',
      reviewMode: 'deep',
      reviewData: {
        review_mode: 'deep',
        summary: { recommended_action: 'request_changes' },
        reviewers: [
          {
            name: 'Security reviewer',
            specialty: 'security',
            status: 'partial_timeout',
            summary: 'Timed out after completing src/session.ts.',
            packet_id: 'reviewer:ReviewSecurity:group-1-of-2',
            covered_files: ['src/session.ts'],
            retry_scope_files: ['src/auth.ts'],
          },
        ],
      },
      phase: 'review_completed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const retryButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Retry incomplete slices'));
    expect(retryButton).toBeTruthy();

    await act(async () => {
      retryButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(sendMessageMock).toHaveBeenCalledTimes(1);
    const [prompt, sessionId, displayMessage, agentType] = sendMessageMock.mock.calls[0];
    expect(prompt).toContain('"retry_coverage"');
    expect(prompt).toContain('"source_packet_id": "reviewer:ReviewSecurity:group-1-of-2"');
    expect(prompt).toContain('"retry_scope_files"');
    expect(sessionId).toBe('deep-review-session');
    expect(displayMessage).toContain('Retry 1 incomplete');
    expect(agentType).toBe('DeepReview');
  });

  it('shows distinct progress text after starting fix and re-review', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

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

    const fixAndReviewButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Fix and re-review'));
    expect(fixAndReviewButton).toBeTruthy();

    await act(async () => {
      fixAndReviewButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Fixing and preparing re-review...');
  });

  it('requires explicit decision confirmation before executing selected decision remediation', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');

    useReviewActionBarStore.getState().showActionBar({
      childSessionId: 'child-session',
      parentSessionId: 'parent-session',
      reviewData: {
        review_mode: 'deep',
        summary: { recommended_action: 'request_changes' },
        report_sections: {
          remediation_groups: {
            needs_decision: [{
              question: 'Which migration strategy should we use?',
              plan: 'Choose a migration strategy before editing.',
              options: ['Fast path', 'Staged path'],
              tradeoffs: 'Fast path is risky; staged path is safer.',
              recommendation: 1,
            }],
          },
        },
      },
      phase: 'review_completed',
    });
    useReviewActionBarStore.getState().setSelectedRemediationIds(new Set(['remediation-needs_decision-0']));

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

    expect(sendMessageMock).not.toHaveBeenCalled();
    expect(container.textContent).toContain('Confirm decision items before fixing');
    expect(container.textContent).toContain('Which migration strategy should we use?');
    expect(container.textContent).toContain('Fast path is risky; staged path is safer.');

    const confirmBeforeSelection = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Confirm and start')) as HTMLButtonElement | undefined;
    expect(confirmBeforeSelection?.disabled).toBe(true);

    const stagedPathButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Staged path'));
    expect(stagedPathButton).toBeTruthy();

    await act(async () => {
      stagedPathButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const confirmButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Confirm and start')) as HTMLButtonElement | undefined;
    expect(confirmButton).toBeTruthy();
    expect(confirmButton?.disabled).toBe(false);

    await act(async () => {
      confirmButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(sendMessageMock).toHaveBeenCalledTimes(1);
    const [prompt] = sendMessageMock.mock.calls[0];
    expect(prompt).toContain('User chose option 2: Staged path');
    expect(prompt).not.toContain('Recommended option 2: Staged path');
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

  it('keeps Deep Review interruption actions in one row without a standalone retry or recovery toggle', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');
    buildErrorAttributionMock.mockReturnValue({
      category: 'network',
      title: 'Network issue',
      severity: 'warning',
      description: 'Please retry later, or check your network and model service status.',
      actions: [
        { code: 'retry', labelKey: 'errors:ai.actions.retry' },
        { code: 'copy_diagnostics', labelKey: 'errors:ai.actions.copyDiagnostics' },
      ],
    });

    useReviewActionBarStore.getState().showInterruptedActionBar({
      childSessionId: 'deep-review-session',
      parentSessionId: 'parent-session',
      interruption: {
        phase: 'resume_failed',
        childSessionId: 'deep-review-session',
        parentSessionId: 'parent-session',
        originalTarget: '/DeepReview review latest commit',
        errorDetail: { category: 'network', rawMessage: 'network timeout' },
        canResume: true,
        recommendedActions: [
          { code: 'retry', labelKey: 'errors:ai.actions.retry' },
          { code: 'switch_model', labelKey: 'errors:ai.actions.switchModel' },
          { code: 'copy_diagnostics', labelKey: 'errors:ai.actions.copyDiagnostics' },
        ],
        reviewers: [
          { reviewer: 'ReviewSecurity', status: 'completed' },
          { reviewer: 'ReviewPerformance', status: 'timed_out' },
        ],
      },
      phase: 'resume_failed',
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const buttonTexts = Array.from(container.querySelectorAll('button'))
      .map((button) => button.textContent ?? '');

    expect(buttonTexts.some((text) => text.includes('Continue review'))).toBe(true);
    expect(buttonTexts.some((text) => text.includes('Switch model'))).toBe(true);
    expect(buttonTexts.some((text) => text.includes('Copy diagnostics'))).toBe(true);
    expect(buttonTexts.some((text) => text.includes('Retry'))).toBe(false);
    expect(buttonTexts.some((text) => text.includes('Show recovery plan'))).toBe(false);
    expect(container.querySelectorAll('.deep-review-action-bar__attribution button')).toHaveLength(0);
    expect(container.querySelector('.deep-review-action-bar__attribution-actions')).toBeNull();
    expect(container.textContent).toContain('1 completed reviewers will be preserved');
    expect(container.textContent).toContain('1 reviewers will be rerun');
  });

  it('minimizes and hides stale interruption controls after a resume request starts successfully', async () => {
    const { DeepReviewActionBar } = await import('./DeepReviewActionBar');
    let resolveContinuation: (() => void) | null = null;
    continueDeepReviewSessionMock.mockReturnValueOnce(new Promise<void>((resolve) => {
      resolveContinuation = resolve;
    }));

    useReviewActionBarStore.getState().showInterruptedActionBar({
      childSessionId: 'deep-review-session',
      parentSessionId: 'parent-session',
      interruption: {
        phase: 'review_interrupted',
        childSessionId: 'deep-review-session',
        parentSessionId: 'parent-session',
        originalTarget: '/DeepReview review latest commit',
        errorDetail: { category: 'network', rawMessage: 'network timeout' },
        canResume: true,
        recommendedActions: [],
        reviewers: [],
      },
    });

    await act(async () => {
      root.render(<DeepReviewActionBar />);
    });

    const continueButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Continue review'));
    expect(continueButton).toBeTruthy();

    await act(async () => {
      continueButton!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const state = useReviewActionBarStore.getState();
    expect(continueDeepReviewSessionMock).toHaveBeenCalledTimes(1);
    expect(state.phase).toBe('resume_running');
    expect(state.minimized).toBe(true);
    expect(state.activeAction).toBe('resume');
    expect(container.textContent).toContain('Continuing review');
    expect(container.textContent).not.toContain('Deep review interrupted');
    expect(Array.from(container.querySelectorAll('button'))
      .some((button) => button.textContent?.includes('Continue review'))).toBe(false);
    expect(Array.from(container.querySelectorAll('button'))
      .some((button) => button.textContent?.includes('Copy diagnostics'))).toBe(false);

    await act(async () => {
      resolveContinuation?.();
      await Promise.resolve();
    });
  });
});
