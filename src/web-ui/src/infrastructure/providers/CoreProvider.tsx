 
import React, { useEffect, useState, ReactNode } from 'react';
import { initializeCore, destroyCore } from '../index';
import { globalEventBus } from '../event-bus';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { CoreContext, type CoreContextType } from './CoreContext';

const log = createLogger('CoreProvider');

interface CoreProviderProps {
  children: ReactNode;
}

export const CoreProvider: React.FC<CoreProviderProps> = ({ children }) => {
  const { t: tCommon } = useI18n('common');
  const { t: tErrors } = useI18n('errors');
  const [isInitialized, setIsInitialized] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

    const initialize = async () => {
      try {
        setIsLoading(true);
        setError(null);
        
        await initializeCore();
        
        if (mounted) {
          setIsInitialized(true);
          setIsLoading(false);
        }
      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err.message : 'Failed to initialize core systems');
          setIsLoading(false);
        }
      }
    };

    initialize();

    return () => {
      mounted = false;
      
      
      destroyCore().catch(error => {
        log.error('Failed to destroy core systems', error);
      });
    };
  }, []);

  const contextValue: CoreContextType = {
    isInitialized,
    isLoading,
    error,
    eventBus: globalEventBus,
  };

  if (isLoading) {
    return (
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        background: 'var(--background-color, #1a1a1a)',
        color: 'var(--text-color, #ffffff)'
      }}>
        <div>{tCommon('core.initializing')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ 
        display: 'flex', 
        flexDirection: 'column',
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        background: 'var(--background-color, #1a1a1a)',
        color: 'var(--text-color, #ffffff)'
      }}>
        <h2>{tErrors('core.initializationFailed')}</h2>
        <p>{error}</p>
        <button 
          onClick={() => window.location.reload()}
          style={{
            padding: '8px 16px',
            background: 'var(--primary-color, #007acc)',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: 'pointer'
          }}
        >
          {tCommon('actions.reload')}
        </button>
      </div>
    );
  }

  return (
    <CoreContext.Provider value={contextValue}>
      {children}
    </CoreContext.Provider>
  );
};
