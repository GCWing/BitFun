// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import UsageStatisticsConfig from './UsageStatisticsConfig';
import type { UsageStatistics } from '@/infrastructure/api';

const getStatisticsMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => vi.fn((key: string) => key));
const TokenUsageStatisticsUnavailableErrorMock = vi.hoisted(() => class extends Error {
  constructor() {
    super('Usage statistics are not supported by the active host');
    this.name = 'TokenUsageStatisticsUnavailableError';
  }
});

vi.mock('@/infrastructure/api', () => ({
  TokenUsageStatisticsUnavailableError: TokenUsageStatisticsUnavailableErrorMock,
  tokenUsageStatisticsApi: {
    getStatistics: getStatisticsMock,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: translateMock,
    formatDate: (date: Date | number) => new Date(date).toISOString(),
    formatNumber: (value: number, options?: Intl.NumberFormatOptions) => (
      new Intl.NumberFormat('en-US', options).format(value)
    ),
    resolvedTimeZone: 'UTC',
  }),
}));

vi.mock('@openbitfun/ui', async importOriginal => ({
  ...await importOriginal<typeof import('@openbitfun/ui')>(),
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  FormSection: ({
    children,
    title,
    ...props
  }: React.HTMLAttributes<HTMLElement> & { title?: React.ReactNode }) => (
    <section {...props}>{title}{children}</section>
  ),
  FieldGroup: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  Tooltip: ({ children }: React.PropsWithChildren) => <>{children}</>,
  IconButton: ({
    children,
    icon,
    tooltip: _tooltip,
    size: _size,
    variant: _variant,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    icon?: React.ReactNode;
    tooltip?: React.ReactNode;
    size?: string;
    variant?: string;
  }) => <button {...props}>{icon ?? children}</button>,
  Input: ({
    leading,
    trailing,
    ...props
  }: React.InputHTMLAttributes<HTMLInputElement> & {
    leading?: React.ReactNode;
    trailing?: React.ReactNode;
  }) => (
    <div>
      {leading}
      <input {...props} />
      {trailing}
    </div>
  ),
  Select: ({
    value,
    options,
    onValueChange,
  }: {
    value: string | number;
    options: { value: string | number; label: string }[];
    onValueChange?: (value: string) => void;
  }) => (
    <select
      data-testid="usage-select"
      value={String(value)}
      onChange={(event) => onValueChange?.(event.target.value)}
    >
      {options.map((option) => (
        <option key={String(option.value)} value={String(option.value)}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('./common', async importOriginal => ({
  ...await importOriginal<typeof import('./common')>(),
  ConfigLoadingState: ({ label }: { label?: string }) => <div data-testid="usage-loading">{label}</div>,
  ConfigMessage: ({
    message,
  }: {
    message: { type: string; text: string } | null;
  }) => message ? (
    <div data-testid="usage-message" data-message-type={message.type}>{message.text}</div>
  ) : null,
  ConfigRefreshButton: () => <button type="button" data-testid="usage-refresh" />,
}));

const SAMPLE_STATS: UsageStatistics = {
  totalRequests: 47,
  totalTokens: 4_800_000,
  totalInputTokens: 4_400_000,
  totalOutputTokens: 400_000,
  totalCachedTokens: 4_200_000,
  totalCacheWriteTokens: 0,
  totalCacheReportedInputTokens: 4_400_000,
  byModel: [
    {
      key: 'model-config:deepseek',
      name: 'deepseek-v4-flash',
      providerName: 'DeepSeek',
      attributionStatus: 'resolved',
      requests: 47,
      tokens: 4_800_000,
      cacheHitRate: 0.95679,
    },
  ],
  byGroup: [
    {
      key: 'provider:deepseek',
      name: 'DeepSeek',
      providerName: null,
      attributionStatus: 'resolved',
      requests: 47,
      tokens: 4_800_000,
      cacheHitRate: 0.95,
    },
  ],
  byEndpoint: [
    {
      key: 'endpoint:api.openbitfun.com/v1/chat/completions',
      name: 'api.openbitfun.com/v1/chat/completions',
      providerName: null,
      attributionStatus: 'resolved',
      requests: 47,
      tokens: 4_800_000,
      cacheHitRate: 0.95,
    },
  ],
  trend: [
    {
      bucket: '2026-08-16T11:00:00.000Z',
      inputTokens: 1_000_000,
      outputTokens: 100_000,
      cacheReadTokens: 900_000,
      cacheWriteTokens: 50_000,
      cacheHitRate: 0.9,
    },
    {
      bucket: '2026-08-16T12:00:00.000Z',
      inputTokens: 2_000_000,
      outputTokens: 200_000,
      cacheReadTokens: 1_900_000,
      cacheWriteTokens: 0,
      cacheHitRate: 0.95,
    },
  ],
  granularity: 'hour',
};

describe('UsageStatisticsConfig', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    getStatisticsMock.mockReset();
    getStatisticsMock.mockResolvedValue(SAMPLE_STATS);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  async function render() {
    await act(async () => {
      root.render(<UsageStatisticsConfig />);
    });
    // Flush the async load that fires from useEffect.
    await act(async () => {
      await Promise.resolve();
    });
  }

  it('requests statistics on mount and renders summary, distributions, and trend', async () => {
    await render();

    expect(getStatisticsMock).toHaveBeenCalledTimes(1);
    expect(getStatisticsMock).toHaveBeenCalledWith({
      timeRange: 'last24Hours',
      granularity: 'hour',
      timeZone: 'UTC',
    });

    const pageHeader = container.querySelector('[data-openbitfun-component="page-header"]');
    expect(pageHeader?.querySelector('h2')?.textContent).toBe('title');
    expect(pageHeader?.textContent).toContain('subtitle');
    expect(container.querySelector('[data-openbitfun-part="summary"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="distributions"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="modelHitRate"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="trendPanel"]')).not.toBeNull();
    expect(container.querySelectorAll('.openbitfun-usage-stats__donut').length).toBe(3);
    expect(container.querySelectorAll('[data-openbitfun-part="trendPanel"] svg').length).toBe(1);
    expect(container.textContent).not.toContain('trend.legend.cacheCreation');
    expect(container.querySelectorAll('.openbitfun-config-page-section')).toHaveLength(4);
    expect(container.querySelectorAll('[data-openbitfun-part="distributions"] table')).toHaveLength(3);
    expect(container.querySelectorAll('[data-openbitfun-part="distributions"] th[scope="row"]')).toHaveLength(3);
    expect(container.querySelector('[data-openbitfun-part="trendPanel"] svg[role="img"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="trendPanel"] table.openbitfun-sr-only')).not.toBeNull();
    expect(container.textContent).toContain('4.8M');
    // Hit rate rounds down and always keeps two decimal places.
    expect(container.textContent).toContain('95.67%');
    expect(container.textContent).toContain('95.00%');
  });

  it('keeps idle hit-rate points continuous but splits active telemetry gaps', async () => {
    const idlePoint = {
      ...SAMPLE_STATS.trend[0],
      bucket: '2026-08-16T12:00:00.000Z',
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheWriteTokens: 0,
      cacheHitRate: null,
    };
    const activeGapPoint = {
      ...SAMPLE_STATS.trend[0],
      bucket: '2026-08-16T14:00:00.000Z',
      cacheHitRate: null,
    };
    getStatisticsMock.mockResolvedValue({
      ...SAMPLE_STATS,
      trend: [
        SAMPLE_STATS.trend[0],
        idlePoint,
        { ...SAMPLE_STATS.trend[1], bucket: '2026-08-16T13:00:00.000Z' },
        activeGapPoint,
        { ...SAMPLE_STATS.trend[1], bucket: '2026-08-16T15:00:00.000Z' },
      ],
    });

    await render();

    expect(container.querySelectorAll('[data-cache-hit-rate-segment="line"]')).toHaveLength(1);
    expect(container.querySelectorAll('[data-cache-hit-rate-segment="point"]')).toHaveLength(1);

    const hoverCapture = container.querySelector(
      '.openbitfun-usage-stats__trend-svg > rect[fill="transparent"]',
    ) as SVGRectElement;
    vi.spyOn(hoverCapture, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      width: 400,
    } as DOMRect);

    await act(async () => {
      hoverCapture.dispatchEvent(new MouseEvent('pointermove', {
        bubbles: true,
        clientX: 300,
      }));
    });
    let tooltipRows = container.querySelectorAll('.openbitfun-usage-stats__trend-tooltip-row');
    expect(tooltipRows[tooltipRows.length - 1]?.textContent).toContain('–');

    await act(async () => {
      hoverCapture.dispatchEvent(new MouseEvent('pointermove', {
        bubbles: true,
        clientX: 100,
      }));
    });
    tooltipRows = container.querySelectorAll('.openbitfun-usage-stats__trend-tooltip-row');
    expect(tooltipRows[tooltipRows.length - 1]?.textContent).toContain('0.00%');
  });

  it('keeps same-named models distinct and labels deleted configurations', async () => {
    getStatisticsMock.mockResolvedValue({
      ...SAMPLE_STATS,
      byModel: [
        {
          ...SAMPLE_STATS.byModel[0],
          key: 'model-config:openbitfun',
          name: 'MiniMax-M3',
          providerName: 'OpenBitFun',
        },
        {
          ...SAMPLE_STATS.byModel[0],
          key: 'model-config:minimax',
          name: 'MiniMax-M3',
          providerName: 'MiniMax',
        },
        {
          ...SAMPLE_STATS.byModel[0],
          key: 'missing-config:deleted',
          name: 'legacy-model',
          providerName: null,
          attributionStatus: 'config_missing',
        },
      ],
    });

    await render();

    expect(container.textContent).toContain('OpenBitFun');
    expect(container.textContent).toContain('MiniMax');
    expect(container.textContent).toContain('attribution.deletedConfig');
    expect(container.querySelectorAll('.openbitfun-usage-stats__hit-rate-row')).toHaveLength(3);
  });

  it('shows the empty state when there are no records', async () => {
    getStatisticsMock.mockResolvedValue({
      ...SAMPLE_STATS,
      totalRequests: 0,
      byModel: [],
      byGroup: [],
      byEndpoint: [],
      trend: [],
    });

    await render();

    expect(container.querySelector('[data-openbitfun-part="empty"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="summary"]')).toBeNull();
  });

  it('shows a distinct informational state for an older Peer host', async () => {
    getStatisticsMock.mockRejectedValue(new TokenUsageStatisticsUnavailableErrorMock());

    await render();

    const message = container.querySelector('[data-testid="usage-message"]');
    expect(message?.getAttribute('data-message-type')).toBe('info');
    expect(message?.textContent).toBe('unsupported');
  });

  it('refetches when the time range selection changes', async () => {
    await render();
    expect(getStatisticsMock).toHaveBeenCalledTimes(1);

    const select = container.querySelector('[data-testid="usage-select"]') as HTMLSelectElement;
    expect(select).not.toBeNull();

    await act(async () => {
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    // Simulate the second (time range) select value change via React state by
    // re-rendering the component with a new selection using the select element.
    // The first select is time range; the second is granularity.
    const selects = container.querySelectorAll('[data-testid="usage-select"]');
    expect(selects.length).toBe(3);

    await act(async () => {
      const nativeSet = Object.getOwnPropertyDescriptor(
        HTMLSelectElement.prototype,
        'value',
      )?.set;
      nativeSet?.call(selects[0], 'thisMonth');
      selects[0].dispatchEvent(new Event('change', { bubbles: true }));
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(getStatisticsMock).toHaveBeenCalledTimes(2);
    expect(getStatisticsMock).toHaveBeenLastCalledWith({
      timeRange: 'thisMonth',
      granularity: 'hour',
      timeZone: 'UTC',
    });
  });

  it('debounces text filtering and refetches when the filter kind changes', async () => {
    await render();
    const input = container.querySelector(
      '[data-testid="usage-filter-input"]',
    ) as HTMLInputElement;
    expect(input).not.toBeNull();

    await act(async () => {
      const nativeSet = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSet?.call(input, 'DeepSeek');
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise(resolve => window.setTimeout(resolve, 350));
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(getStatisticsMock).toHaveBeenCalledTimes(2);
    expect(getStatisticsMock).toHaveBeenLastCalledWith({
      timeRange: 'last24Hours',
      granularity: 'hour',
      timeZone: 'UTC',
      filterKind: 'all',
      filterQuery: 'DeepSeek',
    });

    const selects = container.querySelectorAll('[data-testid="usage-select"]');
    await act(async () => {
      const nativeSet = Object.getOwnPropertyDescriptor(
        HTMLSelectElement.prototype,
        'value',
      )?.set;
      nativeSet?.call(selects[2], 'provider');
      selects[2].dispatchEvent(new Event('change', { bubbles: true }));
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(getStatisticsMock).toHaveBeenCalledTimes(3);
    expect(getStatisticsMock).toHaveBeenLastCalledWith({
      timeRange: 'last24Hours',
      granularity: 'hour',
      timeZone: 'UTC',
      filterKind: 'provider',
      filterQuery: 'DeepSeek',
    });
  });

  it('shows a distinct empty state when a filter has no matches', async () => {
    getStatisticsMock
      .mockResolvedValueOnce(SAMPLE_STATS)
      .mockResolvedValueOnce({
        ...SAMPLE_STATS,
        totalRequests: 0,
        byModel: [],
        byGroup: [],
        byEndpoint: [],
        trend: [],
      });

    await render();
    const input = container.querySelector(
      '[data-testid="usage-filter-input"]',
    ) as HTMLInputElement;
    await act(async () => {
      const nativeSet = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSet?.call(input, 'missing-model');
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await new Promise(resolve => window.setTimeout(resolve, 350));
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(container.textContent).toContain('filter.empty.title');
    expect(container.textContent).toContain('filter.empty.description');
  });

  it('surfaces load failures without crashing', async () => {
    getStatisticsMock.mockRejectedValue(new Error('boom'));

    await render();

    expect(container.querySelector('[data-testid="usage-loading"]')).toBeNull();
  });
});
