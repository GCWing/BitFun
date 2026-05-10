import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import enFlowChat from '@/locales/en-US/flow-chat.json';
import zhCnFlowChat from '@/locales/zh-CN/flow-chat.json';
import zhTwFlowChat from '@/locales/zh-TW/flow-chat.json';
import { SessionRuntimeStatusEntry } from './SessionRuntimeStatusEntry';
import { SessionUsagePanel } from './SessionUsagePanel';
import { SessionUsageReportCard } from './SessionUsageReportCard';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const labels: Record<string, string> = {
        'usage.title': 'Session Usage',
        'usage.unavailable': 'Unavailable',
        'usage.redacted': 'Redacted',
        'usage.percent': '{{value}}%',
        'usage.duration.ms': '{{value}}ms',
        'usage.duration.seconds': '{{value}}s',
        'usage.duration.minutes': '{{value}}m',
        'usage.duration.minutesSeconds': '{{minutes}}m {{seconds}}s',
        'usage.duration.hours': '{{value}}h',
        'usage.duration.hoursMinutes': '{{hours}}h {{minutes}}m',
        'usage.actions.copyMarkdown': 'Copy Markdown',
        'usage.actions.copied': 'Copied',
        'usage.actions.copySessionId': 'Copy session ID',
        'usage.actions.copyWorkspacePath': 'Copy project path',
        'usage.actions.openDetails': 'Open details',
        'usage.coverage.complete': 'Complete',
        'usage.coverage.partial': 'Partial',
        'usage.coverage.minimal': 'Minimal',
        'usage.coverage.partialNotice': 'Some metrics were not reported by this session or provider. Hover underlined values for the specific reason.',
        'usage.toolCategories.git': 'Git',
        'usage.toolCategories.shell': 'Shell',
        'usage.toolCategories.file': 'File',
        'usage.toolCategories.other': 'Other',
        'usage.fileScopes.snapshot_summary': 'Snapshot summary',
        'usage.fileScopes.tool_inputs_only': 'Tool inputs only',
        'usage.fileScopes.unavailable': 'Not tracked',
        'usage.accounting.approximate': 'Approximate',
        'usage.accounting.exact': 'Exact',
        'usage.accounting.unavailable': 'Unavailable',
        'usage.cacheCoverage.available': 'Reported',
        'usage.cacheCoverage.partial': 'Partially reported',
        'usage.cacheCoverage.unavailable': 'Not reported',
        'usage.status.timingNotRecorded': 'Timing not recorded',
        'usage.status.cacheNotReported': 'Cache not reported',
        'usage.status.noFileChanges': 'No file changes',
        'usage.status.notRecorded': 'Not recorded',
        'usage.card.eyebrow': 'Local report',
        'usage.card.turns': '{{count}} turns',
        'usage.card.calls': '{{count}} calls',
        'usage.card.operations': '{{count}} ops',
        'usage.loading.title': 'Generating usage report',
        'usage.loading.description': 'Reading local session records and preparing a privacy-safe summary.',
        'usage.loading.steps.collecting': 'Reading session records',
        'usage.loading.steps.tokens': 'Summarizing token and tool activity',
        'usage.loading.steps.safety': 'Checking privacy-safe display fields',
        'usage.metrics.wall': 'Session span',
        'usage.metrics.active': 'Recorded turn time',
        'usage.metrics.modelTime': 'Model round time',
        'usage.metrics.toolTime': 'Tool call time',
        'usage.metrics.tokens': 'Tokens',
        'usage.metrics.cached': 'Cached',
        'usage.metrics.files': 'Files',
        'usage.metrics.errors': 'Errors',
        'usage.sections.models': 'Models',
        'usage.sections.tools': 'Tools',
        'usage.sections.files': 'Files',
        'usage.empty.models': 'No model metrics',
        'usage.empty.modelsDescription': 'Model rows appear after calls report token usage.',
        'usage.empty.tools': 'No tool metrics',
        'usage.empty.toolsDescription': 'Tool rows appear after the session runs tools.',
        'usage.empty.files': 'No file changes',
        'usage.empty.filesDescription': 'No file-edit records were found for this session.',
        'usage.empty.errors': 'No error examples',
        'usage.empty.errorsDescription': 'No sampled tool or model errors were recorded.',
        'usage.help.wall': 'Span from the first recorded turn start to the last recorded turn end. Idle gaps can be included.',
        'usage.help.active': 'Sum of recorded turn durations that produced reportable activity. It can include orchestration or waiting inside a turn.',
        'usage.help.timeShare': 'Share of recorded turn time. Model and tool spans may overlap, so this is only an approximate indicator.',
        'usage.help.modelRoundTime': 'Recorded model-round duration from persisted start and end timestamps, not pure model streaming or throughput time. The percentage uses recorded turn time and is approximate.',
        'usage.help.toolTime': 'Recorded tool-call duration. The percentage uses recorded turn time and is approximate.',
        'usage.help.cachedTokens': 'The provider did not report cache-read token metadata for this session. Total token counts are still shown when available.',
        'usage.help.cachedTokensPartial': 'Only some calls reported cache-read token metadata, so the cached-token total covers those calls only.',
        'usage.help.filesUnavailable': 'No file snapshot or file-edit tool record was found for this session.',
        'usage.help.filesNoRecordedChanges': 'BitFun did not detect file changes in this session. This is expected when the agent did not edit files.',
        'usage.help.filesRemoteUnavailable': 'Remote session file snapshots are not included in this report yet. File rows appear only when tool records identify edited files.',
        'usage.help.filesNotTracked': 'No local snapshot or identifiable file-edit tool record was found for this session.',
        'usage.meta.generatedAt': 'Generated',
        'usage.meta.sessionId': 'Session ID',
        'usage.meta.workspacePath': 'Project path',
        'usage.runtime.open': 'Generate session usage',
        'usage.runtime.button': 'Usage',
        'usage.runtime.tooltip': 'Generate a usage report in this chat',
        'usage.panel.tabsLabel': 'Usage report sections',
        'usage.panel.accounting': 'Accounting',
        'usage.panel.turnScope': 'Scope',
        'usage.panel.cacheCoverage': 'Cache reporting',
        'usage.panel.compressions': 'Compressions',
        'usage.panel.fileScope': 'File scope',
        'usage.panel.toolErrors': 'Tool errors',
        'usage.panel.modelErrors': 'Model errors',
        'usage.privacy.title': 'Privacy-safe report',
        'usage.privacy.summary': 'Prompts, tool inputs, command outputs, and file contents are not included.',
        'usage.tabs.overview': 'Overview',
        'usage.tabs.models': 'Models',
        'usage.tabs.tools': 'Tools',
        'usage.tabs.files': 'Files',
        'usage.tabs.errors': 'Errors',
        'usage.table.model': 'Model',
        'usage.table.tool': 'Tool',
        'usage.table.category': 'Category',
        'usage.table.calls': 'Calls',
        'usage.table.success': 'Success',
        'usage.table.errors': 'Errors',
        'usage.table.input': 'Input',
        'usage.table.output': 'Output',
        'usage.table.cached': 'Cached',
        'usage.table.duration': 'Recorded time',
        'usage.table.p95': 'P95',
        'usage.table.execution': 'Execution',
        'usage.table.file': 'File',
        'usage.table.operations': 'Ops',
        'usage.table.added': 'Added',
        'usage.table.deleted': 'Deleted',
        'usage.table.turns': 'Turns',
        'usage.table.operationIds': 'Operation IDs',
        'usage.table.label': 'Label',
        'usage.table.count': 'Count',
      };
      return interpolate(labels[key] ?? key, options);
    },
  }),
}));

