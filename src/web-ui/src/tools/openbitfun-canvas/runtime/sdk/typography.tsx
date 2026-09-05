import { commonStyle, sizeValue, toneColor, weightValue } from './style';
import type {
  CanvasCodeProps,
  CanvasHeadingProps,
  CanvasLinkProps,
  CanvasTextProps,
} from './types';

export function H1({ children, style, ...props }: CanvasHeadingProps) {
  return (
    <h1
      {...props}
      style={{
        fontFamily: 'var(--openbitfun-type-heading-display-font-family)',
        fontSize: 'var(--openbitfun-type-heading-display-font-size)',
        lineHeight: 'var(--openbitfun-type-heading-display-line-height)',
        margin: 0,
        fontWeight: 'var(--openbitfun-type-heading-display-font-weight)',
        letterSpacing: 'var(--openbitfun-type-heading-display-letter-spacing)',
        color: 'var(--openbitfun-color-content-primary)',
        ...style,
      }}
    >
      {children}
    </h1>
  );
}

export function H2({ children, style, ...props }: CanvasHeadingProps) {
  return (
    <h2
      {...props}
      style={{
        fontFamily: 'var(--openbitfun-type-heading-page-font-family)',
        fontSize: 'var(--openbitfun-type-heading-page-font-size)',
        lineHeight: 'var(--openbitfun-type-heading-page-line-height)',
        margin: 0,
        fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
        letterSpacing: 'var(--openbitfun-type-heading-page-letter-spacing)',
        color: 'var(--openbitfun-color-content-primary)',
        ...style,
      }}
    >
      {children}
    </h2>
  );
}

export function H3({ children, style, ...props }: CanvasHeadingProps) {
  return (
    <h3
      {...props}
      style={{
        fontFamily: 'var(--openbitfun-type-heading-section-font-family)',
        fontSize: 'var(--openbitfun-type-heading-section-font-size)',
        lineHeight: 'var(--openbitfun-type-heading-section-line-height)',
        margin: 0,
        fontWeight: 'var(--openbitfun-type-heading-section-font-weight)',
        letterSpacing: 'var(--openbitfun-type-heading-section-letter-spacing)',
        color: 'var(--openbitfun-color-content-primary)',
        ...style,
      }}
    >
      {children}
    </h3>
  );
}

export function Text({
  children,
  tone = 'primary',
  size = 'body',
  weight = 'normal',
  italic = false,
  as = 'p',
  truncate = false,
  style,
  color,
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
  opacity,
  minWidth,
  maxWidth,
  minHeight,
  maxHeight,
  ...elementProps
}: CanvasTextProps) {
  const Component = as;
  const truncateStyle = truncate
    ? { overflow: 'hidden', whiteSpace: 'nowrap', textOverflow: 'ellipsis' } as const
    : {};

  return (
    <Component
      {...elementProps}
      style={{
        margin: 0,
        color: color || toneColor(tone),
        fontSize: sizeValue(size),
        fontWeight: weightValue(weight),
        fontStyle: italic ? 'italic' : undefined,
        ...truncateStyle,
        ...commonStyle({
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
        }, style),
      }}
    >
      {children}
    </Component>
  );
}

export function Code({ children, className, ...props }: CanvasCodeProps) {
  return (
    <code {...props} className={['openbitfun-code', className].filter(Boolean).join(' ')}>
      {children}
    </code>
  );
}

export function Link({ children, style, ...props }: CanvasLinkProps) {
  return (
    <a
      {...props}
      target={props.target ?? '_blank'}
      rel={props.rel ?? 'noreferrer'}
      style={{
        color: 'var(--openbitfun-color-accent-default)',
        textDecoration: 'none',
        ...style,
      }}
    >
      {children}
    </a>
  );
}
