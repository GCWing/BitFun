import type { CanvasChartDatum, CanvasChartProps, CanvasChartSeries } from './types';

interface NormalizedSeries {
  name: string;
  color: string;
  values: number[];
}

interface NormalizedCartesianSeries {
  labels: string[];
  series: NormalizedSeries[];
}

interface PieSlice {
  label: string;
  value: number;
  color: string;
}

function pieLegend(slices: PieSlice[]): Array<{ name: string; color: string }> {
  return slices.map(slice => ({ name: slice.label, color: slice.color }));
}

const chartColors = [
  'var(--color-accent-500)',
  'var(--color-success)',
  'var(--color-warning)',
  'var(--color-info)',
  'var(--color-accent-600)',
  'var(--color-error)',
  'var(--color-text-muted)',
  'var(--color-accent-400)',
];

function finiteNumber(value: unknown): number | null {
  const number = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(number) ? number : null;
}

function chartLabel(value: unknown, fallback: string): string {
  if (value === undefined || value === null || value === '') return fallback;
  return String(value);
}

function numericObjectKeys(row: unknown, excludedKeys: Set<string>): string[] {
  if (!row || typeof row !== 'object') return [];
  return Object.keys(row).filter(key => !excludedKeys.has(key) && finiteNumber((row as Record<string, unknown>)[key]) !== null);
}

function isObjectRow(row: unknown): row is CanvasChartDatum {
  return Boolean(row && typeof row === 'object' && !Array.isArray(row));
}

function normalizeCartesianSeries(props: CanvasChartProps = {}): NormalizedCartesianSeries {
  const data = Array.isArray(props.data) ? props.data : [];
  const categories = Array.isArray(props.categories)
    ? props.categories.map((item, index) => chartLabel(item, String(index + 1)))
    : [];
  const labelKey = props.labelKey || props.xKey || 'label';
  const valueKey = props.valueKey || props.yKey || 'value';
  const excludedKeys = new Set([labelKey, valueKey, 'label', 'name', 'title', 'category', 'x', 'id']);

  if (Array.isArray(props.series) && props.series.some(item => item && typeof item === 'object' && Array.isArray((item as CanvasChartSeries).data))) {
    const objectSeries = props.series.filter((item): item is CanvasChartSeries => Boolean(item && typeof item === 'object'));
    const length = Math.max(categories.length, ...objectSeries.map(item => (Array.isArray(item.data) ? item.data.length : 0)), data.length);
    const labels = Array.from({ length }, (_, index) => {
      const row = data[index];
      return categories[index] || chartLabel(isObjectRow(row) ? row[labelKey] ?? row.label ?? row.name ?? row.category ?? row.x : undefined, String(index + 1));
    });
    return {
      labels,
      series: objectSeries.map((item, index) => ({
        name: chartLabel(item.name ?? item.label, `Series ${index + 1}`),
        color: item.color || chartColors[index % chartColors.length],
        values: labels.map((_, valueIndex) => finiteNumber(item.data?.[valueIndex]) ?? 0),
      })),
    };
  }

  if (data.some(isObjectRow)) {
    const rows = data.filter(isObjectRow);
    const labels = rows.map((row, index) => categories[index] || chartLabel(row[labelKey] ?? row.label ?? row.name ?? row.category ?? row.x, String(index + 1)));
    const explicitSeries =
      Array.isArray(props.series) && props.series.length
        ? props.series
            .map(item => (typeof item === 'string' ? item : item?.key || chartLabel(item?.name ?? item?.label, '')))
            .filter(Boolean)
        : Array.from(new Set(rows.flatMap(row => numericObjectKeys(row, excludedKeys))));
    const keys = explicitSeries.length ? explicitSeries : [valueKey];
    return {
      labels,
      series: keys.map((key, index) => {
        const seriesMeta = typeof props.series?.[index] === 'object' ? props.series[index] as CanvasChartSeries : undefined;
        return {
          name: chartLabel(key, `Series ${index + 1}`),
          color: seriesMeta?.color || chartColors[index % chartColors.length],
          values: rows.map(row => finiteNumber(row[key]) ?? finiteNumber(row[valueKey]) ?? 0),
        };
      }),
    };
  }

  const labels = data.map((_, index) => categories[index] || String(index + 1));
  return {
    labels,
    series: [
      {
        name: chartLabel(props.name || props.title, 'Value'),
        color: props.color || chartColors[0],
        values: data.map(value => finiteNumber(value) ?? 0),
      },
    ],
  };
}

