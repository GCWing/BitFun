import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { BarChart3 } from 'lucide-react';
import {
  TokenUsageStatisticsUnavailableError,
  tokenUsageStatisticsApi,
  type UsageGranularity,
  type UsageStatistics,
  type UsageStatisticsEntry,
  type UsageStatisticsFilterKind,
  type UsageTimeRange,
} from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
  ConfigPageSectionStack,
  ConfigLoadingState,
  ConfigMessage,
  ConfigRefreshButton,
} from './common';
import './UsageStatisticsConfig.scss';
import { Icon, IconButton, Input, Select, Tooltip, ScrollArea } from '@bitfun/ui';

// ---------------------------------------------------------------------------
// Chart palette — appearance tokens only (literal vars so the theme color
// audit can statically resolve every reference).
// ---------------------------------------------------------------------------

const SERIES_COLORS = {
  input: 'var(--bf-color-accent-default)',
  output: 'var(--bf-color-status-success-content)',
  cacheRead: 'var(--bf-color-status-info-content)',
  cacheHitRate: 'var(--bf-color-accent-secondary)',
} as const;

const DONUT_PALETTE = [
  'var(--bf-color-accent-default)',
  'var(--bf-color-accent-secondary)',
  'var(--bf-color-status-info-content)',
  'var(--bf-color-status-success-content)',
  'var(--bf-color-status-warning-content)',
  'var(--bf-color-accent-default)',
  'var(--bf-color-status-danger-content)',
  'var(--bf-color-action-secondary-pressed)',
  'color-mix(in srgb, var(--bf-color-accent-secondary) 15%, transparent)',
] as const;

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

type NumberFormatter = (
  value: number,
  options?: Intl.NumberFormatOptions,
) => string;

function formatTokens(value: number, formatNumber: NumberFormatter): string {
  if (Math.abs(value) < 1_000) return formatNumber(value);
  return formatNumber(value, {
    notation: 'compact',
    compactDisplay: 'short',
    maximumFractionDigits: Math.abs(value) >= 1_000_000 ? 2 : 1,
  });
}

