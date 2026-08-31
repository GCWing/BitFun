import { categoryColor, usageColorSequence, toneColor } from './style';
import { Alert as DesignAlert, Disclosure as DesignDisclosure } from '@bitfun/ui';
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

export function Callout({ children, tone = 'info', title, style, ...props }: CanvasCalloutProps) {
  const accent = toneColor(tone);
  return (
    <section
      {...props}
      className={['bf-callout', props.className].filter(Boolean).join(' ')}
      style={{ '--bf-callout-accent': accent, ...style } as React.CSSProperties}
    >
      {title ? (
        <div className="bf-callout-title">
          {title}
        </div>
      ) : null}
      <div className="bf-callout-body">{children}</div>
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
      className={['bf-alert', props.className].filter(Boolean).join(' ')}
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
          fontSize: 'var(--bf-font-size-3xl)',
          fontWeight: 'var(--bf-font-weight-semibold)',
          lineHeight: 'var(--bf-line-height-display)',
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        {value}
      </strong>
      <span style={{ color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-support-font-size)' }}>{label}</span>
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
  const table = (
    <table className="bf-table">
      <thead>
        <tr>
          {headers.map((header, index) => (
            <th
              key={index}
              style={{
                textAlign: columnAlign[index] || 'left',
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
        {rows.length ? (
          rows.map((row, rowIndex) => (
            <tr
              key={rowIndex}
              style={{
                background: striped && rowIndex % 2 === 1 ? 'var(--bf-color-surface-subtle)' : undefined,
              }}
            >
              {headers.map((_, index) => (
                <td key={index} style={{ textAlign: columnAlign[index] || 'left' }}>
                  {index === 0 && rowTone[rowIndex] ? (
                    <span
                      style={{
                        display: 'inline-block',
                        width: 6,
                        height: 6,
                        borderRadius: 99,
                        marginRight: 7,
                        background: toneColor(rowTone[rowIndex]),
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
            <td colSpan={headers.length || 1} style={{ color: 'var(--bf-color-content-muted)' }}>
              {emptyMessage}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );

  return framed ? (
    <div {...props} className={['bf-table-wrap', props.className].filter(Boolean).join(' ')} style={style}>
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
      className={['bf-collapsible-section', props.className].filter(Boolean).join(' ')}
      leading={leading}
      onOpenChange={handleOpenChange}
      open={isOpen}
      style={style}
      summary={(
        <>
          {title}
          {count !== undefined ? <span className="bf-collapsible-section__count">{count}</span> : null}
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
        fontSize: 'var(--bf-type-support-font-size)',
        fontVariantNumeric: 'tabular-nums',
        ...style,
      }}
    >
      {addCount ? <span style={{ color: 'var(--bf-color-status-success-content)' }}>+{addCount}</span> : null}
      {delCount ? <span style={{ color: 'var(--bf-color-status-danger-content)' }}>-{delCount}</span> : null}
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
    <div {...props} className={['bf-diff', props.className].filter(Boolean).join(' ')} style={style}>
      {normalizeDiffLines(lines).map((line, index) => {
        const type = line?.type;
        const accent =
          type === 'added' || type === 'addition'
            ? 'var(--bf-color-status-success-content)'
            : type === 'removed' || type === 'removal'
              ? 'var(--bf-color-status-danger-content)'
              : 'transparent';
        const bg =
          accent === 'var(--bf-color-status-success-content)'
            ? 'color-mix(in srgb, var(--bf-color-status-success-content) 12%, transparent)'
            : accent === 'var(--bf-color-status-danger-content)'
              ? 'color-mix(in srgb, var(--bf-color-status-danger-content) 12%, transparent)'
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
                  color: coloredLineNumbers && accent !== 'transparent' ? accent : 'var(--bf-color-content-muted)',
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
                color: accent === 'transparent' ? 'var(--bf-color-content-muted)' : accent,
                userSelect: 'none',
              }}
            >
              {accent === 'var(--bf-color-status-success-content)' ? '+' : accent === 'var(--bf-color-status-danger-content)' ? '-' : ' '}
            </span>
            <span style={{ paddingRight: 10, color: 'var(--bf-color-content-primary)' }}>
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
      className={['bf-key-value-list', props.className].filter(Boolean).join(' ')}
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
              borderBottom: '1px solid var(--bf-color-border-subtle)',
            }}
          >
            <dt
              style={{
                margin: 0,
                color: 'var(--bf-color-content-muted)',
                fontSize: 'var(--bf-type-meta-font-size)',
                lineHeight: 'var(--bf-type-meta-line-height)',
              }}
            >
              {item.label}
            </dt>
            <dd
              style={{
                margin: '2px 0 0',
                color: toneColor(item.tone),
                fontSize: compact ? 'var(--bf-type-body-xs-font-size)' : 'var(--bf-type-body-sm-font-size)',
                fontWeight: 'var(--bf-font-weight-semibold)',
                lineHeight: 'var(--bf-line-height-compact)',
                overflowWrap: 'anywhere',
              }}
            >
              {item.value}
            </dd>
          </div>
        ))
      ) : (
        <div style={{ color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-support-font-size)' }}>{emptyMessage}</div>
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
  return (
    <ol
      {...props}
      className={['bf-timeline', props.className].filter(Boolean).join(' ')}
      style={{ display: 'grid', gap: 10, margin: 0, padding: 0, listStyle: 'none', ...style }}
    >
      {items.length ? (
        items.map((item, index) => {
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
                  fontSize: 'var(--bf-type-micro-font-size)',
                  fontWeight: 'var(--bf-font-weight-bold)',
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
                  <strong style={{ minWidth: 0, color: 'var(--bf-color-content-primary)', fontSize: 'var(--bf-type-body-sm-font-size)' }}>
                    {item.title}
                  </strong>
                  {item.time ? (
                    <time style={{ flex: '0 0 auto', color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-meta-font-size)' }}>
                      {item.time}
                    </time>
                  ) : null}
                </span>
                {item.description ? (
                  <span style={{ color: 'var(--bf-color-content-secondary)', fontSize: 'var(--bf-type-support-font-size)', overflowWrap: 'anywhere' }}>
                    {item.description}
                  </span>
                ) : null}
              </span>
            </li>
          );
        })
      ) : (
        <li style={{ color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-support-font-size)' }}>{emptyMessage}</li>
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
        <span style={{ flex: '0 0 auto', width: 14, color: isFolder ? 'var(--bf-color-accent-default)' : 'var(--bf-color-content-muted)' }}>
          {isFolder ? '▸' : '•'}
        </span>
        <span
          style={{
            minWidth: 0,
            color: toneColor(item.tone),
            fontFamily: 'var(--bf-type-code-sm-font-family)',
            fontSize: 'var(--bf-type-code-sm-font-size)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {item.name ?? item.path}
        </span>
        {item.meta ? (
          <span style={{ flex: '0 0 auto', marginLeft: 'auto', color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-meta-font-size)' }}>
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
  return (
    <div
      {...props}
      className={['bf-file-tree', props.className].filter(Boolean).join(' ')}
      style={{
        minWidth: 0,
        overflow: 'auto',
        border: '1px solid var(--bf-color-border-subtle)',
        borderRadius: 8,
        padding: '8px 10px',
        background: 'color-mix(in srgb, var(--bf-color-surface-panel) 70%, transparent)',
        ...style,
      }}
    >
      {items.length ? renderFileTreeItems(items, 0, defaultExpanded) : (
        <div style={{ color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-support-font-size)' }}>{emptyMessage}</div>
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
    <div {...props} className={['bf-progress', props.className].filter(Boolean).join(' ')} style={style}>
      {label || showValue ? (
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            gap: 10,
            marginBottom: 5,
            color: 'var(--bf-color-content-secondary)',
            fontSize: 'var(--bf-type-support-font-size)',
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
          background: 'var(--bf-color-action-neutral-surface-hover)',
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
      className={['bf-swatch', props.className].filter(Boolean).join(' ')}
      aria-hidden={props['aria-label'] ? undefined : true}
      style={{
        display: 'inline-block',
        width: 12,
        height: 12,
        borderRadius: 3,
        background: categoryColor(color),
        border: '1px solid var(--bf-color-border-subtle)',
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
  const normalized = segments.map((segment, index) => ({
    ...segment,
    value: positiveSegmentValue(segment.value),
    color: segment.color || usageColorSequence[index % usageColorSequence.length],
  }));
  const segmentTotal = normalized.reduce((sum, segment) => sum + segment.value, 0);
  const safeTotal = Math.max(positiveSegmentValue(total), segmentTotal, 1);
  const remainder = Math.max(0, safeTotal - segmentTotal);

  return (
    <div {...props} className={['bf-usage-bar', props.className].filter(Boolean).join(' ')} style={style}>
      {topLeftLabel || topRightLabel ? (
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            gap: 12,
            marginBottom: 6,
            color: 'var(--bf-color-content-secondary)',
            fontSize: 'var(--bf-type-support-font-size)',
            lineHeight: 'var(--bf-line-height-compact)',
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
          background: 'var(--bf-color-action-neutral-surface-hover)',
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
              background: 'var(--bf-color-action-quiet-hover)',
            }}
          />
        ) : null}
      </div>
    </div>
  );
}

function todoStatusColor(status: CanvasTodoItem['status']) {
  if (status === 'completed') return 'var(--bf-color-status-success-content)';
  if (status === 'in_progress') return 'var(--bf-color-status-warning-content)';
  if (status === 'cancelled') return 'var(--bf-color-content-muted)';
  return 'var(--bf-color-content-muted)';
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
        color: 'var(--bf-color-surface-canvas)',
        fontSize: 'var(--bf-type-micro-font-size)',
        lineHeight: 'var(--bf-line-height-none)',
        fontWeight: 'var(--bf-font-weight-bold)',
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
  if (!todos.length) return null;
  const dimmed = dimmedTodoSet(dimmedTodoIds);

  return (
    <div
      {...props}
      className={['bf-todo-list', props.className].filter(Boolean).join(' ')}
      style={{
        display: 'grid',
        gap: 4,
        ...style,
      }}
    >
      {todos.map((todo) => {
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
          color: 'var(--bf-color-content-primary)',
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
                  color: todo.status === 'completed' ? 'var(--bf-color-content-secondary)' : 'var(--bf-color-content-primary)',
                  fontSize: 'var(--bf-type-support-font-size)',
                  lineHeight: 'var(--bf-type-support-line-height)',
                  textDecoration: todo.status === 'completed' ? 'line-through' : undefined,
                  overflowWrap: 'anywhere',
                }}
              >
                {content}
              </span>
              <span style={{ color: todoStatusColor(todo.status), fontSize: 'var(--bf-type-micro-font-size)', lineHeight: 'var(--bf-type-micro-line-height)' }}>
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
  const completed = todos.filter(todo => todo.status === 'completed').length;
  const key = `todo-list-card:${todos.map(todo => todo.id).join('|')}`;
  const [open, setOpen] = useCanvasState(key, Boolean(defaultExpanded));
  if (!todos.length) return null;

  return (
    <section
      {...props}
      className={['bf-todo-list-card', props.className].filter(Boolean).join(' ')}
      style={{
        border: '1px solid var(--bf-color-border-subtle)',
        borderRadius: 8,
        background: 'var(--bf-color-surface-raised)',
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
          borderBottom: open ? '1px solid var(--bf-color-border-subtle)' : 0,
          background: 'transparent',
          color: 'var(--bf-color-content-primary)',
          padding: '8px 10px',
          font: 'inherit',
          cursor: 'pointer',
          textAlign: 'left',
        }}
      >
        <span
          aria-hidden="true"
          style={{
            color: 'var(--bf-color-content-muted)',
            transform: open ? 'rotate(90deg)' : 'rotate(0deg)',
          }}
        >
          ›
        </span>
        <span style={{ fontWeight: 'var(--bf-font-weight-semibold)', fontSize: 'var(--bf-type-support-font-size)' }}>Tasks</span>
        <span style={{ marginLeft: 'auto', color: 'var(--bf-color-content-muted)', fontSize: 'var(--bf-type-support-font-size)' }}>
          {completed}/{todos.length} done
        </span>
      </button>
      {open ? (
        <div style={{ padding: 8 }}>
          <TodoList todos={todos} dimmedTodoIds={dimmedTodoIds} onTodoClick={onTodoClick} />
        </div>
      ) : null}
    </section>
  );
}
