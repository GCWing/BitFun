import { categoryColor, usageColorSequence, toneColor } from './style';
import { Alert as DesignAlert, Disclosure as DesignDisclosure } from '@openbitfun/ui';
import { useCanvasState } from './hooks';
import { normalizeDiffLines } from './diffLines';
import type {
  CanvasAlertProps,
  CanvasCalloutProps,
  CanvasCollapsibleSectionProps,
  CanvasDiffStatsProps,
  CanvasDiffViewProps,
  CanvasFileTreeItem,
  CanvasFileTreeProps,
  CanvasKeyValueItem,
  CanvasKeyValueListProps,
  CanvasProgressBarProps,
  CanvasStatProps,
  CanvasSwatchProps,
  CanvasTableProps,
  CanvasTimelineProps,
  CanvasTodoItem,
  CanvasTodoListCardProps,
  CanvasTodoListProps,
  CanvasUsageBarProps,
} from './types';
import { canvasArrayProp } from './runtimeValidation';

export function Callout({ children, tone = 'info', title, style, ...props }: CanvasCalloutProps) {
  const accent = toneColor(tone);
  return (
    <section
      {...props}
      className={['openbitfun-callout', props.className].filter(Boolean).join(' ')}
      style={{ '--openbitfun-callout-accent': accent, ...style } as React.CSSProperties}
    >
      {title ? (
        <div className="openbitfun-callout-title">
          {title}
        </div>
      ) : null}
      <div className="openbitfun-callout-body">{children}</div>
    </section>
  );
}

function alertTone(type: CanvasAlertProps['type'], tone: CanvasAlertProps['tone']) {
  if (tone === 'danger' || tone === 'error') return 'error';
  if (tone === 'success' || tone === 'warning' || tone === 'info') return tone;
  return type || 'info';
}

export function Alert({
  children,
  type = 'info',
  tone,
  title,
  message,
  description,
  showIcon = true,
  style,
  ...props
}: CanvasAlertProps) {
  return (
    <DesignAlert
      {...props}
      className={['openbitfun-alert', props.className].filter(Boolean).join(' ')}
      description={description}
      message={message ?? children ?? ''}
      showIcon={showIcon}
      style={style}
      title={title}
      tone={alertTone(type, tone)}
    />
  );
}

