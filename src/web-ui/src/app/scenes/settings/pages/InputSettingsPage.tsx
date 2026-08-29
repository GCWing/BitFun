import { ScrollArea } from '@bitfun/ui';
import React, { lazy, Suspense, useEffect, useRef } from 'react';
import type { SettingsPageProps } from '../settingsTypes';
import './InputSettingsPage.scss';

const VoiceInputConfig = lazy(() => import('@/infrastructure/config/components/VoiceInputConfig'));
const KeyboardShortcutsTab = lazy(() => import('../components/KeyboardShortcutsTab'));

function InputSettingsLoading() {
  return (
    <div className="bitfun-input-settings-page__loading" aria-busy="true" aria-hidden="true">
      <span />
      <span />
      <span className="bitfun-input-settings-page__loading-block" />
    </div>
  );
}

const InputSettingsPage: React.FC<SettingsPageProps> = ({ viewId, navigationRequestId }) => {
  const voiceSectionRef = useRef<HTMLElement>(null);
  const shortcutsSectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const target = viewId === 'shortcuts' ? shortcutsSectionRef.current : voiceSectionRef.current;
    target?.scrollIntoView?.({ block: 'start' });
  }, [navigationRequestId, viewId]);

  return (
    <ScrollArea
      className="bitfun-input-settings-page"
      data-bf-component="input-settings-page"
      data-bf-part="root"
    >
      <section
        ref={voiceSectionRef}
        className="bitfun-input-settings-page__section"
        data-bf-component="input-settings-page"
        data-bf-part="voice"
      >
        <Suspense fallback={<InputSettingsLoading />}>
          <VoiceInputConfig />
        </Suspense>
      </section>
      <section
        ref={shortcutsSectionRef}
        className="bitfun-input-settings-page__section bitfun-input-settings-page__section--shortcuts"
        data-bf-component="input-settings-page"
        data-bf-part="shortcuts"
      >
        <Suspense fallback={<InputSettingsLoading />}>
          <KeyboardShortcutsTab />
        </Suspense>
      </section>
    </ScrollArea>
  );
};

export default InputSettingsPage;
