import React from 'react';
import './DotMatrixLoader.scss';

export type DotMatrixLoaderSize = 'small' | 'medium' | 'large';

export interface DotMatrixLoaderProps {
  /** small: 4px cells; medium: 6px cells (flow chat processing); large: heavier cells for full-page loaders. */
  size?: DotMatrixLoaderSize;
  className?: string;
  /** Decorative; keeps screen readers from counting 9 spans. @default true */
  ariaHidden?: boolean;
}

const sizeClass: Record<DotMatrixLoaderSize, string> = {
  small: 'dot-matrix-loader--small',
  medium: 'dot-matrix-loader--medium',
  large: 'dot-matrix-loader--large',
};

export const DotMatrixLoader: React.FC<DotMatrixLoaderProps> = ({
  size = 'medium',
  className = '',
  ariaHidden = true,
}) => (
  <span
    className={`dot-matrix-loader ${sizeClass[size]} ${className}`.trim()}
    aria-hidden={ariaHidden}
  >
    {Array.from({ length: 9 }, (_, i) => (
      <span key={i} className="dot-matrix-loader__dot" />
    ))}
  </span>
);

DotMatrixLoader.displayName = 'DotMatrixLoader';

export default DotMatrixLoader;