export function Stat({ value, label, tone, style, ...props }: CanvasStatProps) {
  return (
    <div {...props} style={{ display: 'grid', gap: 3, ...style }}>
      <strong
        style={{
          color: toneColor(tone),
          fontSize: 'var(--openbitfun-type-heading-dialog-font-size)',
          fontWeight: 'var(--openbitfun-type-label-selected-font-weight)',
          lineHeight: 'var(--openbitfun-type-display-sm-line-height)',
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        {value}
      </strong>
      <span style={{ color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>{label}</span>
    </div>
  );
}

export function Table({
  headers = [],
  rows = [],
  columnAlign = [],
  rowTone = [],
  framed = true,
  striped = false,
  stickyHeader = false,
  emptyMessage = 'No rows',
  style,
  ...props
}: CanvasTableProps) {
  const safeHeaders = canvasArrayProp<React.ReactNode>('Table', 'headers', headers);
  const safeRows = canvasArrayProp<React.ReactNode[]>('Table', 'rows', rows);
  const safeColumnAlign = canvasArrayProp<React.CSSProperties['textAlign']>('Table', 'columnAlign', columnAlign);
  const safeRowTone = canvasArrayProp<NonNullable<CanvasTableProps['rowTone']>[number]>('Table', 'rowTone', rowTone);
  const table = (
    <table className="openbitfun-table">
      <thead>
        <tr>
          {safeHeaders.map((header, index) => (
            <th
              key={index}
              style={{
                textAlign: safeColumnAlign[index] || 'left',
                position: stickyHeader ? 'sticky' : undefined,
                top: stickyHeader ? 0 : undefined,
              }}
            >
              {header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {safeRows.length ? (
          safeRows.map((row, rowIndex) => (
            <tr
              key={rowIndex}
              style={{
                background: striped && rowIndex % 2 === 1 ? 'var(--openbitfun-color-surface-subtle)' : undefined,
              }}
            >
              {safeHeaders.map((_, index) => (
                <td key={index} style={{ textAlign: safeColumnAlign[index] || 'left' }}>
                  {index === 0 && safeRowTone[rowIndex] ? (
                    <span
                      style={{
                        display: 'inline-block',
                        width: 6,
                        height: 6,
                        borderRadius: 99,
                        marginRight: 7,
                        background: toneColor(safeRowTone[rowIndex]),
                        verticalAlign: 'middle',
                      }}
                    />
                  ) : null}
                  {row[index] ?? ''}
                </td>
              ))}
            </tr>
          ))
        ) : (
          <tr>
            <td colSpan={safeHeaders.length || 1} style={{ color: 'var(--openbitfun-color-content-muted)' }}>
              {emptyMessage}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );

  return framed ? (
    <div {...props} className={['openbitfun-table-wrap', props.className].filter(Boolean).join(' ')} style={style}>
      {table}
    </div>
  ) : (
    <div {...props} style={style}>
      {table}
    </div>
  );
}

export function CollapsibleSection({
  title,
  leading,
  count,
  trailing,
  children,
  defaultOpen = false,
  open,
  onOpenChange,
  style,
  ...props
}: CanvasCollapsibleSectionProps) {
  const [storedOpen, setStoredOpen] = useCanvasState(`collapsible:${String(title ?? '')}`, Boolean(defaultOpen));
  const isOpen = open ?? storedOpen;
  const handleOpenChange = (nextOpen: boolean) => {
    setStoredOpen(nextOpen);
    onOpenChange?.(nextOpen);
  };

  return (
    <DesignDisclosure
      {...props}
      actions={trailing}
      className={['openbitfun-collapsible-section', props.className].filter(Boolean).join(' ')}
      leading={leading}
      onOpenChange={handleOpenChange}
      open={isOpen}
      style={style}
      summary={(
        <>
          {title}
          {count !== undefined ? <span className="openbitfun-collapsible-section__count">{count}</span> : null}
        </>
      )}
    >
      {children}
    </DesignDisclosure>
  );
}

export function DiffStats({ additions = 0, deletions = 0, style, ...props }: CanvasDiffStatsProps) {
  const addCount = Math.abs(Number(additions) || 0);
  const delCount = Math.abs(Number(deletions) || 0);
  if (!addCount && !delCount) return null;
  return (
    <span
      {...props}
      style={{
        display: 'inline-flex',
        gap: 7,
        alignItems: 'center',
        fontSize: 'var(--openbitfun-type-support-font-size)',
        fontVariantNumeric: 'tabular-nums',
        ...style,
      }}
    >
      {addCount ? <span style={{ color: 'var(--openbitfun-color-code-change-added)' }}>+{addCount}</span> : null}
      {delCount ? <span style={{ color: 'var(--openbitfun-color-code-change-removed)' }}>-{delCount}</span> : null}
    </span>
  );
}

export function DiffView({
  lines = [],
  showLineNumbers = true,
  coloredLineNumbers = true,
  showAccentStrip = true,
  style,
  ...props
}: CanvasDiffViewProps) {
  return (
    <div {...props} className={['openbitfun-diff', props.className].filter(Boolean).join(' ')} style={style}>
      {normalizeDiffLines(lines).map((line, index) => {
        const type = line?.type;
        const accent =
          type === 'added' || type === 'addition'
            ? 'var(--openbitfun-color-code-change-added)'
            : type === 'removed' || type === 'removal'
              ? 'var(--openbitfun-color-code-change-removed)'
              : 'transparent';
        const bg =
          accent === 'var(--openbitfun-color-code-change-added)'
            ? 'color-mix(in srgb, var(--openbitfun-color-code-change-added) 12%, transparent)'
            : accent === 'var(--openbitfun-color-code-change-removed)'
              ? 'color-mix(in srgb, var(--openbitfun-color-code-change-removed) 12%, transparent)'
              : 'transparent';
        return (
          <div
            key={index}
            style={{
              display: 'grid',
              gridTemplateColumns: `${showAccentStrip ? '3px ' : ''}${showLineNumbers ? '52px ' : ''}18px minmax(0,1fr)`,
              minWidth: '100%',
              background: bg,
              whiteSpace: 'pre',
            }}
          >
            {showAccentStrip ? <span style={{ background: accent }} /> : null}
            {showLineNumbers ? (
              <span
                style={{
                  color: coloredLineNumbers && accent !== 'transparent' ? accent : 'var(--openbitfun-color-content-muted)',
                  textAlign: 'right',
                  padding: '0 8px',
                  userSelect: 'none',
                }}
              >
                {line?.lineNumber ?? index + 1}
              </span>
            ) : null}
            <span
              style={{
                color: accent === 'transparent' ? 'var(--openbitfun-color-content-muted)' : accent,
                userSelect: 'none',
              }}
            >
              {accent === 'var(--openbitfun-color-code-change-added)' ? '+' : accent === 'var(--openbitfun-color-code-change-removed)' ? '-' : ' '}
            </span>
            <span style={{ paddingRight: 10, color: 'var(--openbitfun-color-content-primary)' }}>
              {line?.content || ''}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function normalizeKeyValueItems(items: CanvasKeyValueListProps['items']): CanvasKeyValueItem[] {
  if (Array.isArray(items)) return items;
  if (items && typeof items === 'object') {
    return Object.entries(items).map(([label, value]) => ({ key: label, label, value }));
  }
  return [];
}

export function KeyValueList({
  items,
  columns = 1,
  compact = false,
  emptyMessage = 'No details',
  style,
  ...props
}: CanvasKeyValueListProps) {
  const entries = normalizeKeyValueItems(items);
  const columnCount = Math.max(1, Math.min(4, Math.floor(Number(columns) || 1)));

  return (
    <dl
      {...props}
      className={['openbitfun-key-value-list', props.className].filter(Boolean).join(' ')}
      style={{
        display: 'grid',
        gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`,
        gap: compact ? 6 : 10,
        margin: 0,
        ...style,
      }}
    >
      {entries.length ? (
        entries.map((item, index) => (
          <div
            key={item.key ?? index}
            style={{
              minWidth: 0,
              padding: compact ? '0 0 6px' : '8px 0',
              borderBottom: '1px solid var(--openbitfun-color-border-subtle)',
            }}
          >
            <dt
              style={{
                margin: 0,
                color: 'var(--openbitfun-color-content-muted)',
                fontSize: 'var(--openbitfun-type-meta-font-size)',
                lineHeight: 'var(--openbitfun-type-meta-line-height)',
              }}
            >
              {item.label}
            </dt>
            <dd
              style={{
                margin: '2px 0 0',
                color: toneColor(item.tone),
                fontSize: compact ? 'var(--openbitfun-type-body-xs-font-size)' : 'var(--openbitfun-type-body-sm-font-size)',
                fontWeight: 'var(--openbitfun-type-label-selected-font-weight)',
                lineHeight: 'var(--openbitfun-type-meta-line-height)',
                overflowWrap: 'anywhere',
              }}
            >
              {item.value}
            </dd>
          </div>
        ))
      ) : (
        <div style={{ color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>{emptyMessage}</div>
      )}
    </dl>
  );
}

export function Timeline({
  items = [],
  emptyMessage = 'No events',
  style,
  ...props
}: CanvasTimelineProps) {
  const safeItems = canvasArrayProp<NonNullable<CanvasTimelineProps['items']>[number]>('Timeline', 'items', items);
  return (
    <ol
      {...props}
      className={['openbitfun-timeline', props.className].filter(Boolean).join(' ')}
      style={{ display: 'grid', gap: 10, margin: 0, padding: 0, listStyle: 'none', ...style }}
    >
      {safeItems.length ? (
        safeItems.map((item, index) => {
          const color = toneColor(item.tone);
          return (
            <li
              key={item.key ?? index}
              style={{
                display: 'grid',
                gridTemplateColumns: '18px minmax(0, 1fr)',
                gap: 9,
                minWidth: 0,
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  display: 'grid',
                  placeItems: 'center',
                  width: 18,
                  height: 18,
                  marginTop: 1,
                  borderRadius: 999,
                  background: 'color-mix(in srgb, currentColor 16%, transparent)',
                  color,
                  fontSize: 'var(--openbitfun-type-micro-font-size)',
                  fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
                }}
              >
                {item.icon ?? ''}
              </span>
              <span style={{ minWidth: 0, display: 'grid', gap: 2 }}>
                <span
                  style={{
                    display: 'flex',
                    gap: 8,
                    alignItems: 'baseline',
                    justifyContent: 'space-between',
                    minWidth: 0,
                  }}
                >
                  <strong style={{ minWidth: 0, color: 'var(--openbitfun-color-content-primary)', fontSize: 'var(--openbitfun-type-body-sm-font-size)' }}>
                    {item.title}
                  </strong>
                  {item.time ? (
                    <time style={{ flex: '0 0 auto', color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-meta-font-size)' }}>
                      {item.time}
                    </time>
                  ) : null}
                </span>
                {item.description ? (
                  <span style={{ color: 'var(--openbitfun-color-content-secondary)', fontSize: 'var(--openbitfun-type-support-font-size)', overflowWrap: 'anywhere' }}>
                    {item.description}
                  </span>
                ) : null}
              </span>
            </li>
          );
        })
      ) : (
        <li style={{ color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>{emptyMessage}</li>
      )}
    </ol>
  );
}

function fileTreeKey(item: CanvasFileTreeItem, index: number, depth: number) {
  return item.key ?? item.path ?? `${depth}-${index}-${String(item.name ?? '')}`;
}

function renderFileTreeItems(items: CanvasFileTreeItem[], depth: number, defaultExpanded: boolean) {
  return items.map((item, index) => {
    const children = Array.isArray(item.children) ? item.children : [];
    const isFolder = item.type === 'folder' || children.length > 0;
    const row = (
      <span
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 7,
          minWidth: 0,
          padding: '3px 0',
          paddingLeft: depth * 16,
        }}
      >
        <span style={{ flex: '0 0 auto', width: 14, color: isFolder ? 'var(--openbitfun-color-accent-default)' : 'var(--openbitfun-color-content-muted)' }}>
          {isFolder ? '▸' : '•'}
        </span>
        <span
          style={{
            minWidth: 0,
            color: toneColor(item.tone),
            fontFamily: 'var(--openbitfun-type-code-sm-font-family)',
            fontSize: 'var(--openbitfun-type-code-sm-font-size)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {item.name ?? item.path}
        </span>
        {item.meta ? (
          <span style={{ flex: '0 0 auto', marginLeft: 'auto', color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-meta-font-size)' }}>
            {item.meta}
          </span>
        ) : null}
      </span>
    );

    if (!isFolder) {
      return <div key={fileTreeKey(item, index, depth)}>{row}</div>;
    }

    return (
      <details key={fileTreeKey(item, index, depth)} open={defaultExpanded}>
        <summary style={{ display: 'block', cursor: 'default', listStyle: 'none' }}>{row}</summary>
        {children.length ? renderFileTreeItems(children, depth + 1, defaultExpanded) : null}
      </details>
    );
  });
}

export function FileTree({
  items = [],
  defaultExpanded = true,
  emptyMessage = 'No files',
  style,
  ...props
}: CanvasFileTreeProps) {
  const safeItems = canvasArrayProp<CanvasFileTreeItem>('FileTree', 'items', items);
  return (
    <div
      {...props}
      className={['openbitfun-file-tree', props.className].filter(Boolean).join(' ')}
      style={{
        minWidth: 0,
        overflow: 'auto',
        border: '1px solid var(--openbitfun-color-border-subtle)',
        borderRadius: 8,
        padding: '8px 10px',
        background: 'color-mix(in srgb, var(--openbitfun-color-surface-panel) 70%, transparent)',
        ...style,
      }}
    >
      {safeItems.length ? renderFileTreeItems([...safeItems], 0, defaultExpanded) : (
        <div style={{ color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>{emptyMessage}</div>
      )}
    </div>
  );
}

export function ProgressBar({
  value = 0,
  max = 100,
  label,
  tone = 'primary',
  showValue = true,
  style,
  ...props
}: CanvasProgressBarProps) {
  const safeMax = Math.max(1, Number(max) || 100);
  const safeValue = Math.max(0, Math.min(safeMax, Number(value) || 0));
  const percent = Math.round((safeValue / safeMax) * 100);

  return (
    <div {...props} className={['openbitfun-progress', props.className].filter(Boolean).join(' ')} style={style}>
      {label || showValue ? (
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            gap: 10,
            marginBottom: 5,
            color: 'var(--openbitfun-color-content-secondary)',
            fontSize: 'var(--openbitfun-type-support-font-size)',
          }}
        >
          <span>{label}</span>
          {showValue ? <span style={{ fontVariantNumeric: 'tabular-nums' }}>{percent}%</span> : null}
        </div>
      ) : null}
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={safeMax}
        aria-valuenow={safeValue}
        style={{
          height: 8,
          overflow: 'hidden',
          borderRadius: 999,
          background: 'var(--openbitfun-color-action-neutral-surface-hover)',
        }}
      >
        <div
          style={{
            width: `${percent}%`,
            height: '100%',
            borderRadius: 999,
            background: toneColor(tone),
          }}
        />
      </div>
    </div>
  );
}

export function Swatch({
  color = 'gray',
  style,
  ...props
}: CanvasSwatchProps) {
  return (
    <span
      {...props}
      className={['openbitfun-swatch', props.className].filter(Boolean).join(' ')}
      aria-hidden={props['aria-label'] ? undefined : true}
      style={{
        display: 'inline-block',
        width: 12,
        height: 12,
        borderRadius: 3,
        background: categoryColor(color),
        border: '1px solid var(--openbitfun-color-border-subtle)',
        flex: '0 0 auto',
        ...style,
      }}
    />
  );
}

function positiveSegmentValue(value: unknown): number {
  const next = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(next) && next > 0 ? next : 0;
}

export function UsageBar({
  segments = [],
  total = 0,
  topLeftLabel,
  topRightLabel,
  style,
  ...props
}: CanvasUsageBarProps) {
  const normalized = canvasArrayProp<NonNullable<CanvasUsageBarProps['segments']>[number]>(
    'UsageBar',
    'segments',
    segments,
  ).map((segment, index) => ({
    ...segment,
    value: positiveSegmentValue(segment.value),
    color: segment.color || usageColorSequence[index % usageColorSequence.length],
  }));
  const segmentTotal = normalized.reduce((sum, segment) => sum + segment.value, 0);
  const safeTotal = Math.max(positiveSegmentValue(total), segmentTotal, 1);
  const remainder = Math.max(0, safeTotal - segmentTotal);

  return (
    <div {...props} className={['openbitfun-usage-bar', props.className].filter(Boolean).join(' ')} style={style}>
      {topLeftLabel || topRightLabel ? (
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            gap: 12,
            marginBottom: 6,
            color: 'var(--openbitfun-color-content-secondary)',
            fontSize: 'var(--openbitfun-type-support-font-size)',
            lineHeight: 'var(--openbitfun-type-meta-line-height)',
          }}
        >
          <span>{topLeftLabel}</span>
          <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>{topRightLabel}</span>
        </div>
      ) : null}
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={safeTotal}
        aria-valuenow={Math.min(segmentTotal, safeTotal)}
        style={{
          display: 'flex',
          gap: 2,
          height: 10,
          overflow: 'hidden',
          borderRadius: 999,
          background: 'var(--openbitfun-color-action-neutral-surface-hover)',
          padding: 1,
        }}
      >
        {normalized.map((segment, index) => {
          if (segment.value <= 0) return null;
          return (
            <span
              key={segment.id || index}
              title={`${segment.id}: ${segment.value}`}
              style={{
                flex: `${segment.value} 1 0`,
                minWidth: 2,
                borderRadius: 999,
                background: categoryColor(segment.color, index),
              }}
            />
          );
        })}
        {remainder > 0 ? (
          <span
            aria-hidden="true"
            style={{
              flex: `${remainder} 1 0`,
              minWidth: 2,
              borderRadius: 999,
              background: 'var(--openbitfun-color-action-quiet-hover)',
            }}
          />
        ) : null}
      </div>
    </div>
  );
}

function todoStatusColor(status: CanvasTodoItem['status']) {
  if (status === 'completed') return 'var(--openbitfun-color-status-success-content)';
  if (status === 'in_progress') return 'var(--openbitfun-color-status-warning-content)';
  if (status === 'cancelled') return 'var(--openbitfun-color-content-muted)';
  return 'var(--openbitfun-color-content-muted)';
}

function todoStatusLabel(status: CanvasTodoItem['status']) {
  if (status === 'completed') return 'completed';
  if (status === 'in_progress') return 'in progress';
  if (status === 'cancelled') return 'cancelled';
  return 'pending';
}

function dimmedTodoSet(value: CanvasTodoListProps['dimmedTodoIds']): ReadonlySet<string> {
  if (!value) return new Set();
  return value instanceof Set ? value : new Set(value);
}

function TodoMarker({ status }: { status: CanvasTodoItem['status'] }) {
  const color = todoStatusColor(status);
  const isCompleted = status === 'completed';
  return (
    <span
      aria-hidden="true"
      style={{
        width: 14,
        height: 14,
        marginTop: 2,
        flex: '0 0 auto',
        display: 'inline-grid',
        placeItems: 'center',
        borderRadius: status === 'in_progress' ? 999 : 3,
        border: `1.5px solid ${color}`,
        background: isCompleted ? color : 'transparent',
        color: 'var(--openbitfun-color-surface-canvas)',
        fontSize: 'var(--openbitfun-type-micro-font-size)',
        lineHeight: 'var(--openbitfun-type-modifier-leading-none-line-height)',
        fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
      }}
    >
      {isCompleted ? '✓' : ''}
    </span>
  );
}

export function TodoList({
  todos = [],
  dimmedTodoIds,
  onTodoClick,
  style,
  ...props
}: CanvasTodoListProps) {
  const safeTodos = canvasArrayProp<CanvasTodoItem>('TodoList', 'todos', todos);
  if (!safeTodos.length) return null;
  const dimmed = dimmedTodoSet(dimmedTodoIds);

  return (
    <div
      {...props}
      className={['openbitfun-todo-list', props.className].filter(Boolean).join(' ')}
      style={{
        display: 'grid',
        gap: 4,
        ...style,
      }}
    >
      {safeTodos.map((todo) => {
        const content = todo.content || todo.id;
        const isDimmed = dimmed.has(todo.id);
        const rowStyle = {
          width: '100%',
          display: 'grid',
          gridTemplateColumns: '18px minmax(0, 1fr)',
          gap: 8,
          alignItems: 'start',
          border: 0,
          borderRadius: 6,
          padding: '6px 7px',
          background: 'transparent',
          color: 'var(--openbitfun-color-content-primary)',
          font: 'inherit',
          textAlign: 'left' as const,
          opacity: isDimmed ? 0.5 : 1,
          cursor: onTodoClick ? 'pointer' : 'default',
        };
        const body = (
          <>
            <TodoMarker status={todo.status} />
            <span style={{ minWidth: 0, display: 'grid', gap: 2 }}>
              <span
                style={{
                  color: todo.status === 'completed' ? 'var(--openbitfun-color-content-secondary)' : 'var(--openbitfun-color-content-primary)',
                  fontSize: 'var(--openbitfun-type-support-font-size)',
                  lineHeight: 'var(--openbitfun-type-support-line-height)',
                  textDecoration: todo.status === 'completed' ? 'line-through' : undefined,
                  overflowWrap: 'anywhere',
                }}
              >
                {content}
              </span>
              <span style={{ color: todoStatusColor(todo.status), fontSize: 'var(--openbitfun-type-micro-font-size)', lineHeight: 'var(--openbitfun-type-micro-line-height)' }}>
                {todoStatusLabel(todo.status)}
              </span>
            </span>
          </>
        );
        return onTodoClick ? (
          <button
            key={todo.id}
            type="button"
            onClick={() => onTodoClick(todo)}
            style={rowStyle}
          >
            {body}
          </button>
        ) : (
          <div key={todo.id} style={rowStyle}>
            {body}
          </div>
        );
      })}
    </div>
  );
}

export function TodoListCard({
  todos = [],
  dimmedTodoIds,
  defaultExpanded = false,
  onTodoClick,
  style,
  ...props
}: CanvasTodoListCardProps) {
  const safeTodos = canvasArrayProp<CanvasTodoItem>('TodoListCard', 'todos', todos);
  const completed = safeTodos.filter(todo => todo.status === 'completed').length;
  const key = `todo-list-card:${safeTodos.map(todo => todo.id).join('|')}`;
  const [open, setOpen] = useCanvasState(key, Boolean(defaultExpanded));
  if (!safeTodos.length) return null;

  return (
    <section
      {...props}
      className={['openbitfun-todo-list-card', props.className].filter(Boolean).join(' ')}
      style={{
        border: '1px solid var(--openbitfun-color-border-subtle)',
        borderRadius: 8,
        background: 'var(--openbitfun-color-surface-raised)',
        overflow: 'hidden',
        ...style,
      }}
    >
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        style={{
          width: '100%',
          minHeight: 34,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          border: 0,
          borderBottom: open ? '1px solid var(--openbitfun-color-border-subtle)' : 0,
          background: 'transparent',
          color: 'var(--openbitfun-color-content-primary)',
          padding: '8px 10px',
          font: 'inherit',
          cursor: 'pointer',
          textAlign: 'left',
        }}
      >
        <span
          aria-hidden="true"
          style={{
            color: 'var(--openbitfun-color-content-muted)',
            transform: open ? 'rotate(90deg)' : 'rotate(0deg)',
          }}
        >
          ›
        </span>
        <span style={{ fontWeight: 'var(--openbitfun-type-label-selected-font-weight)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>Tasks</span>
        <span style={{ marginLeft: 'auto', color: 'var(--openbitfun-color-content-muted)', fontSize: 'var(--openbitfun-type-support-font-size)' }}>
          {completed}/{safeTodos.length} done
        </span>
      </button>
      {open ? (
        <div style={{ padding: 8 }}>
          <TodoList todos={safeTodos} dimmedTodoIds={dimmedTodoIds} onTodoClick={onTodoClick} />
        </div>
      ) : null}
    </section>
  );
}
