import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BarChart3, CalendarRange } from 'lucide-react';
import {
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
  Select,
} from '@/component-library';
import {
  tokenUsageStatisticsApi,
  type UsageGranularity,
  type UsageStatistics,
  type UsageStatisticsEntry,
  type UsageTimeRange,
} from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
} from './common';
import './UsageStatisticsConfig.scss';

// ---------------------------------------------------------------------------
// Chart palette — appearance tokens only (theme audit safe).
// ---------------------------------------------------------------------------

const TOKEN = (name: string): string => `var(--bf-appearance-token-${name})`;

const SERIES_COLORS = {
  input: TOKEN('color-accent-500'),
  output: TOKEN('color-success'),
  cacheCreation: TOKEN('color-warning'),
  cacheRead: TOKEN('color-cyan-500'),
  cacheHitRate: TOKEN('color-purple-500'),
} as const;

const DONUT_PALETTE = [
  TOKEN('color-accent-500'),
  TOKEN('color-purple-500'),
  TOKEN('color-cyan-500'),
  TOKEN('color-success'),
  TOKEN('color-warning'),
  TOKEN('color-indigo-500'),
  TOKEN('color-error'),
  TOKEN('color-accent-300'),
  TOKEN('color-purple-200'),
] as const;

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

function formatTokens(value: number): string {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}

function formatHitRate(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '–';
  return `${Math.round(value * 100)}%`;
}

function formatBucketLabel(bucket: string, granularity: UsageGranularity): string {
  const date = new Date(bucket);
  if (Number.isNaN(date.getTime())) return bucket;
  const mm = String(date.getUTCMonth() + 1).padStart(2, '0');
  const dd = String(date.getUTCDate()).padStart(2, '0');
  if (granularity === 'day') return `${mm}-${dd}`;
  return `${mm}-${dd} ${String(date.getUTCHours()).padStart(2, '0')}:00`;
}

function truncateName(name: string, max = 28): string {
  return name.length > max ? `${name.slice(0, max - 1)}…` : name;
}

// ---------------------------------------------------------------------------
// Donut chart
// ---------------------------------------------------------------------------

