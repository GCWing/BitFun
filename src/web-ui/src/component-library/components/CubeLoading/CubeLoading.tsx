import React from 'react';
import { DotMatrixLoader, type DotMatrixLoaderSize } from '../DotMatrixLoader';

export type CubeLoadingSize = 'small' | 'medium' | 'large';

export interface CubeLoadingProps {
  /** Size: small | medium | large (maps to DotMatrixLoader grid; unified with flow chat processing). */
  size?: CubeLoadingSize;
  /** Loading text */
  text?: string;
  /** Custom class name */
  className?: string;
}

const cubeToMatrix: Record<CubeLoadingSize, DotMatrixLoaderSize> = {
  small: 'small',
  medium: 'medium',
  large: 'large',
};

export const CubeLoading: React.FC<CubeLoadingProps> = ({
  size = 'medium',
  text,
  className = '',
}) => {
  return (
    <div
      className={`cube-loading cube-loading--${size} ${className}`}
      data-bf-component="cube-loading"
      data-bf-part="root"
      data-bf-size={size}
      style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px' }}
    >
      <DotMatrixLoader size={cubeToMatrix[size]} />
      {text && <div className="cube-loading__text" data-bf-component="cube-loading" data-bf-part="text">{text}</div>}
    </div>
  );
};

CubeLoading.displayName = 'CubeLoading';

export default CubeLoading;
