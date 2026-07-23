import React, { ReactNode, useMemo } from 'react';
import { CanvasContext, type CanvasContextValue } from './CanvasContext';

export interface CanvasProviderProps {
  children: ReactNode;
  value: CanvasContextValue;
}

export const CanvasProvider: React.FC<CanvasProviderProps> = ({
  children,
  value,
}) => {
  const memoizedValue = useMemo(() => value, [value]);

  return (
    <CanvasContext.Provider value={memoizedValue}>
      {children}
    </CanvasContext.Provider>
  );
};
