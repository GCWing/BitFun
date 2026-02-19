 

import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ViewModeContext');

export type ViewMode = 'cowork' | 'coder';

interface ViewModeContextType {
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  isCoworkMode: boolean;
  isCoderMode: boolean;
}

const ViewModeContext = createContext<ViewModeContextType | undefined>(undefined);

interface ViewModeProviderProps {
  children: ReactNode;
  defaultMode?: ViewMode;
}

export const ViewModeProvider: React.FC<ViewModeProviderProps> = ({ 
  children, 
  defaultMode = 'coder' 
}) => {
  const [viewMode, setViewModeState] = useState<ViewMode>(defaultMode);

  const setViewMode = useCallback((mode: ViewMode) => {
    log.debug('View mode changed', { to: mode });
    setViewModeState(mode);
  }, []);

  const value: ViewModeContextType = {
    viewMode,
    setViewMode,
    isCoworkMode: viewMode === 'cowork',
    isCoderMode: viewMode === 'coder',
  };

  return (
    <ViewModeContext.Provider value={value}>
      {children}
    </ViewModeContext.Provider>
  );
};

export const useViewMode = (): ViewModeContextType => {
  const context = useContext(ViewModeContext);
  if (!context) {
    throw new Error('useViewMode must be used within ViewModeProvider');
  }
  return context;
};

