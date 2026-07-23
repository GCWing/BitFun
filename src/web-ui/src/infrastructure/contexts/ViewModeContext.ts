 
import { createContext, useContext } from 'react';

export type ViewMode = 'cowork' | 'coder';

export interface ViewModeContextType {
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
  isCoworkMode: boolean;
  isCoderMode: boolean;
}

export const ViewModeContext = createContext<ViewModeContextType | undefined>(undefined);

export const useViewMode = (): ViewModeContextType => {
  const context = useContext(ViewModeContext);
  if (!context) {
    throw new Error('useViewMode must be used within ViewModeProvider');
  }
  return context;
};
