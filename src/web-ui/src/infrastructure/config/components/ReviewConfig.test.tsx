// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ReviewConfig from './ReviewConfig';

const loadDefaultReviewTeamMock = vi.hoisted(() => vi.fn());
const saveDefaultReviewTeamStrategyLevelMock = vi.hoisted(() => vi.fn());
const saveDefaultReviewTeamConcurrencyPolicyMock = vi.hoisted(() => vi.fn());
const notifySuccessMock = vi.hoisted(() => vi.fn());
const notifyErrorMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => (key: string, params?: Record<string, unknown>) => {
  const translations: Record<string, string> = {
    title: 'Review',
    subtitle: 'Review chooses the right depth for each request without exposing reviewer orchestration.',
    'overview.title': 'Review entry',
    'overview.description': 'Use /review for normal and strict review requests.',
    'overview.command.title': '/review',
    'overview.command.description': 'One command adapts to task size, risk, and requested care.',
    'overview.reviewers.title': 'Adaptive coverage',
    'overview.reviewers.description': 'BitFun chooses enough independent checks for the current task.',
    'overview.qualityGate.title': 'Quality gate',
    'overview.qualityGate.description': 'Findings stay adversarial and evidence based.',
    'strategy.title': 'Review depth',
    'strategy.description': 'Set the default balance between cost, latency, and confidence.',
    'strategy.quick.label': 'Quick',
    'strategy.quick.summary': 'Fast check for small changes.',
    'strategy.normal.label': 'Standard',
    'strategy.normal.summary': 'Balanced review for typical changes.',
    'strategy.deep.label': 'Strict',
    'strategy.deep.summary': 'Higher confidence for larger or riskier changes.',
    'capacity.title': 'Capacity',
    'capacity.description': 'Limit parallel Review work so cost and latency stay predictable.',
    'capacity.maxParallelReviewers.label': 'Parallel checks',
    'capacity.maxParallelReviewers.description': 'Higher values may start more model requests in parallel.',
    'capacity.maxQueueWaitSeconds.label': 'Queue wait',
    'capacity.maxQueueWaitSeconds.description': 'Maximum time Review waits for capacity.',
    'shared:features.deepReview': 'Review',
    'messages.saved': 'Saved',
    'messages.loadFailed': 'Failed to load Review settings.',
    'messages.saveFailed': 'Failed to save Review settings.',
  };
  if (key in translations) return translations[key];
  return key;
});

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: translateMock,
  }),
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
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
  ConfigPageLoading: ({ text }: { text: string }) => <div>{text}</div>,
  NumberInput: ({
    disabled,
    value,
    onChange,
  }: {
    disabled?: boolean;
    value: number;
    onChange: (value: number) => void;
  }) => (
    <input
      type="number"
      disabled={disabled}
      value={value}
      onChange={(event) => onChange(Number(event.currentTarget.value))}
    />
  ),
}));

vi.mock('./common', () => ({
  ConfigPageContent: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ConfigPageHeader: ({ title, subtitle }: { title: string; subtitle: string }) => (
    <header>
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </header>
  ),
  ConfigPageLayout: ({ children }: { children: React.ReactNode }) => <main>{children}</main>,
  ConfigPageRow: ({
    children,
    description,
    label,
  }: {
    children: React.ReactNode;
    description: string;
    label: string;
  }) => (
    <label>
      <span>{label}</span>
      <span>{description}</span>
      {children}
    </label>
  ),
  ConfigPageSection: ({
    children,
    description,
    title,
    titleSuffix,
  }: {
    children: React.ReactNode;
    description?: string;
    title: string;
    titleSuffix?: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      {titleSuffix}
      {description ? <p>{description}</p> : null}
      {children}
    </section>
  ),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useCurrentWorkspace: () => ({ workspacePath: 'D:/workspace/project' }),
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({
    success: notifySuccessMock,
    error: notifyErrorMock,
  }),
}));

vi.mock('@/shared/services/reviewTeamService', () => ({
  REVIEW_STRATEGY_LEVELS: ['quick', 'normal', 'deep'],
  loadDefaultReviewTeam: loadDefaultReviewTeamMock,
  saveDefaultReviewTeamConcurrencyPolicy: saveDefaultReviewTeamConcurrencyPolicyMock,
  saveDefaultReviewTeamStrategyLevel: saveDefaultReviewTeamStrategyLevelMock,
}));

function createReviewTeam() {
  const coreMember = {
    id: 'member-logic',
    subagentId: 'ReviewBusinessLogic',
    displayName: 'Logic Reviewer',
    roleName: 'Business Logic Reviewer',
    definitionKey: 'businessLogic',
    source: 'core',
    locked: true,
    model: 'fast',
    strategyLevel: 'normal',
    strategyOverride: 'inherit',
    strategySource: 'team',
  };
  const extraMember = {
    id: 'member-extra',
    subagentId: 'ReviewDocs',
    displayName: 'Docs Reviewer',
    roleName: 'Documentation Reviewer',
    source: 'extra',
    locked: false,
    model: 'fast',
    strategyLevel: 'normal',
    strategyOverride: 'inherit',
    strategySource: 'team',
  };

  return {
    id: 'default-review-team',
    name: 'Strict Review Coverage',
    description: 'Strict review coverage.',
    warning: 'Review may take longer.',
    strategyLevel: 'normal',
    memberStrategyOverrides: {},
    executionPolicy: {},
    concurrencyPolicy: {
      maxParallelInstances: 2,
      maxQueueWaitSeconds: 300,
      providerCapacityQueueEnabled: true,
      autoRetryEnabled: true,
      autoRetryElapsedGuardSeconds: 120,
    },
    members: [coreMember, extraMember],
    coreMembers: [coreMember],
    extraMembers: [extraMember],
  };
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('ReviewConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.clearAllMocks();

    loadDefaultReviewTeamMock.mockResolvedValue(createReviewTeam());
    saveDefaultReviewTeamStrategyLevelMock.mockResolvedValue(undefined);
    saveDefaultReviewTeamConcurrencyPolicyMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('keeps strict Review settings as the active configuration surface', async () => {
    await act(async () => {
      root.render(<ReviewConfig />);
    });
    await flushPromises();

    expect(container.textContent).toContain('Review');
    expect(container.textContent).toContain('/review');
    expect(container.textContent).toContain('Review depth');
    expect(container.textContent).toContain('Capacity');
    expect(container.textContent).not.toContain('DeepReview');
    expect(container.textContent).not.toContain('Sub-Agent');
    expect(container.textContent).not.toContain('members.title');
    expect(container.textContent).not.toContain('extra.title');
    expect(container.textContent).not.toContain('team management');
    expect(container.textContent).not.toContain('orchestration controls');
  });

  it('saves strategy and capacity changes', async () => {
    await act(async () => {
      root.render(<ReviewConfig />);
    });
    await flushPromises();

    const strictStrategyButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Strict'));
    expect(strictStrategyButton).toBeTruthy();
    await act(async () => {
      strictStrategyButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(saveDefaultReviewTeamStrategyLevelMock).toHaveBeenCalledWith('deep');

    const numberInputs = Array.from(container.querySelectorAll('input[type="number"]'));
    expect(numberInputs).toHaveLength(2);
    await act(async () => {
      const valueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
      valueSetter?.call(numberInputs[0], '4');
      numberInputs[0].dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });
    expect(saveDefaultReviewTeamConcurrencyPolicyMock).toHaveBeenCalledWith(
      expect.objectContaining({ maxParallelInstances: 4 }),
    );
  });
});
