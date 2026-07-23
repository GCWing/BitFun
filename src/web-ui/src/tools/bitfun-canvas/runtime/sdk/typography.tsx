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
        fontSize: 28,
        lineHeight: 1.12,
        margin: 0,
        fontWeight: 720,
        letterSpacing: 0,
        color: 'var(--color-text-primary)',
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
        fontSize: 18,
        lineHeight: 1.25,
        margin: 0,
        fontWeight: 680,
        letterSpacing: 0,
        color: 'var(--color-text-primary)',
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
        fontSize: 14,
        lineHeight: 1.3,
        margin: 0,
        fontWeight: 680,
        letterSpacing: 0,
        color: 'var(--color-text-primary)',
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
        color: 'var(--color-accent-500)',
        textDecoration: 'none',
        ...style,
      }}
    >
      {children}
    </a>
  );
}
