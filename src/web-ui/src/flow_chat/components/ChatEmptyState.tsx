import React from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import './ChatEmptyState.scss';

/**
 * Chat empty state component
 * Displays current workspace and prompts user to interact via AI chat
 */
export const ChatEmptyState: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const { workspace: currentWorkspace } = useCurrentWorkspace();

  return (
    <div className="fc-chat-empty">
      <div className="fc-chat-empty__container">
        {currentWorkspace && (
          <>
            <div className="fc-chat-empty__greeting">
              <p>{t('emptyState.welcomeBack')}</p>
              <p>
                <Trans
                  i18nKey="emptyState.workingIn"
                  t={t}
                  values={{ workspace: currentWorkspace.name }}
                  components={{
                    workspace: <span className="fc-chat-empty__workspace-name" />
                  }}
                />
              </p>
            </div>

            <div className="fc-chat-empty__divider" />

            <div className="fc-chat-empty__prompt">
              <p>{t('emptyState.capabilities')}</p>
              <p>{t('emptyState.capabilities2')}</p>
              <p className="fc-chat-empty__prompt-hint">{t('emptyState.readyToHelp')}</p>
            </div>
          </>
        )}
      </div>
    </div>
  );
};
