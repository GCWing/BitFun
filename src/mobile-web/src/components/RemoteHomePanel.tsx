import React from 'react';
import { MobileButton, MobileIconButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

interface RemoteHomePanelProps {
  onOpenSidebar?: () => void;
  onOpenWorkspace?: () => void;
  onStartConversation?: () => void;
  conversationStarting?: boolean;
}

const RemoteHomePanel: React.FC<RemoteHomePanelProps> = ({
  onOpenSidebar,
  onOpenWorkspace,
  onStartConversation,
  conversationStarting = false,
}) => {
  const { t } = useI18n();
  const openNavigation = onOpenSidebar;

  return (
    <main className="remote-home" aria-labelledby="remote-home-title">
      <header className="remote-home__header">
        {openNavigation ? (
          <MobileIconButton
            appearance="floating"
            className="remote-home__menu"
            icon={(
              <svg viewBox="0 0 24 24" fill="none">
                <path d="M5 8H19M5 16H15" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            )}
            onClick={openNavigation}
            aria-label={t('sessions.sessionHistory')}
          />
        ) : (
          <span className="remote-home__menu remote-home__menu--placeholder" aria-hidden="true" />
        )}
        <div className="remote-home__heading">
          <h2 id="remote-home-title">OpenBitFun</h2>
        </div>
        <span className="remote-home__header-spacer" aria-hidden="true" />
      </header>
      <div className="remote-home__content">
        <p className="remote-home__message">{t('shell.selectConversation')}</p>
      </div>
      <div className="remote-home__composer-wrap">
        <MobileButton
          appearance="secondary"
          block
          className="remote-home__composer"
          onClick={onStartConversation ?? onOpenWorkspace ?? openNavigation}
          disabled={conversationStarting}
          loading={conversationStarting}
          leading={<svg className="remote-home__composer-plus" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true"><path d="M12 4v16M4 12h16"/></svg>}
        >
          <span>{t('shell.askOpenBitFun')}</span>
        </MobileButton>
      </div>
    </main>
  );
};

export default RemoteHomePanel;
