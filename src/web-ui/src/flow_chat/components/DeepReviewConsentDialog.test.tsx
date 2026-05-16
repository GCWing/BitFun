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
  launchContext,
  onResult,
}: {
  preview?: ReviewTeamRunManifest;
  launchContext?: unknown;
  onResult: (confirmed: boolean) => void;
}) {
  const { confirmDeepReviewLaunch, deepReviewConsentDialog } = useDeepReviewConsent();

  return (
    <>
      <button
        onClick={async () => {
          onResult(await (confirmDeepReviewLaunch as (...args: unknown[]) => Promise<boolean>)(
            preview,
            launchContext,
          ));
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
    scopeProfile: {
      reviewDepth: 'risk_expanded',
      riskFocusTags: ['security', 'cross_boundary_api_contracts'],
      maxDependencyHops: 1,
      optionalReviewerPolicy: 'configured',
      allowBroadToolExploration: false,
      coverageExpectation: 'Risk-expanded pass; changed files remain visible.',
    },
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
      reviewerTimeoutSeconds: 1800,
      judgeTimeoutSeconds: 1200,
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

function buildPreviewWithoutSkippedReviewers(): ReviewTeamRunManifest {
  return {
    ...buildPreview(),
    skippedReviewers: [],
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

  it('shows a compact launch summary with skipped reviewers only when needed', async () => {
    const result = vi.fn();

    await act(async () => {
      root.render(<Harness preview={buildPreview()} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Launch summary');
    expect(container.textContent).toContain('1 file');
    expect(container.textContent).toContain('2 skipped');
    expect(container.textContent).toContain('Run strategy: Normal');
    expect(container.textContent).not.toContain('Risk areas: Backend core');
    expect(container.textContent).not.toContain('3 reviewer calls');
    expect(container.textContent).not.toContain('1 extra specialist');
    expect(container.textContent).not.toContain('Review depth: Risk-expanded');
    expect(container.textContent).toContain('Frontend reviewer');
    expect(container.textContent).toContain('Not applicable to this target');
    expect(container.textContent).toContain('Custom invalid reviewer');
    expect(container.textContent).toContain('Configuration issue');
    expect(container.textContent).not.toContain('Logic reviewer');
    expect(container.textContent).not.toContain('Custom security reviewer');
  });

  it('uses a generic target summary when the review is not file-based', async () => {
    const result = vi.fn();
    const preview: ReviewTeamRunManifest = {
      ...buildPreview(),
      target: {
        source: 'manual_prompt',
        resolution: 'unknown',
        tags: ['unknown'],
        files: [],
        evidence: ['manual prompt'],
        warnings: [{
          code: 'target_unknown',
          message: 'Manual prompt target',
        }],
      },
      skippedReviewers: [],
    };

    await act(async () => {
      root.render(<Harness preview={preview} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Provided context');
    expect(container.textContent).not.toContain('0 files');
    expect(container.textContent).not.toContain('Risk areas:');
    expect(container.textContent).not.toContain('reviewer calls');
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

  it('still opens when skip preference is set but the active session is busy', async () => {
    localStorage.setItem('bitfun.deepReview.skipCostConfirmation', 'true');
    const result = vi.fn();

    await act(async () => {
      root.render(
        <Harness
          preview={buildPreviewWithoutSkippedReviewers()}
          launchContext={{
            sessionConcurrencyGuard: {
              activeSubagentCount: 2,
              highActivity: true,
            },
          }}
          onResult={result}
        />,
      );
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.querySelector('[role="dialog"]')).not.toBeNull();
    expect(container.textContent).toContain('Active session is busy');
    expect(container.textContent).toContain('2 running subagent tasks');
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
      .find((button) => button.textContent?.includes('Deep'));
    expect(deepStrategyButton).not.toBeUndefined();

    await act(async () => {
      deepStrategyButton?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });
    await act(async () => {
      Array.from(container.querySelectorAll('button'))
        .find((button) => button.textContent === 'Start Deep Review')
        ?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(mockSaveReviewTeamProjectStrategyOverride).toHaveBeenCalledWith(
      '/test-fixtures/project-a',
      'deep',
    );
    expect(result).toHaveBeenCalledWith(true);
  });

  it('keeps the launch dialog sparse and makes the selected strategy prominent', async () => {
    const result = vi.fn();

    await act(async () => {
      root.render(<Harness preview={buildPreview()} onResult={result} />);
    });
    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.querySelectorAll('.deep-review-consent__priority-grid')).toHaveLength(0);
    expect(container.querySelectorAll('.deep-review-consent__priority-point')).toHaveLength(0);
    expect(container.querySelectorAll('.deep-review-consent__strategy-heading')).toHaveLength(0);
    expect(container.textContent).not.toContain('Quick is narrower');
    expect(container.textContent).not.toContain('Risk areas: Backend core');
    expect(container.textContent).not.toContain('3 reviewer calls');
    expect(container.textContent).not.toContain('1 extra specialist');
    expect(container.textContent).not.toContain('Expected cost:');
    expect(container.querySelectorAll('.deep-review-consent__strategy-selected-summary')).toHaveLength(0);
    expect(container.querySelectorAll('.deep-review-consent__strategy-current')).toHaveLength(1);
    expect(container.querySelectorAll('.deep-review-consent__strategy-option')).toHaveLength(3);
    expect(container.querySelectorAll('.deep-review-consent__strategy-option--active')).toHaveLength(1);
    expect(container.textContent).not.toContain('Team default');
    expect(container.textContent).toContain('Selected');
    expect(container.textContent).toContain('Token: 1x');
    expect(container.textContent).toContain('Time: 1x');
    expect(container.textContent).toContain('Normal stays practical for slower models');
    expect(container.querySelectorAll('.deep-review-consent__strategy-option-summary')).toHaveLength(0);

    const quickStrategyButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Quick'));
    expect(quickStrategyButton).not.toBeUndefined();

    await act(async () => {
      quickStrategyButton?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(quickStrategyButton?.getAttribute('aria-pressed')).toBe('true');
    expect(quickStrategyButton?.className).toContain('deep-review-consent__strategy-option--active');
    expect(container.textContent).toContain('Run strategy: Quick');
    expect(container.textContent).toContain('Token: 0.4-0.6x');
    expect(container.textContent).toContain('Time: 0.5-0.7x');
    expect(container.textContent).toContain('Quick keeps built-in target-matched reviewers');
  });
});