const DonutChart: React.FC<{ entries: UsageStatisticsEntry[] }> = ({ entries }) => {
  const totalTokens = entries.reduce((sum, entry) => sum + entry.tokens, 0);
  const radius = 54;
  const circumference = 2 * Math.PI * radius;
  let cumulative = 0;

  return (
    <div className="bitfun-usage-stats__donut" data-bf-part="donut">
      <svg viewBox="0 0 140 140" role="img" aria-label={`${totalTokens} tokens`}>
        <circle
          cx="70"
          cy="70"
          r={radius}
          fill="none"
          stroke={TOKEN('element-bg-soft')}
          strokeWidth="16"
        />
        {entries.map((entry, index) => {
          const fraction = totalTokens > 0 ? entry.tokens / totalTokens : 0;
          const dash = Math.max(fraction * circumference - 1.5, 0);
          const segment = (
            <circle
              key={entry.name}
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
              <title>{`${entry.name}: ${formatTokens(entry.tokens)}`}</title>
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
            stroke={TOKEN('element-bg-soft')}
            strokeWidth="16"
          />
        )}
        <text
          x="70"
          y="66"
          textAnchor="middle"
          className="bitfun-usage-stats__donut-total"
        >
          {formatTokens(totalTokens)}
        </text>
        <text
          x="70"
          y="82"
          textAnchor="middle"
          className="bitfun-usage-stats__donut-unit"
        >
          Tokens
        </text>
      </svg>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Distribution panel: donut + table
// ---------------------------------------------------------------------------

const DISTRIBUTION_HEADER_KEY: Record<
  'model' | 'group' | 'endpoint',
  string
> = {
  model: 'table.model',
  group: 'table.group',
  endpoint: 'table.endpoint',
};

const DistributionPanel: React.FC<{
  kind: 'model' | 'group' | 'endpoint';
  entries: UsageStatisticsEntry[];
}> = ({ kind, entries }) => {
  const { t } = useI18n('settings/usage-statistics');
  const titleKey = {
    model: 'distributions.byModel',
    group: 'distributions.byGroup',
    endpoint: 'distributions.byEndpoint',
  }[kind];

  return (
    <div
      className="bitfun-usage-stats__panel"
      data-bf-component="usage-statistics"
      data-bf-part={kind}
    >
      <div className="bitfun-usage-stats__panel-title">{t(titleKey)}</div>
      <div className="bitfun-usage-stats__panel-body">
        <DonutChart entries={entries} />
        <div className="bitfun-usage-stats__table">
          <div className="bitfun-usage-stats__table-head">
            <span>{t(DISTRIBUTION_HEADER_KEY[kind])}</span>
            <span>{t('table.requests')}</span>
            <span>{t('table.tokens')}</span>
            <span>{t('table.cacheHitRate')}</span>
          </div>
          <div className="bitfun-usage-stats__table-body">
            {entries.map((entry, index) => (
              <div
                className="bitfun-usage-stats__table-row"
                key={entry.name}
                title={entry.name}
              >
                <span className="bitfun-usage-stats__table-name">
                  <i
                    className="bitfun-usage-stats__table-swatch"
                    style={{ background: DONUT_PALETTE[index % DONUT_PALETTE.length] }}
                  />
                  {truncateName(entry.name)}
                </span>
                <span>{entry.requests}</span>
                <span>{formatTokens(entry.tokens)}</span>
                <span className="bitfun-usage-stats__hit-rate">
                  {formatHitRate(entry.cacheHitRate)}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Token usage trend line chart (SVG, no chart library)
// ---------------------------------------------------------------------------

interface TrendChartProps {
  points: UsageStatistics['trend'];
  granularity: UsageGranularity;
}

const TREND_SERIES: {
  key: 'inputTokens' | 'outputTokens' | 'cacheReadTokens' | 'cacheWriteTokens';
  color: string;
  legendKey: string;
}[] = [
  { key: 'inputTokens', color: SERIES_COLORS.input, legendKey: 'trend.legend.input' },
  { key: 'outputTokens', color: SERIES_COLORS.output, legendKey: 'trend.legend.output' },
  { key: 'cacheWriteTokens', color: SERIES_COLORS.cacheCreation, legendKey: 'trend.legend.cacheCreation' },
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

const TrendChart: React.FC<TrendChartProps> = ({ points, granularity }) => {
  const { t } = useI18n('settings/usage-statistics');
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  const plotWidth = CHART_WIDTH - PAD_LEFT - PAD_RIGHT;
  const plotHeight = CHART_HEIGHT - PAD_TOP - PAD_BOTTOM;

  const maxTokens = niceMax(
    points.reduce((max, point) => Math.max(
      max,
      point.inputTokens,
      point.outputTokens,
      point.cacheReadTokens,
      point.cacheWriteTokens,
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
  const rateFor = (value: number | null): number | null => (
    value === null ? null : PAD_TOP + plotHeight - value * plotHeight
  );

  const xTickIndexes = useMemo(() => {
    if (points.length <= 6) return points.map((_, index) => index);
    const count = 6;
    const step = (points.length - 1) / (count - 1);
    return Array.from({ length: count }, (_, index) => Math.round(index * step));
  }, [points]);

  if (points.length === 0) return null;

  const hovered = hoverIndex !== null ? points[hoverIndex] : null;

  return (
    <div className="bitfun-usage-stats__trend" data-bf-part="trend">
      <svg
        viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}
        className="bitfun-usage-stats__trend-svg"
        onMouseLeave={() => setHoverIndex(null)}
      >
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
                {formatTokens(value)}
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
            {formatBucketLabel(points[index].bucket, granularity)}
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

        {/* Cache hit rate (right axis, dashed) */}
        <polyline
          points={points
            .map((point, index) => {
              const y = rateFor(point.cacheHitRate);
              return y === null ? '' : `${xFor(index)},${y}`;
            })
            .filter(Boolean)
            .join(' ')}
          fill="none"
          stroke={SERIES_COLORS.cacheHitRate}
          strokeWidth="2"
          strokeDasharray="4 4"
          strokeLinejoin="round"
          strokeLinecap="round"
        />

        {/* Hover capture */}
        <rect
          x={PAD_LEFT}
          y={PAD_TOP}
          width={plotWidth}
          height={plotHeight}
          fill="transparent"
          onMouseMove={(event) => {
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
                {formatBucketLabel(hovered.bucket, granularity)}
              </text>
              {[
                { label: t('trend.legend.input'), value: hovered.inputTokens, color: SERIES_COLORS.input },
                { label: t('trend.legend.output'), value: hovered.outputTokens, color: SERIES_COLORS.output },
                { label: t('trend.legend.cacheCreation'), value: hovered.cacheWriteTokens, color: SERIES_COLORS.cacheCreation },
                { label: t('trend.legend.cacheRead'), value: hovered.cacheReadTokens, color: SERIES_COLORS.cacheRead },
                {
                  label: t('trend.legend.cacheHitRate'),
                  value: hovered.cacheHitRate === null ? null : `${Math.round(hovered.cacheHitRate * 100)}%`,
                  color: SERIES_COLORS.cacheHitRate,
                },
              ].map((row, index) => (
                <text
                  key={row.label}
                  x={Math.min(Math.max(xFor(hoverIndex) - 80, PAD_LEFT + 12), CHART_WIDTH - PAD_RIGHT - 172)}
                  y={PAD_TOP + 34 + index * 14}
                  className="bitfun-usage-stats__trend-tooltip-row"
                >
                  <tspan fill={row.color}>● </tspan>
                  {row.label}: {row.value === null ? '–' : formatTokens(row.value as number)}
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
            <i
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

const UsageStatisticsConfig: React.FC = () => {
  const { t } = useI18n('settings/usage-statistics');
  const [timeRange, setTimeRange] = useState<UsageTimeRange>('last24Hours');
  const [granularity, setGranularity] = useState<UsageGranularity>('hour');
  const [stats, setStats] = useState<UsageStatistics | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [message, setMessage] = useState<{ type: 'error'; text: string } | null>(null);
  const requestIdRef = useRef(0);

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
      });
      if (requestId !== requestIdRef.current) return;
      setStats(result);
    } catch {
      if (requestId !== requestIdRef.current) return;
      setMessage({ type: 'error', text: t('loadFailed') });
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, [timeRange, granularity, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const empty = stats !== null && stats.totalRequests === 0;

  const summaryCards = useMemo(() => {
    if (!stats) return [];
    const overallHitRate = stats.totalCacheReportedInputTokens > 0
      ? stats.totalCachedTokens / stats.totalCacheReportedInputTokens
      : null;
    return [
      { key: 'summary.requests', value: String(stats.totalRequests) },
      { key: 'summary.tokens', value: formatTokens(stats.totalTokens) },
      { key: 'summary.cachedTokens', value: formatTokens(stats.totalCachedTokens) },
      { key: 'summary.cacheHitRate', value: formatHitRate(overallHitRate), highlight: true },
    ];
  }, [stats]);

  return (
    <ConfigPageLayout
      className="bitfun-usage-stats"
      data-bf-component="usage-statistics"
      data-bf-part="root"
    >
      <ConfigPageHeader
        icon={<BarChart3 size={20} aria-hidden />}
        title={t('title')}
        subtitle={t('subtitle')}
      />
      <ConfigPageContent>
        <div className="bitfun-usage-stats__filters-bar" data-bf-part="filters">
          <label className="bitfun-usage-stats__filter">
            <CalendarRange size={14} aria-hidden />
            <Select
              size="small"
              value={timeRange}
              options={TIME_RANGE_OPTIONS.map((option) => ({
                value: option.value,
                label: t(option.key),
              }))}
              onChange={(value) => setTimeRange(value as UsageTimeRange)}
              triggerAriaLabel={t('timeRange.label')}
            />
          </label>
          <label className="bitfun-usage-stats__filter">
            <Select
              size="small"
              value={granularity}
              options={GRANULARITY_OPTIONS.map((option) => ({
                value: option.value,
                label: t(option.key),
              }))}
              onChange={(value) => setGranularity(value as UsageGranularity)}
              triggerAriaLabel={t('granularity.label')}
            />
          </label>
          <ConfigPageRefreshButton
            tooltip={t('refresh')}
            onClick={() => void load(true)}
            loading={refreshing}
            disabled={loading}
          />
        </div>

        <ConfigPageMessage message={message} />

        {loading ? (
          <ConfigPageLoading text={t('loading')} />
        ) : empty ? (
          <div className="bitfun-usage-stats__empty" data-bf-part="empty">
            <BarChart3 size={26} aria-hidden />
            <div>
              <h4>{t('empty.title')}</h4>
              <p>{t('empty.description')}</p>
            </div>
          </div>
        ) : stats ? (
          <>
            <div className="bitfun-usage-stats__summary" data-bf-part="summary">
              {summaryCards.map((card) => (
                <div className="bitfun-usage-stats__summary-card" key={card.key}>
                  <span className="bitfun-usage-stats__summary-label">{t(card.key)}</span>
                  <span
                    className={[
                      'bitfun-usage-stats__summary-value',
                      card.highlight && 'bitfun-usage-stats__summary-value--highlight',
                    ].filter(Boolean).join(' ')}
                  >
                    {card.value}
                  </span>
                </div>
              ))}
            </div>

            <div className="bitfun-usage-stats__grid" data-bf-part="distributions">
              <DistributionPanel kind="model" entries={stats.byModel} />
              <DistributionPanel kind="group" entries={stats.byGroup} />
              <DistributionPanel kind="endpoint" entries={stats.byEndpoint} />
              <div className="bitfun-usage-stats__panel" data-bf-part="trendPanel">
                <div className="bitfun-usage-stats__panel-title">{t('trend.title')}</div>
                <TrendChart points={stats.trend} granularity={stats.granularity} />
              </div>
            </div>
          </>
        ) : null}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default UsageStatisticsConfig;
