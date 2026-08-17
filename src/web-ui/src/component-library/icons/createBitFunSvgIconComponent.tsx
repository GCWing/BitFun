import { forwardRef } from 'react';
import type { BitFunIconProps } from './types';

interface BitFunSvgPathBase {
  d: string;
  fillRule?: 'evenodd' | 'nonzero';
  clipRule?: 'evenodd' | 'nonzero';
}

type BitFunSvgFillPath = BitFunSvgPathBase & {
  fill: 'currentColor';
  stroke?: never;
  strokeWidth?: never;
  strokeLinecap?: never;
  strokeLinejoin?: never;
};

type BitFunSvgStrokePath = BitFunSvgPathBase & {
  fill: 'none';
  stroke: 'currentColor';
  strokeWidth: number;
  strokeLinecap?: 'butt' | 'round' | 'square';
  strokeLinejoin?: 'miter' | 'round' | 'bevel';
};

export type BitFunSvgPath = BitFunSvgFillPath | BitFunSvgStrokePath;

export function createBitFunSvgIconComponent(
  id: string,
  paths: readonly BitFunSvgPath[],
) {
  const BitFunSvgIconComponent = forwardRef<SVGSVGElement, BitFunIconProps>(({
    decorative = true,
    label,
    size = '1em',
    absoluteStrokeWidth: _absoluteStrokeWidth,
    strokeWidth: _strokeWidth,
    ...props
  }, ref) => (
    <svg
      {...props}
      ref={ref}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      data-bf-icon={id}
      data-bf-source="bitfun-svg"
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={decorative ? undefined : label}
      role={decorative ? undefined : 'img'}
      focusable="false"
    >
      {paths.map(path => (
        <path
          key={path.d}
          d={path.d}
          fill={path.fill}
          stroke={path.stroke}
          strokeWidth={path.strokeWidth}
          strokeLinecap={path.strokeLinecap}
          strokeLinejoin={path.strokeLinejoin}
          fillRule={path.fillRule}
          clipRule={path.clipRule}
        />
      ))}
    </svg>
  ));

  BitFunSvgIconComponent.displayName = `BitFunSvgIcon(${id})`;
  return BitFunSvgIconComponent;
}
