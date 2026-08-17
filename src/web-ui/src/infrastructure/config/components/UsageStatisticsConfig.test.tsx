// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import UsageStatisticsConfig from './UsageStatisticsConfig';
import type { UsageStatistics } from '@/infrastructure/api';

const getStatisticsMock = vi.hoisted(() => vi.fn());
const translateMock = vi.hoisted(() => vi.fn((key: string) => key));

vi.mock('@/infrastructure/api', () => ({
  tokenUsageStatisticsApi: {
    getStatistics: getStatisticsMock,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: translateMock,
    formatDate: (date: Date | number) => new Date(date).toISOString(),
  }),
}));

vi.mock('@/component-library', () => ({
  ConfigPageLoading: ({ text }: { text?: string }) => <div data-testid="usage-loading">{text}</div>,
  ConfigPageMessage: () => null,
  ConfigPageRefreshButton: () => <button type="button" data-testid="usage-refresh" />,
  Select: ({
    value,
    options,
    onChange,
  }: {
    value: string | number;
    options: { value: string | number; label: string }[];
    onChange?: (value: string | number) => void;
  }) => (
    <select
      data-testid="usage-select"
      value={String(value)}
      onChange={(event) => onChange?.(event.target.value)}
    >
      {options.map((option) => (
        <option key={String(option.value)} value={String(option.value)}>
          {option.label}
        </option>
      ))}
    </select>
  ),
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
    { name: 'deepseek-v4-flash', requests: 47, tokens: 4_800_000, cacheHitRate: 0.95 },
  ],
  byGroup: [
    { name: 'DeepSeek', requests: 47, tokens: 4_800_000, cacheHitRate: 0.95 },
  ],
  byEndpoint: [
    { name: 'api.openbitfun.com/v1/chat/completions', requests: 47, tokens: 4_800_000, cacheHitRate: 0.95 },
  ],
  trend: [
    {
      bucket: '2026-08-16T11:00:00.000Z',
      inputTokens: 1_000_000,
      outputTokens: 100_000,
      cacheReadTokens: 900_000,
      cacheWriteTokens: 0,
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
    const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
    expect(getStatisticsMock).toHaveBeenCalledWith({
      timeRange: 'last24Hours',
      granularity: 'hour',
      timeZone,
    });

    expect(container.querySelector('[data-bf-part="summary"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="distributions"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="modelHitRate"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="trendPanel"]')).not.toBeNull();
    expect(container.querySelectorAll('.bitfun-usage-stats__donut').length).toBe(3);
    expect(container.querySelectorAll('[data-bf-part="trendPanel"] svg').length).toBe(1);
    // Hit rate is truncated to two decimals, never rounded up.
    expect(container.textContent).toContain('95.00%');
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

    expect(container.querySelector('[data-bf-part="empty"]')).not.toBeNull();
    expect(container.querySelector('[data-bf-part="summary"]')).toBeNull();
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
    expect(selects.length).toBe(2);

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
      timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC',
    });
  });

  it('surfaces load failures without crashing', async () => {
    getStatisticsMock.mockRejectedValue(new Error('boom'));

    await render();

    expect(container.querySelector('[data-testid="usage-loading"]')).toBeNull();
  });
});