vi.mock('@/component-library', () => ({
  IconButton: ({
    children,
    variant: _variant,
    size: _size,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: string; size?: string }) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
  MarkdownRenderer: ({ content }: { content: string }) => <div data-testid="markdown">{content}</div>,
  Tooltip: ({ children, content }: { children: React.ReactNode; content?: React.ReactNode }) => (
    <span data-tooltip={typeof content === 'string' ? content : undefined}>{children}</span>
  ),
  ToolProcessingDots: ({ className }: { className?: string }) => <span className={className}>...</span>,
}));

function interpolate(template: string, options?: Record<string, unknown>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_match, key) => String(options?.[key] ?? ''));
}

function usageReport(overrides: Partial<SessionUsageReport> = {}): SessionUsageReport {
  return {
    schemaVersion: 1,
    reportId: 'usage-session-1',
    sessionId: 'session-1',
    generatedAt: Date.UTC(2026, 4, 10, 8, 0),
    workspace: {
      kind: 'local',
      pathLabel: 'D:/workspace/bitfun',
    },
    scope: {
      kind: 'entire_session',
      turnCount: 3,
      includesSubagents: false,
    },
    coverage: {
      level: 'partial',
      available: ['workspace_identity'],
      missing: ['cost_estimates'],
      notes: [],
    },
    time: {
      accounting: 'approximate',
      denominator: 'session_wall_time',
      wallTimeMs: 120_000,
      activeTurnMs: 80_000,
      modelMs: 40_000,
      toolMs: 20_000,
    },
    tokens: {
      source: 'token_usage_records',
      inputTokens: 1200,
      outputTokens: 300,
      totalTokens: 1500,
      cacheCoverage: 'unavailable',
    },
    models: [
      {
        modelId: 'gpt-5.4',
        callCount: 2,
        inputTokens: 1200,
        outputTokens: 300,
        totalTokens: 1500,
        durationMs: 40_000,
      },
    ],
    tools: [
      {
        toolName: 'secret shell command output',
        category: 'shell',
        callCount: 2,
        successCount: 1,
        errorCount: 1,
        durationMs: 20_000,
        p95DurationMs: 18_000,
        executionMs: 16_000,
        redacted: true,
      },
    ],
    files: {
      scope: 'snapshot_summary',
      changedFiles: 1,
      addedLines: 4,
      deletedLines: 2,
      files: [
        {
          pathLabel: 'secrets/raw-file-content.txt',
          operationCount: 2,
          addedLines: 4,
          deletedLines: 2,
          turnIndexes: [1],
          operationIds: ['operation-1'],
          redacted: true,
        },
      ],
    },
    compression: {
      compactionCount: 2,
      manualCompactionCount: 1,
      automaticCompactionCount: 1,
    },
    errors: {
      totalErrors: 1,
      toolErrors: 1,
      modelErrors: 0,
      examples: [
        {
          label: 'raw provider error with secret payload',
          count: 1,
          redacted: true,
        },
      ],
    },
    slowest: [],
    privacy: {
      promptContentIncluded: false,
      toolInputsIncluded: false,
      commandOutputsIncluded: false,
      fileContentsIncluded: false,
      redactedFields: ['tools', 'files', 'errors'],
    },
    ...overrides,
  };
}

