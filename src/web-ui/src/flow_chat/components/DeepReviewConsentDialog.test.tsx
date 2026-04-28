import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useDeepReviewConsent } from './DeepReviewConsentDialog';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

const mockSaveReviewTeamProjectStrategyOverride = vi.hoisted(() => vi.fn());

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown>) => {
      const value = typeof options?.defaultValue === 'string' ? options.defaultValue : _key;
      return value.replace(/\{\{(\w+)\}\}/g, (_match, key) => String(options?.[key] ?? ''));
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    onClick,
  }: {
    children: React.ReactNode;
    onClick?: () => void;
  }) => <button onClick={onClick}>{children}</button>,
  Checkbox: ({
    checked,
    label,
    onChange,
  }: {
    checked: boolean;
    label: string;
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  }) => (
    <label>
      <input
        type="checkbox"
        checked={checked}
        onChange={onChange}
      />
      {label}
    </label>
  ),
  Modal: ({
    children,
    isOpen,
  }: {
    children: React.ReactNode;
    isOpen: boolean;
  }) => (isOpen ? <div role="dialog">{children}</div> : null),
}));

vi.mock('@/shared/services/reviewTeamService', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/shared/services/reviewTeamService')>();
  return {
    ...actual,
    saveReviewTeamProjectStrategyOverride: (
      ...args: Parameters<typeof actual.saveReviewTeamProjectStrategyOverride>
    ) => mockSaveReviewTeamProjectStrategyOverride(...args),
  };
});

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

function Harness({
  preview,
  onResult,
}: {
  preview?: ReviewTeamRunManifest;
  onResult: (confirmed: boolean) => void;
}) {
  const { confirmDeepReviewLaunch, deepReviewConsentDialog } = useDeepReviewConsent();

  return (
    <>
      <button
        onClick={async () => {
          onResult(await confirmDeepReviewLaunch(preview));
        }}
      >
        Open consent
      </button>
      {deepReviewConsentDialog}
    </>
  );
}

function buildPreview(): ReviewTeamRunManifest {
  return {
    reviewMode: 'deep',
    workspacePath: '/test-fixtures/project-a',
    policySource: 'default-review-team-config',
    target: {
      source: 'session_files',
      resolution: 'resolved',
      tags: ['backend_core'],
      files: ['src/crates/core/src/service/config/types.rs'],
      warnings: [],
    },
    strategyLevel: 'normal',
    strategyRecommendation: {
      strategyLevel: 'deep',
      score: 24,
      rationale: 'Large/high-risk change (8 files, 900 lines; 2 security-sensitive files, 3 workspace areas). Deep review recommended.',
      factors: {
        fileCount: 8,
        totalLinesChanged: 900,
        lineCountSource: 'diff_stat',
        securityFileCount: 2,
        workspaceAreaCount: 3,
        contractSurfaceChanged: true,
      },
    },
    executionPolicy: {
      reviewerTimeoutSeconds: 300,
      judgeTimeoutSeconds: 240,
      reviewerFileSplitThreshold: 20,
      maxSameRoleInstances: 3,
    },
    tokenBudget: {
      mode: 'balanced',
      estimatedReviewerCalls: 3,
      maxReviewerCalls: 4,
      maxExtraReviewers: 1,
      largeDiffSummaryFirst: false,
      skippedReviewerIds: [],
      warnings: [],
    },
    coreReviewers: [
      {
        subagentId: 'ReviewBusinessLogic',
        displayName: 'Logic reviewer',
        roleName: 'Business Logic Reviewer',
        model: 'fast',
        configuredModel: 'fast',
        defaultModelSlot: 'fast',
        strategyLevel: 'normal',
        strategySource: 'team',
        strategyDirective: 'Review logic.',
        locked: true,
        source: 'core',
        subagentSource: 'builtin',
      },
    ],
    qualityGateReviewer: {
      subagentId: 'ReviewJudge',
      displayName: 'Quality inspector',
      roleName: 'Review Quality Inspector',
      model: 'fast',
      configuredModel: 'fast',
      defaultModelSlot: 'fast',
      strategyLevel: 'normal',
      strategySource: 'team',
      strategyDirective: 'Check report quality.',
      locked: true,
      source: 'core',
      subagentSource: 'builtin',
    },
    enabledExtraReviewers: [
      {
        subagentId: 'CustomSecurity',
        displayName: 'Custom security reviewer',
        roleName: 'Additional Specialist Reviewer',
        model: 'fast',
        configuredModel: 'fast',
        defaultModelSlot: 'fast',
        strategyLevel: 'normal',
        strategySource: 'team',
        strategyDirective: 'Review security.',
        locked: false,
        source: 'extra',
        subagentSource: 'user',
      },
    ],
    skippedReviewers: [
      {
        subagentId: 'ReviewFrontend',
        displayName: 'Frontend reviewer',
        roleName: 'Frontend Reviewer',
        model: 'fast',
        configuredModel: 'fast',
        defaultModelSlot: 'fast',
        strategyLevel: 'normal',
        strategySource: 'team',
        strategyDirective: 'Review frontend.',
        locked: true,
        source: 'core',
        subagentSource: 'builtin',
        reason: 'not_applicable',
      },
      {
        subagentId: 'CustomInvalid',
        displayName: 'Custom invalid reviewer',
        roleName: 'Additional Specialist Reviewer',
        model: 'fast',
        configuredModel: 'fast',
        defaultModelSlot: 'fast',
        strategyLevel: 'normal',
        strategySource: 'team',
        strategyDirective: 'Review custom rules.',
        locked: false,
        source: 'extra',
        subagentSource: 'user',
        reason: 'invalid_tooling',
      },
    ],
  };
}

describeWithJsdom('DeepReviewConsentDialog', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mockSaveReviewTeamProjectStrategyOverride.mockResolvedValue(undefined);
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });

    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('Event', window.Event);
    vi.stubGlobal('localStorage', window.localStorage);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
  });

  it('shows active and skipped reviewers from the manifest preview', async () => {
    const result = vi.fn();

    await act(async () => {
      root.render(<Harness preview={buildPreview()} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Review lineup');
    expect(container.textContent).toContain('3 reviewer calls');
    expect(container.textContent).toContain('Recommended strategy: deep');
    expect(container.textContent).toContain('Large/high-risk change');
    expect(container.textContent).toContain('Logic reviewer');
    expect(container.textContent).toContain('Custom security reviewer');
    expect(container.textContent).toContain('Frontend reviewer');
    expect(container.textContent).toContain('Not applicable to this target');
    expect(container.textContent).toContain('Custom invalid reviewer');
    expect(container.textContent).toContain('Configuration issue');
  });

  it('still opens when skip preference is set but reviewers are skipped', async () => {
    localStorage.setItem('bitfun.deepReview.skipCostConfirmation', 'true');
    const result = vi.fn();

    await act(async () => {
      root.render(<Harness preview={buildPreview()} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.querySelector('[role="dialog"]')).not.toBeNull();
    expect(result).not.toHaveBeenCalled();
  });

  it('persists a selected project strategy override before confirming', async () => {
    const result = vi.fn();

    await act(async () => {
      root.render(<Harness preview={buildPreview()} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    const deepStrategyButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent === 'Deep');
    expect(deepStrategyButton).not.toBeUndefined();

    await act(async () => {
      deepStrategyButton?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });
    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find((button) => button.textContent === 'deepReviewConsent.confirm')
        ?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(mockSaveReviewTeamProjectStrategyOverride).toHaveBeenCalledWith(
      '/test-fixtures/project-a',
      'deep',
    );
    expect(result).toHaveBeenCalledWith(true);
  });
});
