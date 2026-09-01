import type { LucideProps } from 'lucide-react';

export interface BitFunIconMetadata {
  id: string;
  exportName: string;
  semantic: string;
  category: string;
  displayName: string;
  description: string;
  source: BitFunIconSource;
  license: string;
  tags: readonly string[];
}

export type BitFunIconSource = {
  type: 'lucide';
  icon: string;
} | {
  type: 'bitfun-svg';
  path: string;
  origin: 'bitfun-reference-redraw' | 'bitfun-authored';
  referenceId?: string;
  revision: number;
};

type BitFunIconBaseProps = Omit<LucideProps, 'aria-hidden' | 'aria-label'>;

type DecorativeBitFunIconProps = {
  decorative?: true;
  label?: never;
};

type AccessibleBitFunIconProps = {
  decorative?: false;
  label: string;
};

/**
 * BitFun icons are decorative by default. Standalone icons must opt into an
 * accessible image with `decorative={false}` and a meaningful `label`.
 */
export type BitFunIconProps = BitFunIconBaseProps & (
  | DecorativeBitFunIconProps
  | AccessibleBitFunIconProps
);
