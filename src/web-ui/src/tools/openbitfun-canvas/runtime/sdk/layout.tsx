import { Stack as DesignStack } from '@openbitfun/ui';
import { commonStyle, flexAlign, flexJustify } from './style';
import type {
  CanvasBoxProps,
  CanvasCommonStyleProps,
  CanvasDividerProps,
  CanvasGridProps,
  CanvasRowProps,
  CanvasStackProps,
} from './types';

function splitCommonProps<T extends CanvasCommonStyleProps>(props: T) {
  const {
    padding,
    margin,
    background,
    border,
    borderTop,
    borderRight,
    borderBottom,
    borderLeft,
    borderRadius,
    width,
    height,
    flex,
    display,
    color,
    opacity,
    minWidth,
    maxWidth,
    minHeight,
    maxHeight,
    ...elementProps
  } = props;
  return {
    elementProps,
    styleProps: {
      padding,
      margin,
      background,
      border,
      borderTop,
      borderRight,
      borderBottom,
      borderLeft,
      borderRadius,
      width,
      height,
      flex,
      display,
      color,
      opacity,
      minWidth,
      maxWidth,
      minHeight,
      maxHeight,
    },
  };
}

export function Stack({ children, gap = 12, style, ...props }: CanvasStackProps) {
  const { elementProps, styleProps } = splitCommonProps(props);
  return (
    <DesignStack
      {...elementProps}
      className={['openbitfun-canvas-stack', elementProps.className].filter(Boolean).join(' ')}
      direction="vertical"
      gap="0"
      style={{
        gap,
        ...commonStyle(styleProps, style),
      }}
    >
      {children}
    </DesignStack>
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
  const { elementProps, styleProps } = splitCommonProps(props);
  return (
    <DesignStack
      {...elementProps}
      direction="horizontal"
      gap="0"
      wrap={wrap}
      style={{
        gap,
        alignItems: flexAlign(align),
        justifyContent: flexJustify(justify),
        ...commonStyle(styleProps, style),
      }}
    >
      {children}
    </DesignStack>
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
  const { elementProps, styleProps } = splitCommonProps(props);
  return (
    <div
      {...elementProps}
      style={{
        display: 'grid',
        gridTemplateColumns: typeof columns === 'number' ? `repeat(${columns}, minmax(0, 1fr))` : columns,
        gap,
        alignItems: flexAlign(align),
        ...commonStyle(styleProps, style),
      }}
    >
      {children}
    </div>
  );
}

export function Box({ children, style, ...props }: CanvasBoxProps) {
  const { elementProps, styleProps } = splitCommonProps(props);
  return (
    <div {...elementProps} style={commonStyle(styleProps, style)}>
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
        borderTop: '1px solid var(--openbitfun-color-border-subtle)',
        width: '100%',
        margin: '4px 0',
        ...style,
      }}
    />
  );
}
