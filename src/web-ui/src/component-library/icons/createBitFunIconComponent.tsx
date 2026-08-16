import { forwardRef } from 'react';
import type { LucideIcon } from 'lucide-react';
import type { BitFunIconProps } from './types';

export function createBitFunIconComponent(
  id: string,
  Glyph: LucideIcon,
) {
  const BitFunIconComponent = forwardRef<SVGSVGElement, BitFunIconProps>(({
    decorative = true,
    label,
    size = '1em',
    strokeWidth = 1.9,
    ...props
  }, ref) => (
    <Glyph
      {...props}
      ref={ref}
      size={size}
      strokeWidth={strokeWidth}
      data-bf-component="icon"
      data-bf-icon={id}
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={decorative ? undefined : label}
      role={decorative ? undefined : 'img'}
      focusable="false"
    />
  ));

  BitFunIconComponent.displayName = `BitFunIcon(${id})`;
  return BitFunIconComponent;
}
