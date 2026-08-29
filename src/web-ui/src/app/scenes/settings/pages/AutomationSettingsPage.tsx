import { ScrollArea } from '@bitfun/ui';
import React, { lazy, Suspense, useEffect, useRef } from 'react';
import type { SettingsPageProps } from '../settingsTypes';
import './AutomationSettingsPage.scss';

const QuickActionsConfig = lazy(() => import('@/infrastructure/config/components/QuickActionsConfig'));
const HooksConfig = lazy(() => import('@/infrastructure/config/components/HooksConfig'));

function AutomationSettingsLoading() {
  return (
    <div className="bitfun-automation-settings-page__loading" aria-busy="true" aria-hidden="true">
      <span />
      <span />
      <span className="bitfun-automation-settings-page__loading-block" />
    </div>
  );
}

const AutomationSettingsPage: React.FC<SettingsPageProps> = ({ viewId, navigationRequestId }) => {
  const quickActionsSectionRef = useRef<HTMLElement>(null);
  const hooksSectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const target = viewId === 'hooks' ? hooksSectionRef.current : quickActionsSectionRef.current;
    target?.scrollIntoView?.({ block: 'start' });
  }, [navigationRequestId, viewId]);

  return (
    <ScrollArea
      className="bitfun-automation-settings-page"
      data-bf-component="automation-settings-page"
      data-bf-part="root"
    >
      <section
        ref={quickActionsSectionRef}
        className="bitfun-automation-settings-page__section"
        data-bf-component="automation-settings-page"
        data-bf-part="quickActions"
      >
        <Suspense fallback={<AutomationSettingsLoading />}>
          <QuickActionsConfig />
        </Suspense>
      </section>
      <section
        ref={hooksSectionRef}
        className="bitfun-automation-settings-page__section bitfun-automation-settings-page__section--hooks"
        data-bf-component="automation-settings-page"
        data-bf-part="hooks"
      >
        <Suspense fallback={<AutomationSettingsLoading />}>
          <HooksConfig />
        </Suspense>
      </section>
    </ScrollArea>
  );
};

export default AutomationSettingsPage;