function formatHitRate(value: number | null, formatNumber: NumberFormatter): string {
  if (value === null || !Number.isFinite(value)) return '–';
  // Only a true full hit (cached == reported input) shows as 100%.
  if (value >= 1) return formatNumber(1, { style: 'percent', maximumFractionDigits: 0 });
  // Truncate to two decimals — never round up.
  const truncated = Math.floor(value * 10_000) / 10_000;
  return formatNumber(truncated, {
    style: 'percent',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function formatBucketLabel(
  bucket: string,
  granularity: UsageGranularity,
  timeZone: string,
  formatDate: (date: Date | number, options?: Intl.DateTimeFormatOptions) => string,
): string {
  const date = new Date(bucket);
  if (Number.isNaN(date.getTime())) return bucket;
  return formatDate(date, {
    month: '2-digit',
    day: '2-digit',
    timeZone,
    ...(granularity === 'hour'
      ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' as const }
      : {}),
  });
}

type DistributionKind = 'model' | 'group' | 'endpoint';

interface UsageEntryDisplay {
  primary: string;
  secondary?: string;
}

function unresolvedConfigLabel(
  entry: UsageStatisticsEntry,
  t: (key: string) => string,
): string | undefined {
  if (entry.attributionStatus === 'config_missing') return t('attribution.deletedConfig');
  if (entry.attributionStatus === 'config_id_missing') return t('attribution.unknownConfig');
  return undefined;
}

function getEntryDisplay(
  entry: UsageStatisticsEntry,
  kind: DistributionKind,
  t: (key: string) => string,
): UsageEntryDisplay {
  const unresolvedLabel = unresolvedConfigLabel(entry, t);

  if (kind === 'model') {
    return {
      primary: entry.name || t('attribution.unknownModel'),
      secondary: unresolvedLabel || entry.providerName || t('attribution.unknownProvider'),
    };
  }
  if (kind === 'group' && unresolvedLabel) {
    return {
      primary: unresolvedLabel,
      secondary: entry.name || t('attribution.unknownModel'),
    };
  }
  if (kind === 'endpoint' && unresolvedLabel) {
    return {
      primary: t('attribution.unknownEndpoint'),
      secondary: unresolvedLabel,
    };
  }
  return {
    primary: entry.name || (
      kind === 'endpoint'
        ? t('attribution.unknownEndpoint')
        : t('attribution.unknownProvider')
    ),
  };
}

function entryTitle(display: UsageEntryDisplay): string {
  return display.secondary ? `${display.primary} · ${display.secondary}` : display.primary;
}

// ---------------------------------------------------------------------------
// Donut chart
// ---------------------------------------------------------------------------

const DonutChart: React.FC<{
  kind: DistributionKind;
  entries: UsageStatisticsEntry[];
}> = ({ kind, entries }) => {
  const { t, formatNumber } = useI18n('settings/usage');
  const totalTokens = entries.reduce((sum, entry) => sum + entry.tokens, 0);
  const formattedTotal = formatTokens(totalTokens, formatNumber);
  const radius = 54;
  const circumference = 2 * Math.PI * radius;
  let cumulative = 0;

  return (
    <div className="bitfun-usage-stats__donut">
      <svg
        viewBox="0 0 140 140"
        role="img"
        aria-label={t('chart.totalTokens', { count: formattedTotal })}
      >
        <circle
          cx="70"
          cy="70"
          r={radius}
          fill="none"
          stroke="var(--bf-color-action-quiet-hover)"
          strokeWidth="16"
        />
        {entries.map((entry, index) => {
          const display = getEntryDisplay(entry, kind, t);
          const fraction = totalTokens > 0 ? entry.tokens / totalTokens : 0;
          const dash = Math.max(fraction * circumference - 1.5, 0);
          const segment = (
            <circle
              key={entry.key}
              cx="70"
              cy="70"
              r={radius}
              fill="none"
              stroke={DONUT_PALETTE[index % DONUT_PALETTE.length]}
              strokeWidth="16"
              strokeDasharray={`${dash} ${circumference - dash}`}
              strokeDashoffset={-cumulative}
              transform="rotate(-90 70 70)"
            >
              <title>
                {`${entryTitle(display)}: ${formatTokens(entry.tokens, formatNumber)} ${t('chart.tokensUnit')}`}
              </title>
            </circle>
          );
          cumulative += fraction * circumference;
          return segment;
        })}
        {totalTokens === 0 && (
          <circle
            cx="70"
            cy="70"
            r={radius}
            fill="none"
            stroke="var(--bf-color-action-quiet-hover)"
            strokeWidth="16"
          />
        )}
        <text
          x="70"
          y="66"
          textAnchor="middle"
          className="bitfun-usage-stats__donut-total"
        >
          {formattedTotal}
        </text>
        <text
          x="70"
          y="82"
          textAnchor="middle"
          className="bitfun-usage-stats__donut-unit"
        >
          {t('chart.tokensUnit')}
        </text>
      </svg>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Distribution panel: donut + table
// ---------------------------------------------------------------------------

const DISTRIBUTION_HEADER_KEY: Record<
  DistributionKind,
  string
> = {
  model: 'table.model',
  group: 'table.group',
  endpoint: 'table.endpoint',
};

const DistributionPanel: React.FC<{
  kind: DistributionKind;
  entries: UsageStatisticsEntry[];
}> = ({ kind, entries }) => {
  const { t, formatNumber } = useI18n('settings/usage');
  const titleId = useId();
  const titleKey = {
    model: 'distributions.byModel',
    group: 'distributions.byGroup',
    endpoint: 'distributions.byEndpoint',
  }[kind];
  const title = t(titleKey);

  return (
    <section
      className="bitfun-usage-stats__distribution"
      aria-labelledby={titleId}
    >
      <h4 id={titleId} className="bitfun-usage-stats__distribution-title">{title}</h4>
      <div className="bitfun-usage-stats__panel-body">
        <DonutChart kind={kind} entries={entries} />
        <ScrollArea className="bitfun-usage-stats__table-scroll">
          <table className="bitfun-usage-stats__table">
            <caption className="bitfun-sr-only">
              {t('table.caption', { dimension: title })}
            </caption>
            <thead>
              <tr>
                <th scope="col">{t(DISTRIBUTION_HEADER_KEY[kind])}</th>
                <th scope="col">{t('table.requests')}</th>
                <th scope="col">{t('table.tokens')}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry, index) => {
                const display = getEntryDisplay(entry, kind, t);
                return (
                  <tr key={entry.key} title={entryTitle(display)}>
                    <th scope="row">
                      <span className="bitfun-usage-stats__table-name">
                        <span
                          aria-hidden="true"
                          className="bitfun-usage-stats__table-swatch"
                          style={{ background: DONUT_PALETTE[index % DONUT_PALETTE.length] }}
                        />
                        <span className="bitfun-usage-stats__entry-copy">
                          <span className="bitfun-usage-stats__entry-primary">
                            {display.primary}
                          </span>
                          {display.secondary && (
                            <span className="bitfun-usage-stats__entry-secondary">
                              {display.secondary}
                            </span>
                          )}
                        </span>
                      </span>
                    </th>
                    <td>{formatNumber(entry.requests)}</td>
                    <td>{formatTokens(entry.tokens, formatNumber)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </ScrollArea>
      </div>
    </section>
  );
};

// ---------------------------------------------------------------------------
// Per-model average cache hit rate list
// ---------------------------------------------------------------------------

const ModelCacheHitRateList: React.FC<{ entries: UsageStatisticsEntry[] }> = ({ entries }) => {
  const { t, formatNumber } = useI18n('settings/usage');

  return (
    <ScrollArea
      className="bitfun-usage-stats__hit-rate-list"
      role="list"
    >
      {entries.map((entry, index) => {
        const display = getEntryDisplay(entry, 'model', t);
        const rate = entry.cacheHitRate;
        const pct = rate === null || !Number.isFinite(rate)
          ? 0
          : Math.min(Math.max(rate * 100, 0), 100);
        const color = DONUT_PALETTE[index % DONUT_PALETTE.length];
        return (
          <div
            className="bitfun-usage-stats__hit-rate-row"
            key={entry.key}
            title={entryTitle(display)}
            role="listitem"
          >
            <span className="bitfun-usage-stats__hit-rate-name">
              <span
                aria-hidden="true"
                className="bitfun-usage-stats__table-swatch"
                style={{ background: color }}
              />
              <span className="bitfun-usage-stats__entry-copy">
                <span className="bitfun-usage-stats__entry-primary">
                  {display.primary}
                </span>
                {display.secondary && (
                  <span className="bitfun-usage-stats__entry-secondary">
                    {display.secondary}
                  </span>
                )}
              </span>
            </span>
            <div className="bitfun-usage-stats__hit-rate-track" aria-hidden="true">
              <div
                className="bitfun-usage-stats__hit-rate-fill"
                style={{ width: `${pct}%`, background: color }}
              />
            </div>
            <span className="bitfun-usage-stats__hit-rate-value">
              {formatHitRate(entry.cacheHitRate, formatNumber)}
            </span>
          </div>
        );
      })}
    </ScrollArea>
  );
};

// ---------------------------------------------------------------------------
// Token usage trend line chart (SVG, no chart library)
// ---------------------------------------------------------------------------

interface TrendChartProps {
  points: UsageStatistics['trend'];
  granularity: UsageGranularity;
  timeZone: string;
}

const TREND_SERIES: {
  key: 'inputTokens' | 'outputTokens' | 'cacheReadTokens';
  color: string;
  legendKey: string;
}[] = [
  { key: 'inputTokens', color: SERIES_COLORS.input, legendKey: 'trend.legend.input' },
  { key: 'outputTokens', color: SERIES_COLORS.output, legendKey: 'trend.legend.output' },
  { key: 'cacheReadTokens', color: SERIES_COLORS.cacheRead, legendKey: 'trend.legend.cacheRead' },
];

const CHART_WIDTH = 640;
const CHART_HEIGHT = 240;
const PAD_LEFT = 56;
const PAD_RIGHT = 48;
const PAD_TOP = 16;
const PAD_BOTTOM = 30;

function niceMax(value: number): number {
  if (value <= 0) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(value));
  const normalized = value / magnitude;
  const nice = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 2.5 ? 2.5 : normalized <= 5 ? 5 : 10;
  return nice * magnitude;
}

function cacheHitRateForTrend(
  point: UsageStatistics['trend'][number],
): number | null {
  if (point.cacheHitRate !== null) return point.cacheHitRate;

  const isIdleBucket = point.inputTokens === 0
    && point.outputTokens === 0
    && point.cacheReadTokens === 0
    && point.cacheWriteTokens === 0;
  return isIdleBucket ? 0 : null;
}

const TrendChart: React.FC<TrendChartProps> = ({ points, granularity, timeZone }) => {
  const { t, formatDate, formatNumber } = useI18n('settings/usage');
  const titleId = useId();
  const descriptionId = useId();
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  const plotWidth = CHART_WIDTH - PAD_LEFT - PAD_RIGHT;
  const plotHeight = CHART_HEIGHT - PAD_TOP - PAD_BOTTOM;

  const maxTokens = niceMax(
    points.reduce((max, point) => Math.max(
      max,
      point.inputTokens,
      point.outputTokens,
      point.cacheReadTokens,
    ), 0),
  );
  const yTicks = 4;

  const xFor = (index: number): number => {
    if (points.length <= 1) return PAD_LEFT + plotWidth / 2;
    return PAD_LEFT + (index / (points.length - 1)) * plotWidth;
  };
  const yFor = (value: number): number => (
    PAD_TOP + plotHeight - (value / maxTokens) * plotHeight
  );
  const rateFor = (value: number): number => (
    PAD_TOP + plotHeight - value * plotHeight
  );

  const xTickIndexes = useMemo(() => {
    if (points.length <= 6) return points.map((_, index) => index);
    const count = 6;
    const step = (points.length - 1) / (count - 1);
    return Array.from({ length: count }, (_, index) => Math.round(index * step));
  }, [points]);

  if (points.length === 0) return null;

  const hovered = hoverIndex !== null ? points[hoverIndex] : null;
  const hoveredHitRate = hovered ? cacheHitRateForTrend(hovered) : null;

  // Synthesized idle buckets sit at 0% to keep ordinary idle stretches
  // continuous. Active buckets without cache telemetry remain real gaps so
  // the chart does not claim that an unsupported provider had a 0% hit rate.
  const hitRateSegments: Array<Array<{ x: number; y: number }>> = [];
  {
    let current: Array<{ x: number; y: number }> = [];
    points.forEach((point, index) => {
      const rate = cacheHitRateForTrend(point);
      if (rate === null) {
        if (current.length > 0) {
          hitRateSegments.push(current);
          current = [];
        }
        return;
      }
      current.push({ x: xFor(index), y: rateFor(rate) });
    });
    if (current.length > 0) hitRateSegments.push(current);
  }
  const tooltipRows = hovered ? [
    {
      label: t('trend.legend.input'),
      value: formatTokens(hovered.inputTokens, formatNumber),
      color: SERIES_COLORS.input,
    },
    {
      label: t('trend.legend.output'),
      value: formatTokens(hovered.outputTokens, formatNumber),
      color: SERIES_COLORS.output,
    },
    {
      label: t('trend.legend.cacheRead'),
      value: formatTokens(hovered.cacheReadTokens, formatNumber),
      color: SERIES_COLORS.cacheRead,
    },
    {
      label: t('trend.legend.cacheHitRate'),
      value: formatHitRate(hoveredHitRate, formatNumber),
      color: SERIES_COLORS.cacheHitRate,
    },
  ] : [];

  return (
    <div className="bitfun-usage-stats__trend">
      <svg
        viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}
        className="bitfun-usage-stats__trend-svg"
        role="img"
        aria-labelledby={`${titleId} ${descriptionId}`}
        onPointerLeave={() => setHoverIndex(null)}
      >
        <title id={titleId}>{t('trend.title')}</title>
        <desc id={descriptionId}>{t('trend.description')}</desc>
        {/* Horizontal grid + left (tokens) and right (hit rate) axis labels */}
        {Array.from({ length: yTicks + 1 }, (_, index) => {
          const value = (maxTokens / yTicks) * index;
          const y = yFor(value);
          const rate = Math.round((index / yTicks) * 100);
          return (
            <g key={index}>
              <line
                x1={PAD_LEFT}
                y1={y}
                x2={CHART_WIDTH - PAD_RIGHT}
                y2={y}
                className="bitfun-usage-stats__trend-grid"
              />
              <text x={PAD_LEFT - 8} y={y + 4} textAnchor="end" className="bitfun-usage-stats__trend-axis">
                {formatTokens(value, formatNumber)}
              </text>
              <text x={CHART_WIDTH - PAD_RIGHT + 8} y={y + 4} className="bitfun-usage-stats__trend-axis">
                {rate}%
              </text>
            </g>
          );
        })}

        {/* X axis labels */}
        {xTickIndexes.map((index) => (
          <text
            key={index}
            x={xFor(index)}
            y={CHART_HEIGHT - 8}
            textAnchor="middle"
            className="bitfun-usage-stats__trend-axis"
          >
            {formatBucketLabel(points[index].bucket, granularity, timeZone, formatDate)}
          </text>
        ))}

        {/* Token series */}
        {TREND_SERIES.map((series) => (
          <polyline
            key={series.key}
            points={points
              .map((point, index) => `${xFor(index)},${yFor(point[series.key])}`)
              .join(' ')}
            fill="none"
            stroke={series.color}
            strokeWidth="2"
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        ))}

        {/* Cache hit rate (right axis, dashed). */}
        {hitRateSegments.map((segment, segmentIndex) =>
          segment.length === 1 ? (
            <circle
              key={`rate-segment-${segmentIndex}`}
              cx={segment[0].x}
              cy={segment[0].y}
              r="2.5"
              fill={SERIES_COLORS.cacheHitRate}
              data-cache-hit-rate-segment="point"
            />
          ) : (
            <polyline
              key={`rate-segment-${segmentIndex}`}
              points={segment.map(point => `${point.x},${point.y}`).join(' ')}
              fill="none"
              stroke={SERIES_COLORS.cacheHitRate}
              strokeWidth="2"
              strokeDasharray="4 4"
              strokeLinejoin="round"
              strokeLinecap="round"
              data-cache-hit-rate-segment="line"
            />
          ),
        )}

        {/* Hover capture */}
        <rect
          x={PAD_LEFT}
          y={PAD_TOP}
          width={plotWidth}
          height={plotHeight}
          fill="transparent"
          onPointerMove={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            const ratio = (event.clientX - rect.left) / rect.width;
            const index = Math.round(ratio * (points.length - 1));
            setHoverIndex(Math.min(Math.max(index, 0), points.length - 1));
          }}
        />
        {hovered && hoverIndex !== null && (
          <g>
            <line
              x1={xFor(hoverIndex)}
              y1={PAD_TOP}
              x2={xFor(hoverIndex)}
              y2={PAD_TOP + plotHeight}
              className="bitfun-usage-stats__trend-cursor"
            />
            {/* Hover markers: one dot per series so small values stay visible
                where a zero-baseline token axis would otherwise flatten them
                (e.g. 276K next to a 20M peak). */}
            {TREND_SERIES.map((series) => (
              <circle
                key={`hover-dot-${series.key}`}
                cx={xFor(hoverIndex)}
                cy={yFor(hovered[series.key])}
                r="3.5"
                fill={series.color}
                stroke="var(--bf-color-action-quiet-hover)"
                strokeWidth="1"
              />
            ))}
            {hoveredHitRate !== null && (
              <circle
                cx={xFor(hoverIndex)}
                cy={rateFor(hoveredHitRate)}
                r="3.5"
                fill={SERIES_COLORS.cacheHitRate}
                stroke="var(--bf-color-action-quiet-hover)"
                strokeWidth="1"
              />
            )}
            <g className="bitfun-usage-stats__trend-tooltip">
              <rect
                x={Math.min(Math.max(xFor(hoverIndex) - 92, PAD_LEFT), CHART_WIDTH - PAD_RIGHT - 184)}
                y={PAD_TOP}
                width="184"
                height="92"
                rx="6"
              />
              <text
                x={Math.min(Math.max(xFor(hoverIndex) - 80, PAD_LEFT + 12), CHART_WIDTH - PAD_RIGHT - 172)}
                y={PAD_TOP + 16}
                className="bitfun-usage-stats__trend-tooltip-title"
              >
                {formatBucketLabel(hovered.bucket, granularity, timeZone, formatDate)}
              </text>
              {tooltipRows.map((row, index) => (
                <text
                  key={row.label}
                  x={Math.min(Math.max(xFor(hoverIndex) - 80, PAD_LEFT + 12), CHART_WIDTH - PAD_RIGHT - 172)}
                  y={PAD_TOP + 34 + index * 14}
                  className="bitfun-usage-stats__trend-tooltip-row"
                >
                  <tspan fill={row.color}>● </tspan>
                  {row.label}: {row.value}
                </text>
              ))}
            </g>
          </g>
        )}
      </svg>

      <div className="bitfun-usage-stats__trend-legend">
        {[
          ...TREND_SERIES.map((series) => ({
            label: t(series.legendKey),
            color: series.color,
            dashed: false,
          })),
          { label: t('trend.legend.cacheHitRate'), color: SERIES_COLORS.cacheHitRate, dashed: true },
        ].map((item) => (
          <span key={item.label} className="bitfun-usage-stats__trend-legend-item">
            <span
              aria-hidden="true"
              className="bitfun-usage-stats__trend-legend-swatch"
              style={{
                background: item.color,
                ...(item.dashed
                  ? { backgroundImage: `repeating-linear-gradient(90deg, ${item.color} 0 4px, transparent 4px 8px)` }
                  : {}),
              }}
            />
            {item.label}
          </span>
        ))}
      </div>

      <table className="bitfun-sr-only">
        <caption>{t('trend.dataTableCaption')}</caption>
        <thead>
          <tr>
            <th scope="col">{t('trend.time')}</th>
            {TREND_SERIES.map((series) => (
              <th scope="col" key={series.key}>{t(series.legendKey)}</th>
            ))}
            <th scope="col">{t('trend.legend.cacheHitRate')}</th>
          </tr>
        </thead>
        <tbody>
          {points.map((point) => (
            <tr key={point.bucket}>
              <th scope="row">
                {formatBucketLabel(point.bucket, granularity, timeZone, formatDate)}
              </th>
              {TREND_SERIES.map((series) => (
                <td key={series.key}>{formatTokens(point[series.key], formatNumber)}</td>
              ))}
              <td>{formatHitRate(point.cacheHitRate, formatNumber)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

const TIME_RANGE_OPTIONS: { value: UsageTimeRange; key: string }[] = [
  { value: 'last24Hours', key: 'timeRange.last24Hours' },
  { value: 'today', key: 'timeRange.today' },
  { value: 'thisWeek', key: 'timeRange.thisWeek' },
  { value: 'thisMonth', key: 'timeRange.thisMonth' },
  { value: 'all', key: 'timeRange.all' },
];

const GRANULARITY_OPTIONS: { value: UsageGranularity; key: string }[] = [
  { value: 'hour', key: 'granularity.hour' },
  { value: 'day', key: 'granularity.day' },
];

const FILTER_KIND_OPTIONS: { value: UsageStatisticsFilterKind; key: string }[] = [
  { value: 'all', key: 'filter.kind.all' },
  { value: 'provider', key: 'filter.kind.provider' },
  { value: 'model', key: 'filter.kind.model' },
];

const FILTER_DEBOUNCE_MS = 300;

const UsageStatisticsConfig: React.FC = () => {
  const { t, resolvedTimeZone: timeZone, formatNumber } = useI18n('settings/usage');
  const [timeRange, setTimeRange] = useState<UsageTimeRange>('last24Hours');
  const [granularity, setGranularity] = useState<UsageGranularity>('hour');
  const [filterKind, setFilterKind] = useState<UsageStatisticsFilterKind>('all');
  const [filterInput, setFilterInput] = useState('');
  const [filterQuery, setFilterQuery] = useState('');
  const [stats, setStats] = useState<UsageStatistics | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [message, setMessage] = useState<{
    type: 'error' | 'info';
    text: string;
  } | null>(null);
  const requestIdRef = useRef(0);
  const hasLoadedRef = useRef(false);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setFilterQuery(filterInput.trim());
    }, FILTER_DEBOUNCE_MS);
    return () => window.clearTimeout(timeout);
  }, [filterInput]);

  const clearFilter = useCallback(() => {
    setFilterInput('');
    setFilterQuery('');
  }, []);

  const activeFilterKind: UsageStatisticsFilterKind = filterQuery ? filterKind : 'all';

  const load = useCallback(async (background = false) => {
    const requestId = ++requestIdRef.current;
    if (background) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setMessage(null);
    try {
      const result = await tokenUsageStatisticsApi.getStatistics({
        timeRange,
        granularity,
        timeZone,
        ...(filterQuery ? { filterKind: activeFilterKind, filterQuery } : {}),
      });
      if (requestId !== requestIdRef.current) return;
      setStats(result);
      hasLoadedRef.current = true;
    } catch (error) {
      if (requestId !== requestIdRef.current) return;
      setMessage(error instanceof TokenUsageStatisticsUnavailableError
        ? { type: 'info', text: t('unsupported') }
        : { type: 'error', text: t('loadFailed') });
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, [timeRange, granularity, timeZone, activeFilterKind, filterQuery, t]);

  useEffect(() => {
    void load(hasLoadedRef.current);
  }, [load]);

  const empty = stats !== null && stats.totalRequests === 0;
  const filteredEmpty = empty && filterQuery.length > 0;

  const summaryCards = useMemo(() => {
    if (!stats) return [];
    const overallHitRate = stats.totalCacheReportedInputTokens > 0
      ? stats.totalCachedTokens / stats.totalCacheReportedInputTokens
      : null;
    return [
      { key: 'summary.requests', value: formatNumber(stats.totalRequests) },
      { key: 'summary.tokens', value: formatTokens(stats.totalTokens, formatNumber) },
      { key: 'summary.cachedTokens', value: formatTokens(stats.totalCachedTokens, formatNumber) },
      {
        key: 'summary.cacheHitRate',
        value: formatHitRate(overallHitRate, formatNumber),
        highlight: true,
      },
    ];
  }, [formatNumber, stats]);

  return (
    <ConfigPageLayout
      className="bitfun-usage-stats"
      data-bf-component="usage-statistics-config"
      data-bf-part="root"
    >
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      <ConfigPageContent>
        <ConfigPageSectionStack>
          <ConfigPageSection
            title={t('overview.title')}
            description={t('overview.description')}
            extra={(
              <ConfigRefreshButton
                tooltip={t('refresh')}
                onClick={() => void load(true)}
                loading={refreshing}
                disabled={loading}
              />
            )}
            data-bf-component="usage-statistics-config"
            data-bf-part="overview"
          >
            <div
              className="bitfun-usage-stats__filters"
              data-bf-component="usage-statistics-config"
              data-bf-part="filters"
            >
              <div className="bitfun-usage-stats__filter-field">
                <span className="bitfun-usage-stats__filter-label">{t('timeRange.label')}</span>
                <Select
                  className="bitfun-usage-stats__filter-select"
                  size="sm"
                  value={timeRange}
                  options={TIME_RANGE_OPTIONS.map((option) => ({
                    value: option.value,
                    label: t(option.key),
                  }))}
                  onValueChange={(value) => setTimeRange(value as UsageTimeRange)}
                  aria-label={t('timeRange.label')}
                  disabled={loading}
                />
              </div>
              <div className="bitfun-usage-stats__filter-field">
                <span className="bitfun-usage-stats__filter-label">{t('granularity.label')}</span>
                <Select
                  className="bitfun-usage-stats__filter-select"
                  size="sm"
                  value={granularity}
                  options={GRANULARITY_OPTIONS.map((option) => ({
                    value: option.value,
                    label: t(option.key),
                  }))}
                  onValueChange={(value) => setGranularity(value as UsageGranularity)}
                  aria-label={t('granularity.label')}
                  disabled={loading}
                />
              </div>
              <div className="bitfun-usage-stats__filter-field bitfun-usage-stats__filter-field--query">
                <span className="bitfun-usage-stats__filter-label">{t('filter.inputLabel')}</span>
                <div className="bitfun-usage-stats__filter-query">
                  <Select
                    className="bitfun-usage-stats__filter-kind"
                    size="sm"
                    value={filterKind}
                    options={FILTER_KIND_OPTIONS.map((option) => ({
                      value: option.value,
                      label: t(option.key),
                    }))}
                    onValueChange={(value) => setFilterKind(value as UsageStatisticsFilterKind)}
                    aria-label={t('filter.kind.label')}
                    disabled={loading}
                  />
                  <Input
                    className="bitfun-usage-stats__filter-input"
                    value={filterInput}
                    onChange={(event) => setFilterInput(event.target.value)}
                    placeholder={t('filter.placeholder')}
                    aria-label={t('filter.inputLabel')}
                    data-testid="usage-filter-input"
                    maxLength={100}
                    disabled={loading}
                    leading={<Icon name="search" size="sm" aria-hidden />}
                    trailing={filterInput ? (
                      <Tooltip content={t('filter.clear')}>
                        <IconButton
                          type="button"
                          size="sm"
                          aria-label={t('filter.clear')}
                          onClick={clearFilter}
                          disabled={loading}
                          icon={<Icon name="xmark" size="xs" aria-hidden />}
                        />
                      </Tooltip>
                    ) : undefined}
                    size="sm"
                  />
                </div>
              </div>
            </div>

            <ConfigMessage
              className="bitfun-usage-stats__message"
              message={message}
            />

            {loading ? (
              <ConfigLoadingState label={t('loading')} />
            ) : empty ? (
              <div
                className="bitfun-usage-stats__empty"
                data-bf-component="usage-statistics-config"
                data-bf-part="empty"
              >
                <BarChart3 size={26} aria-hidden />
                <div>
                  <h4>{t(filteredEmpty ? 'filter.empty.title' : 'empty.title')}</h4>
                  <p>{t(filteredEmpty ? 'filter.empty.description' : 'empty.description')}</p>
                </div>
              </div>
            ) : stats ? (
              <div
                className="bitfun-usage-stats__summary"
                data-bf-component="usage-statistics-config"
                data-bf-part="summary"
              >
                {summaryCards.map((card) => (
                  <div className="bitfun-usage-stats__summary-card" key={card.key}>
                    <span className="bitfun-usage-stats__summary-label">{t(card.key)}</span>
                    <strong
                      className={[
                        'bitfun-usage-stats__summary-value',
                        card.highlight && 'bitfun-usage-stats__summary-value--highlight',
                      ].filter(Boolean).join(' ')}
                    >
                      {card.value}
                    </strong>
                  </div>
                ))}
              </div>
            ) : null}
          </ConfigPageSection>

          {!loading && !empty && stats ? (
            <>
              <ConfigPageSection
                title={t('cacheHitRate.title')}
                data-bf-component="usage-statistics-config"
                data-bf-part="modelHitRate"
              >
                <ModelCacheHitRateList entries={stats.byModel} />
              </ConfigPageSection>

              <ConfigPageSection
                title={t('distributions.title')}
                data-bf-component="usage-statistics-config"
                data-bf-part="distributions"
              >
                <div className="bitfun-usage-stats__distribution-list">
                  <DistributionPanel kind="model" entries={stats.byModel} />
                  <DistributionPanel kind="group" entries={stats.byGroup} />
                  <DistributionPanel kind="endpoint" entries={stats.byEndpoint} />
                </div>
              </ConfigPageSection>

              <ConfigPageSection
                title={t('trend.title')}
                description={t('trend.description')}
                data-bf-component="usage-statistics-config"
                data-bf-part="trendPanel"
              >
                <TrendChart
                  points={stats.trend}
                  granularity={stats.granularity}
                  timeZone={timeZone}
                />
              </ConfigPageSection>
            </>
          ) : null}
        </ConfigPageSectionStack>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default UsageStatisticsConfig;
