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
        fontFamily: 'var(--bf-type-heading-display-font-family)',
        fontSize: 'var(--bf-type-heading-display-font-size)',
        lineHeight: 'var(--bf-type-heading-display-line-height)',
        margin: 0,
        fontWeight: 'var(--bf-type-heading-display-font-weight)',
        letterSpacing: 'var(--bf-type-heading-display-letter-spacing)',
        color: 'var(--bf-color-content-primary)',
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
        fontFamily: 'var(--bf-type-heading-page-font-family)',
        fontSize: 'var(--bf-type-heading-page-font-size)',
        lineHeight: 'var(--bf-type-heading-page-line-height)',
        margin: 0,
        fontWeight: 'var(--bf-type-heading-page-font-weight)',
        letterSpacing: 'var(--bf-type-heading-page-letter-spacing)',
        color: 'var(--bf-color-content-primary)',
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
        fontFamily: 'var(--bf-type-heading-section-font-family)',
        fontSize: 'var(--bf-type-heading-section-font-size)',
        lineHeight: 'var(--bf-type-heading-section-line-height)',
        margin: 0,
        fontWeight: 'var(--bf-type-heading-section-font-weight)',
        letterSpacing: 'var(--bf-type-heading-section-letter-spacing)',
        color: 'var(--bf-color-content-primary)',
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
  ...props
}: CanvasTextProps) {
  const Component = as;
  const truncateStyle = truncate
    ? { overflow: 'hidden', whiteSpace: 'nowrap', textOverflow: 'ellipsis' } as const
    : {};

  return (
    <Component
      style={{
        margin: 0,
        color: color || toneColor(tone),
        fontSize: sizeValue(size),
        fontWeight: weightValue(weight),
        fontStyle: italic ? 'italic' : undefined,
        ...truncateStyle,
        ...commonStyle(props, style),
      }}
    >
      {children}
    </Component>
  );
}

export function Code({ children, className, ...props }: CanvasCodeProps) {
  return (
    <code {...props} className={['bf-code', className].filter(Boolean).join(' ')}>
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
        color: 'var(--bf-color-accent-default)',
        textDecoration: 'none',
        ...style,
      }}
    >
      {children}
    </a>
  );
}
