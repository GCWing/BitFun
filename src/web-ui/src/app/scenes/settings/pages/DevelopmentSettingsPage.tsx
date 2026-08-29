import { ScrollArea } from '@bitfun/ui';
import React, { lazy, Suspense, useEffect, useRef } from 'react';
import type { SettingsPageProps } from '../settingsTypes';
import './DevelopmentSettingsPage.scss';

const EditorConfig = lazy(() => import('@/infrastructure/config/components/EditorConfig'));
const TerminalSettingsPage = lazy(() => import('@/infrastructure/config/components/ApplicationSettingsPages').then((module) => ({
  default: module.TerminalSettingsPage,
})));

function DevelopmentSettingsLoading() {
  return (
    <div className="bitfun-development-settings-page__loading" aria-busy="true" aria-hidden="true">
      <span />
      <span />
      <span className="bitfun-development-settings-page__loading-block" />
    </div>
  );
}

const DevelopmentSettingsPage: React.FC<SettingsPageProps> = ({ viewId, navigationRequestId }) => {
  const terminalSectionRef = useRef<HTMLElement>(null);
  const editorSectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const target = viewId === 'editor' ? editorSectionRef.current : terminalSectionRef.current;
    target?.scrollIntoView?.({ block: 'start' });
  }, [navigationRequestId, viewId]);

  return (
    <ScrollArea
      className="bitfun-development-settings-page"
      data-bf-component="development-settings-page"
      data-bf-part="root"
    >
      <section
        ref={terminalSectionRef}
        className="bitfun-development-settings-page__section"
        data-bf-component="development-settings-page"
        data-bf-part="terminal"
      >
        <Suspense fallback={<DevelopmentSettingsLoading />}>
          <TerminalSettingsPage />
        </Suspense>
      </section>
      <section
        ref={editorSectionRef}
        className="bitfun-development-settings-page__section bitfun-development-settings-page__section--editor"
        data-bf-component="development-settings-page"
        data-bf-part="editor"
      >
        <Suspense fallback={<DevelopmentSettingsLoading />}>
          <EditorConfig />
        </Suspense>
      </section>
    </ScrollArea>
  );
};

export default DevelopmentSettingsPage;
