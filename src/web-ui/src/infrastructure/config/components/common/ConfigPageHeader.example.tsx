 

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@bitfun/ui';
import { ConfigPageHeader } from './ConfigPageHeader';
import { ConfigPageLayout, ConfigPageContent } from './ConfigPageLayout';

 
export const BasicHeaderExample: React.FC = () => {
  const { t } = useTranslation('settings');
  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('configPageHeaderExample.basic.title')}
      />
      <ConfigPageContent>
        
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

 
export const HeaderWithIconExample: React.FC = () => {
  const { t } = useTranslation('settings');
  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('configPageHeaderExample.apiNetwork.title')}
        subtitle={t('configPageHeaderExample.apiNetwork.subtitle')}
      />
      <ConfigPageContent>
        
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

 
export const HeaderWithActionsExample: React.FC = () => {
  const { t } = useTranslation('settings');
  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('configPageHeaderExample.learning.title')}
        subtitle={t('configPageHeaderExample.learning.subtitle')}
        extra={
          <>
            <Button variant="outline" size="sm">
              {t('configPageHeaderExample.learning.import')}
            </Button>
            <Button variant="fill" size="sm">
              {t('configPageHeaderExample.learning.addMemory')}
            </Button>
          </>
        }
      />
      <ConfigPageContent>
        
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

 
export const CompleteConfigPageExample: React.FC = () => {
  const { t } = useTranslation('settings');
  return (
    <ConfigPageLayout className="my-custom-config-page">
      
      <ConfigPageHeader
        title={t('configPageHeaderExample.agents.title')}
        subtitle={t('configPageHeaderExample.agents.subtitle')}
        extra={
          <Button variant="fill" size="sm">
            {t('configPageHeaderExample.agents.create')}
          </Button>
        }
      />
      
      
      <ConfigPageContent>
        
        <div className="config-toolbar">
          
        </div>
        
        
        <div className="config-items">
          
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

 
export const AppearanceConfigExample: React.FC = () => {
  const { t } = useTranslation('settings');
  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('configPageHeaderExample.appearance.title')}
        subtitle={t('configPageHeaderExample.appearance.subtitle')}
        extra={
          <>
            <Button variant="outline" size="sm">
              {t('configPageHeaderExample.appearance.import')}
            </Button>
            <Button variant="outline" size="sm">
              {t('configPageHeaderExample.appearance.export')}
            </Button>
          </>
        }
      />
      <ConfigPageContent>
        
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};



