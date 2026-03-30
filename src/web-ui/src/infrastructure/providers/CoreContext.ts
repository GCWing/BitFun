import { createContext } from 'react';
import { globalEventBus } from '../event-bus';

export interface CoreContextType {
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;
  eventBus: typeof globalEventBus;
}

export const CoreContext = createContext<CoreContextType | null>(null);
