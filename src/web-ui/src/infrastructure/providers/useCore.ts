import { useContext } from 'react';
import { CoreContext, type CoreContextType } from './CoreContext';

export const useCore = (): CoreContextType => {
  const context = useContext(CoreContext);
  if (!context) {
    throw new Error('useCore must be used within a CoreProvider');
  }
  return context;
};

export const useCoreInitialized = (): boolean => {
  const { isInitialized } = useCore();
  return isInitialized;
};

export const useCoreLoading = (): boolean => {
  const { isLoading } = useCore();
  return isLoading;
};

export const useCoreError = (): string | null => {
  const { error } = useCore();
  return error;
};
