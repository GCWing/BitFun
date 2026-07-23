 

import React, { useEffect } from 'react';
import { I18nextProvider } from 'react-i18next';
import { i18nService } from '../core/I18nService';

interface I18nProviderProps {
  children: React.ReactNode;
}

 
export const I18nProvider: React.FC<I18nProviderProps> = ({ children }) => {
  useEffect(() => {
    if (!i18nService.isInitialized()) {
      i18nService.initialize();
    }
  }, []);

  
  return (
    <I18nextProvider i18n={i18nService.getI18nInstance()}>
      {children}
    </I18nextProvider>
  );
};

export default I18nProvider;