function collectLeafPaths(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }
  return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
    collectLeafPaths(child, prefix ? `${prefix}.${key}` : key)
  );
}

function resolvePath(value: unknown, dottedPath: string): unknown {
  return dottedPath.split('.').reduce<unknown>((current, segment) => {
    if (!current || typeof current !== 'object') {
      return undefined;
    }
    return (current as Record<string, unknown>)[segment];
  }, value);
}

function flattenStrings(value: unknown): string[] {
  if (typeof value === 'string') {
    return [value];
  }
  if (!value || typeof value !== 'object') {
    return [];
  }
  return Object.values(value as Record<string, unknown>).flatMap(flattenStrings);
}

describe('Session usage report UI components', () => {
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
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: vi.fn(),
      },
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  const render = (element: React.ReactElement) => {
    act(() => {
      root.render(element);
    });
  };

  it('renders localized partial coverage and cache unavailable without showing zero', () => {
    const onOpenDetails = vi.fn();
    const report = usageReport();

    render(
      <SessionUsageReportCard
        report={report}
        markdown="## Session Usage"
        onOpenDetails={onOpenDetails}
      />
    );

    const cachedMetric = Array.from(container.querySelectorAll('.session-usage-report-card__metric'))
      .find(metric => metric.textContent?.includes('Cached'));
    expect(container.textContent).toContain('Partial');
    expect(container.textContent).toContain('Hover underlined values');
    expect(cachedMetric?.textContent).toContain('Cache not reported');
    expect(cachedMetric?.textContent).not.toMatch(/Cached\s*0/);
    expect(cachedMetric?.querySelector('[data-tooltip]')?.getAttribute('data-tooltip'))
      .toContain('Total token counts are still shown');
    expect(cachedMetric?.querySelector('.session-usage-report-card__metric-value--help')?.hasAttribute('title'))
      .toBe(false);

    const openButton = container.querySelector('button[aria-label="Open details"]');
    act(() => {
      openButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(onOpenDetails).toHaveBeenCalledWith(report);
  });

  it('shows an immediate usage loading card before report data exists', () => {
    render(
      <SessionUsageReportCard
        isLoading
        markdown="Generating..."
      />
    );

    expect(container.querySelector('.session-usage-report-card--loading')).not.toBeNull();
    expect(container.textContent).toContain('Generating usage report');
    expect(container.textContent).toContain('Reading local session records');
    expect(container.textContent).not.toContain('Unknown values are not counted as zero');
  });

  it('switches panel sections and keeps raw sensitive details redacted', () => {
    render(<SessionUsagePanel report={usageReport()} markdown="## Session Usage" />);

    for (const tab of ['Models', 'Tools', 'Files', 'Errors']) {
      const tabButton = Array.from(container.querySelectorAll('.session-usage-panel__tab'))
        .find(button => button.textContent === tab);
      act(() => {
        tabButton?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
      });
      expect(container.textContent).toContain(tab);
    }

    expect(container.textContent).toContain('Redacted');
    expect(container.textContent).not.toContain('secret shell command output');
    expect(container.textContent).not.toContain('secrets/raw-file-content.txt');
    expect(container.textContent).not.toContain('raw provider error with secret payload');
  });

  it('renders the runtime status entry as a lightweight usage trigger', () => {
    const onOpen = vi.fn();

    render(<SessionRuntimeStatusEntry onOpen={onOpen} />);

    expect(container.querySelector('.session-runtime-status-entry')?.textContent).toContain('Usage');
    expect(container.querySelector('[data-tooltip]')?.getAttribute('data-tooltip')).toBe('Generate a usage report in this chat');
    expect(container.textContent).not.toContain('1500 tokens');
    expect(container.textContent).not.toContain('tool calls');
    expect(container.textContent).not.toContain('files');
    expect(container.textContent).not.toContain('50%');
    expect(container.textContent).not.toContain('25%');

    act(() => {
      container.querySelector('button')?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(onOpen).toHaveBeenCalledTimes(1);

    render(<SessionRuntimeStatusEntry />);
    expect(container.querySelector('.session-runtime-status-entry')).toBeNull();
  });

  it('shows copyable detail metadata and explains unavailable model/file metrics', async () => {
    const report = usageReport({
      models: [
        {
          modelId: 'glm-5.1',
          callCount: 1,
          inputTokens: 421_000,
          outputTokens: 959,
          totalTokens: 421_959,
          durationMs: undefined,
        },
      ],
      files: {
        scope: 'unavailable',
        changedFiles: undefined,
        addedLines: undefined,
        deletedLines: undefined,
        files: [],
      },
    });

    render(
      <SessionUsagePanel
        report={report}
        markdown="## Session Usage"
        sessionId="session-1"
        workspacePath="D:/workspace/bitfun"
      />
    );

    expect(container.querySelectorAll('.session-usage-panel__meta-row')).toHaveLength(3);
    const cacheCoverageHelp = Array.from(container.querySelectorAll('[data-tooltip]'))
      .find(node => node.getAttribute('data-tooltip')?.includes('Total token counts are still shown'));
    expect(cacheCoverageHelp?.textContent).toContain('Not reported');

    const sessionCopy = container.querySelector('button[aria-label="Copy session ID"]');
    await act(async () => {
      sessionCopy?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('session-1');

    const modelTab = Array.from(container.querySelectorAll('.session-usage-panel__tab'))
      .find(button => button.textContent === 'Models');
    act(() => {
      modelTab?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(container.textContent).toContain('glm-5.1');
    expect(container.textContent).not.toContain('Timing not recorded');

    const filesTab = Array.from(container.querySelectorAll('.session-usage-panel__tab'))
      .find(button => button.textContent === 'Files');
    act(() => {
      filesTab?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });
    expect(container.textContent).toContain('No file changes');
    const fileUnavailableHelp = Array.from(container.querySelectorAll('[data-tooltip]'))
      .find(node => node.getAttribute('data-tooltip')?.includes('did not detect file changes'));
    expect(fileUnavailableHelp).toBeTruthy();
  });
});

describe('Session usage report i18n and theme guards', () => {
  it('keeps usage locale keys aligned across English, Simplified Chinese, and Traditional Chinese', () => {
    const enUsage = enFlowChat.usage;
    const zhCnUsage = zhCnFlowChat.usage;
    const zhTwUsage = zhTwFlowChat.usage;

    for (const key of collectLeafPaths(enUsage)) {
      expect(resolvePath(zhCnUsage, key), `zh-CN missing usage.${key}`).not.toBeUndefined();
      expect(resolvePath(zhTwUsage, key), `zh-TW missing usage.${key}`).not.toBeUndefined();
    }
  });

  it('localizes the /usage command text in all flow chat locales', () => {
    const keys = [
      'usageAction',
      'usageNoSession',
      'usageCommandUsage',
      'usageBusy',
      'usageNoWorkspace',
      'usageFailed',
    ];

    for (const key of keys) {
      expect(enFlowChat.chatInput[key as keyof typeof enFlowChat.chatInput], `en-US missing chatInput.${key}`)
        .toEqual(expect.any(String));
      expect(zhCnFlowChat.chatInput[key as keyof typeof zhCnFlowChat.chatInput], `zh-CN missing chatInput.${key}`)
        .toEqual(expect.any(String));
      expect(zhTwFlowChat.chatInput[key as keyof typeof zhTwFlowChat.chatInput], `zh-TW missing chatInput.${key}`)
        .toEqual(expect.any(String));
    }
  });

  it('keeps usage copy token-only without billing or package language', () => {
    const usageCopy = [
      ...flattenStrings(enFlowChat.usage),
      ...flattenStrings(zhCnFlowChat.usage),
      ...flattenStrings(zhTwFlowChat.usage),
    ].join('\n');

    expect(usageCopy).not.toMatch(/\b(cost|price|billing|currency|invoice|package|subscription|usd|cny|rmb)\b/i);
    expect(usageCopy).not.toMatch(/[$\u00a5\u20ac]/);
  });

  it('keeps usage styles on semantic theme colors', () => {
    const usageStylePaths = [
      'src/flow_chat/components/usage/SessionUsageReportCard.scss',
      'src/flow_chat/components/usage/SessionUsagePanel.scss',
      'src/flow_chat/components/usage/SessionRuntimeStatusEntry.scss',
    ];
    const styleText = usageStylePaths
      .map(stylePath => fs.readFileSync(path.resolve(stylePath), 'utf8'))
      .join('\n');

    expect(styleText).toContain('var(--color-text-primary)');
    expect(styleText).toContain('width: auto;');
    expect(styleText).toContain('margin: 0.12rem 3rem');
    expect(styleText).toContain('border: 1px solid color-mix(in srgb, var(--border-base)');
    expect(styleText).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i);
  });
});
