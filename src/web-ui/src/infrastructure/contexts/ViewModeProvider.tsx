import React, { ReactNode, useCallback, useState } from 'react';
import { createLogger } from '@/shared/utils/logger';
import {
  ViewModeContext,
  type ViewMode,
  type ViewModeContextType,
} from './ViewModeContext';

const log = createLogger('ViewModeContext');

interface ViewModeProviderProps {
  children: ReactNode;
  defaultMode?: ViewMode;
}

export const ViewModeProvider: React.FC<ViewModeProviderProps> = ({
  children,
  defaultMode = 'coder',
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

  return <ViewModeContext.Provider value={value}>{children}</ViewModeContext.Provider>;
};
