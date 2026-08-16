import { forwardRef } from 'react';
import {
  bitFunIconComponents,
  type BitFunIconName,
} from './generated/iconRegistry.generated';
import type { BitFunIconProps } from './types';

export type DynamicBitFunIconProps = BitFunIconProps & {
  name: BitFunIconName;
};

/**
 * Dynamic icon boundary for catalogs and persisted icon ids. Product code with
 * a known semantic should import the generated static component instead.
 */
export const BitFunIcon = forwardRef<SVGSVGElement, DynamicBitFunIconProps>(({
  name,
  ...props
}, ref) => {
  const Icon = bitFunIconComponents[name];
  return <Icon {...props} ref={ref} />;
});

BitFunIcon.displayName = 'BitFunIcon';
