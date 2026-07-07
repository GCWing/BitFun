import { commonStyle, flexAlign, flexJustify } from './style';
import type {
  CanvasBoxProps,
  CanvasDividerProps,
  CanvasGridProps,
  CanvasRowProps,
  CanvasStackProps,
} from './types';

export function Stack({ children, gap = 12, style, ...props }: CanvasStackProps) {
  return (
    <div
      {...props}
      className={['bf-canvas-stack', props.className].filter(Boolean).join(' ')}
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap,
        ...commonStyle(props, style),
      }}
    >
      {children}
    </div>
  );
}

export function Row({
  children,
  gap = 8,
  align = 'center',
  justify = 'start',
  wrap = false,
  style,
  ...props
}: CanvasRowProps) {
  return (
    <div
      {...props}
      style={{
        display: 'flex',
        flexDirection: 'row',
        gap,
        alignItems: flexAlign(align),
        justifyContent: flexJustify(justify),
        flexWrap: wrap ? 'wrap' : 'nowrap',
        ...commonStyle(props, style),
      }}
    >
      {children}
    </div>
  );
}

export function Grid({
  children,
  columns = 2,
  gap = 12,
  align = 'stretch',
  style,
  ...props
}: CanvasGridProps) {
  return (
    <div
      {...props}
      style={{
        display: 'grid',
        gridTemplateColumns: typeof columns === 'number' ? `repeat(${columns}, minmax(0, 1fr))` : columns,
        gap,
        alignItems: flexAlign(align),
        ...commonStyle(props, style),
      }}
    >
      {children}
    </div>
  );
}

export function Box({ children, style, ...props }: CanvasBoxProps) {
  return (
    <div {...props} style={commonStyle(props, style)}>
      {children}
    </div>
  );
}

export function Spacer() {
  return <div style={{ flex: '1 1 auto', minWidth: 0, minHeight: 0 }} />;
}

export function Divider({ style, ...props }: CanvasDividerProps) {
  return (
    <hr
      {...props}
      style={{
        border: 0,
        borderTop: '1px solid var(--border-subtle)',
        width: '100%',
        margin: '4px 0',
        ...style,
      }}
    />
  );
}