function chartMaximum(series: NormalizedSeries[]): number {
  const max = Math.max(0, ...series.flatMap(item => item.values.map(value => Math.max(0, value))));
  return max > 0 ? max : 1;
}

function ChartShell({
  title,
  legend = [],
  height = 220,
  style,
  children,
  ...props
}: CanvasChartProps & { legend?: Array<{ name: string; color: string }> }) {
  return (
    <div {...props} className={['bf-chart', props.className].filter(Boolean).join(' ')} style={style}>
      {title || legend.length > 1 ? (
        <div className="bf-chart__header">
          {title ? <div className="bf-chart__title">{title}</div> : <span />}
          {legend.length > 1 ? (
            <div className="bf-chart__legend">
              {legend.map((item, index) => (
                <span key={item.name + index} className="bf-chart__legend-item">
                  <span className="bf-chart__swatch" style={{ background: item.color }} />
                  {item.name}
                </span>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
      <div style={{ height }}>{children}</div>
    </div>
  );
}

function chartGrid(left: number, top: number, plotWidth: number, plotHeight: number, max: number) {
  return Array.from({ length: 5 }, (_, index) => {
    const y = top + plotHeight - (plotHeight * index) / 4;
    const label = Math.round((max * index) / 4);
    return (
      <g key={`grid-${index}`}>
        <line x1={left} y1={y} x2={left + plotWidth} y2={y} stroke="var(--border-subtle)" strokeWidth={1} />
        <text x={left - 8} y={y + 4} textAnchor="end" fill="var(--color-text-muted)" fontSize={10}>
          {label}
        </text>
      </g>
    );
  });
}

export function BarChart(props: CanvasChartProps = {}) {
  const normalized = normalizeCartesianSeries(props);
  const labels = normalized.labels;
  const series = normalized.series.filter(item => item.values.length);
  const width = 720;
  const height = Number(props.height) || 220;
  const left = 38;
  const right = 12;
  const top = 12;
  const bottom = 34;
  const plotWidth = width - left - right;
  const plotHeight = Math.max(40, height - top - bottom);
  const max = chartMaximum(series);
  const groupWidth = labels.length ? plotWidth / labels.length : plotWidth;
  const barGap = 4;
  const barWidth = Math.max(2, (groupWidth - 10 - Math.max(0, series.length - 1) * barGap) / Math.max(1, series.length));
  return (
    <ChartShell title={props.title} legend={series} height={height} style={props.style}>
      <svg viewBox={`0 0 ${width} ${height}`} height={height} role="img" aria-label={String(props.title || 'Bar chart')}>
        {chartGrid(left, top, plotWidth, plotHeight, max)}
        {labels.flatMap((_, labelIndex) =>
          series.map((item, seriesIndex) => {
            const value = Math.max(0, item.values[labelIndex] ?? 0);
            const barHeight = (value / max) * plotHeight;
            const x = left + labelIndex * groupWidth + 5 + seriesIndex * (barWidth + barGap);
            const y = top + plotHeight - barHeight;
            return <rect key={`${item.name}-${labelIndex}`} x={x} y={y} width={barWidth} height={barHeight} rx={3} fill={item.color} />;
          }),
        )}
        {labels.map((label, index) => (
          <text key={`label-${index}`} x={left + index * groupWidth + groupWidth / 2} y={height - 10} textAnchor="middle" fill="var(--color-text-muted)" fontSize={10}>
            {String(label).slice(0, 14)}
          </text>
        ))}
      </svg>
    </ChartShell>
  );
}

export function LineChart(props: CanvasChartProps = {}) {
  const normalized = normalizeCartesianSeries(props);
  const labels = normalized.labels;
  const series = normalized.series.filter(item => item.values.length);
  const width = 720;
  const height = Number(props.height) || 220;
  const left = 38;
  const right = 14;
  const top = 14;
  const bottom = 34;
  const plotWidth = width - left - right;
  const plotHeight = Math.max(40, height - top - bottom);
  const max = chartMaximum(series);
  const xFor = (index: number) => left + (labels.length <= 1 ? plotWidth / 2 : (index * plotWidth) / (labels.length - 1));
  const yFor = (value: number) => top + plotHeight - (Math.max(0, value) / max) * plotHeight;
  return (
    <ChartShell title={props.title} legend={series} height={height} style={props.style}>
      <svg viewBox={`0 0 ${width} ${height}`} height={height} role="img" aria-label={String(props.title || 'Line chart')}>
        {chartGrid(left, top, plotWidth, plotHeight, max)}
        {series.map(item => {
          const d = item.values.map((value, index) => `${index === 0 ? 'M ' : 'L '}${xFor(index)} ${yFor(value)}`).join(' ');
          return <path key={`line-${item.name}`} d={d} fill="none" stroke={item.color} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />;
        })}
        {series.flatMap(item =>
          item.values.map((value, index) => (
            <circle key={`point-${item.name}-${index}`} cx={xFor(index)} cy={yFor(value)} r={3} fill={item.color} stroke="var(--color-bg-secondary)" strokeWidth={1.5} />
          )),
        )}
        {labels.map((label, index) => (
          <text key={`label-${index}`} x={xFor(index)} y={height - 10} textAnchor="middle" fill="var(--color-text-muted)" fontSize={10}>
            {String(label).slice(0, 14)}
          </text>
        ))}
      </svg>
    </ChartShell>
  );
}

function normalizePieSlices(props: CanvasChartProps = {}): PieSlice[] {
  const data = Array.isArray(props.data) ? props.data : [];
  const labelKey = props.labelKey || props.nameKey || 'label';
  const valueKey = props.valueKey || 'value';
  if (Array.isArray(props.series) && props.series.length && props.series.every(item => item && typeof item === 'object')) {
    return (props.series as CanvasChartSeries[])
      .map((item, index) => ({
        label: chartLabel(item.name ?? item.label, `Slice ${index + 1}`),
        value: Math.max(0, finiteNumber(item.value ?? item.data) ?? 0),
        color: item.color || chartColors[index % chartColors.length],
      }))
      .filter(item => item.value > 0);
  }
  return data
    .map((item, index) => {
      if (isObjectRow(item)) {
        return {
          label: chartLabel(item[labelKey] ?? item.name ?? item.category, `Slice ${index + 1}`),
          value: Math.max(0, finiteNumber(item[valueKey]) ?? 0),
          color: item.color || chartColors[index % chartColors.length],
        };
      }
      return {
        label: Array.isArray(props.categories) ? chartLabel(props.categories[index], `Slice ${index + 1}`) : `Slice ${index + 1}`,
        value: Math.max(0, finiteNumber(item) ?? 0),
        color: chartColors[index % chartColors.length],
      };
    })
    .filter(item => item.value > 0);
}

function arcPath(cx: number, cy: number, radius: number, startAngle: number, endAngle: number): string {
  const startX = cx + radius * Math.cos(startAngle);
  const startY = cy + radius * Math.sin(startAngle);
  const endX = cx + radius * Math.cos(endAngle);
  const endY = cy + radius * Math.sin(endAngle);
  const largeArc = endAngle - startAngle > Math.PI ? 1 : 0;
  return `M ${cx} ${cy} L ${startX} ${startY} A ${radius} ${radius} 0 ${largeArc} 1 ${endX} ${endY} Z`;
}

export function PieChart(props: CanvasChartProps = {}) {
  const slices = normalizePieSlices(props);
  const width = 360;
  const height = Number(props.height) || 220;
  const radius = Math.min(82, (height - 24) / 2);
  const cx = 112;
  const cy = height / 2;
  const total = slices.reduce((sum, item) => sum + item.value, 0) || 1;
  let angle = -Math.PI / 2;
  const paths = slices.map((item, index) => {
    const nextAngle = angle + (item.value / total) * Math.PI * 2;
    const path = arcPath(cx, cy, radius, angle, nextAngle);
    angle = nextAngle;
    return <path key={item.label + index} d={path} fill={item.color} stroke="var(--color-bg-secondary)" strokeWidth={1.5} />;
  });
  return (
    <ChartShell title={props.title} legend={pieLegend(slices)} height={height} style={props.style}>
      <svg viewBox={`0 0 ${width} ${height}`} height={height} role="img" aria-label={String(props.title || 'Pie chart')}>
        {paths}
        {slices.slice(0, 6).map((item, index) => (
          <g key={`label-${index}`} transform={`translate(225 ${52 + index * 23})`}>
            <rect x={0} y={-8} width={9} height={9} rx={2} fill={item.color} />
            <text x={16} y={0} fill="var(--color-text-secondary)" fontSize={11}>
              {String(item.label).slice(0, 20)}
            </text>
            <text x={118} y={0} fill="var(--color-text-muted)" fontSize={11} textAnchor="end">
              {Math.round((item.value / total) * 100)}%
            </text>
          </g>
        ))}
      </svg>
    </ChartShell>
  );
}
