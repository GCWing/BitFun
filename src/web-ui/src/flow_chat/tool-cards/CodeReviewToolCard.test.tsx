import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CodeReviewToolCard } from './CodeReviewToolCard';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';
import type { ReviewTeamManifestMember, ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const flowState = vi.hoisted(() => ({
  current: {
    sessions: new Map<string, unknown>(),
    activeSessionId: null,
  },
  listeners: new Set<(state: { sessions: Map<string, unknown>; activeSessionId: string | null }) => void>(),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const value = typeof options?.defaultValue === 'string' ? options.defaultValue : key;
      return value.replace(/\{\{(\w+)\}\}/g, (_match, name) => String(options?.[name] ?? ''));
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('./CodeReviewReportExportActions', () => ({
  CodeReviewReportExportActions: () => null,
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => flowState.current,
    subscribe: (listener: (state: typeof flowState.current) => void) => {
      flowState.listeners.add(listener);
      return () => flowState.listeners.delete(listener);
    },
  },
  FlowChatStore: {
    getInstance: () => ({
      getState: () => flowState.current,
      subscribe: (listener: (state: typeof flowState.current) => void) => {
        flowState.listeners.add(listener);
        return () => flowState.listeners.delete(listener);
      },
    }),
  },
}));

function buildManifestMember(
  subagentId: string,
  displayName: string,
  source: ReviewTeamManifestMember['source'],
  reason?: ReviewTeamManifestMember['reason'],
): ReviewTeamManifestMember {
  return {
    subagentId,
    displayName,
    roleName: displayName,
    model: 'fast-model',
    configuredModel: 'fast-model',
    defaultModelSlot: 'fast',
    strategyLevel: 'normal',
    strategySource: 'team',
    strategyDirective: 'Review the target.',
    locked: source === 'core',
    source,
    subagentSource: source === 'extra' ? 'user' : 'builtin',
    ...(reason ? { reason } : {}),
  };
}

function buildManifest(): ReviewTeamRunManifest {
  return {
    reviewMode: 'deep',
    workspacePath: 'C:/repo/project',
    policySource: 'default-review-team-config',
    target: {
      source: 'session_files',
      resolution: 'resolved',
      tags: ['frontend'],
      files: ['src/App.tsx'],
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
      skippedReviewerIds: ['CustomInvalid'],
      warnings: [],
    },
    coreReviewers: [
      buildManifestMember('ReviewBusinessLogic', 'Logic reviewer', 'core'),
    ],
    qualityGateReviewer: buildManifestMember('ReviewJudge', 'Quality inspector', 'core'),
    enabledExtraReviewers: [
      buildManifestMember('CustomSecurity', 'Custom security reviewer', 'extra'),
    ],
    skippedReviewers: [
      buildManifestMember('ReviewFrontend', 'Frontend reviewer', 'core', 'not_applicable'),
      buildManifestMember('CustomInvalid', 'Custom invalid reviewer', 'extra', 'invalid_tooling'),
    ],
  };
}

function notifyFlowState(): void {
  for (const listener of flowState.listeners) {
    listener(flowState.current);
  }
}

describe('CodeReviewToolCard', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('navigator', dom.window.navigator);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);

    flowState.current = {
      sessions: new Map<string, unknown>([
        ['review-session', { id: 'review-session', deepReviewRunManifest: buildManifest() }],
      ]),
      activeSessionId: 'review-session',
    };
    flowState.listeners.clear();
    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
    dom.window.close();
  });

  it('echoes the deep review run manifest from the review session', () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      timestamp: Date.now(),
      toolName: 'submit_code_review',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        input: {},
      },
      toolResult: {
        success: true,
        result: {
          review_mode: 'deep',
          summary: {
            overall_assessment: 'No validated issues.',
            risk_level: 'low',
            recommended_action: 'approve',
          },
          issues: [],
          reviewers: [],
        },
      },
    };
    const config: ToolCardConfig = {
      toolName: 'submit_code_review',
      displayName: 'Code Review',
      icon: 'REVIEW',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
    };

    act(() => {
      root.render(
        <CodeReviewToolCard
          toolItem={toolItem}
          config={config}
          sessionId="review-session"
        />,
      );
    });
    act(() => {
      container.querySelector('.preview-toggle-btn')?.dispatchEvent(
        new window.Event('click', { bubbles: true }),
      );
    });

    expect(container.textContent).toContain('Run manifest');
    expect(container.textContent).toContain('3 active');
    expect(container.textContent).toContain('2 skipped');

    const manifestSectionButton = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Run manifest'));

    act(() => {
      manifestSectionButton?.dispatchEvent(new window.Event('click', { bubbles: true }));
    });

    expect(container.textContent).toContain('Logic reviewer');
    expect(container.textContent).toContain('Quality inspector');
    expect(container.textContent).toContain('Custom security reviewer');
    expect(container.textContent).toContain('Frontend reviewer');
    expect(container.textContent).toContain('Not applicable to this target');
    expect(container.textContent).toContain('Custom invalid reviewer');
    expect(container.textContent).toContain('Configuration issue');
    expect(container.textContent).toContain('Recommended strategy');
    expect(container.textContent).toContain('deep');
    expect(container.textContent).toContain('Large/high-risk change');
  });

  it('updates the manifest echo when session metadata arrives after render', () => {
    flowState.current = {
      sessions: new Map<string, unknown>([
        ['review-session', { id: 'review-session' }],
      ]),
      activeSessionId: 'review-session',
    };

    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      timestamp: Date.now(),
      toolName: 'submit_code_review',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        input: {},
      },
      toolResult: {
        success: true,
        result: {
          review_mode: 'deep',
          summary: {
            overall_assessment: 'No validated issues.',
            risk_level: 'low',
            recommended_action: 'approve',
          },
          issues: [],
          reviewers: [],
        },
      },
    };
    const config: ToolCardConfig = {
      toolName: 'submit_code_review',
      displayName: 'Code Review',
      icon: 'REVIEW',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
    };

    act(() => {
      root.render(
        <CodeReviewToolCard
          toolItem={toolItem}
          config={config}
          sessionId="review-session"
        />,
      );
    });
    act(() => {
      container.querySelector('.preview-toggle-btn')?.dispatchEvent(
        new window.Event('click', { bubbles: true }),
      );
    });

    expect(container.textContent).not.toContain('Run manifest');

    act(() => {
      flowState.current = {
        sessions: new Map<string, unknown>([
          ['review-session', { id: 'review-session', deepReviewRunManifest: buildManifest() }],
        ]),
        activeSessionId: 'review-session',
      };
      notifyFlowState();
    });

    expect(container.textContent).toContain('Run manifest');
    expect(container.textContent).toContain('3 active');
  });

  it('renders compact reliability status when a reviewer timed out with partial evidence', () => {
    const toolItem: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      timestamp: Date.now(),
      toolName: 'submit_code_review',
      status: 'completed',
      toolCall: {
        id: 'call-1',
        input: {},
      },
      toolResult: {
        success: true,
        result: {
          review_mode: 'deep',
          summary: {
            overall_assessment: 'Review completed with reduced confidence.',
            risk_level: 'medium',
            recommended_action: 'request_changes',
          },
          issues: [],
          reviewers: [
            {
              name: 'Security Reviewer',
              specialty: 'security',
              status: 'partial_timeout',
              summary: 'Timed out after producing partial evidence.',
              partial_output: 'Found likely token logging in src/auth.ts before timeout.',
            },
          ],
        },
      },
    };
    const config: ToolCardConfig = {
      toolName: 'submit_code_review',
      displayName: 'Code Review',
      icon: 'REVIEW',
      requiresConfirmation: false,
      resultDisplayType: 'detailed',
    };

    act(() => {
      root.render(
        <CodeReviewToolCard
          toolItem={toolItem}
          config={config}
          sessionId="review-session"
        />,
      );
    });
    act(() => {
      container.querySelector('.preview-toggle-btn')?.dispatchEvent(
        new window.Event('click', { bubbles: true }),
      );
    });

    expect(container.textContent).toContain('Review status');
    expect(container.textContent).toContain('Reviewer timed out with partial result');
    expect(container.textContent).toContain('1 reviewer result is partial; confidence is reduced.');
  });
});
