import React from 'react';
import {
  MobileButton,
  MobileIconButton,
  MobileStatus,
  MobileTextField,
} from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { SessionInfo } from '../services/RemoteSessionManager';

interface SessionLaunchPanelProps {
  creating: boolean;
  hasWorkspace: boolean;
  isProMode: boolean;
  targetInitializing: boolean;
  onCreateClaw: () => void;
  onCreateCowork: () => void;
  onRequestCodeHarness: () => void;
  renderSessionIcon: (agentType: string) => React.ReactNode;
}

export const SessionLaunchPanel: React.FC<SessionLaunchPanelProps> = ({
  creating,
  hasWorkspace,
  isProMode,
  targetInitializing,
  onCreateClaw,
  onCreateCowork,
  onRequestCodeHarness,
  renderSessionIcon,
}) => {
  const { t } = useI18n();
  const disabled = creating || targetInitializing;
  const action = (
    agentType: 'code' | 'cowork' | 'claw',
    title: string,
    description: string,
    onClick: () => void,
  ) => (
    <MobileButton
      appearance="secondary"
      block
      className={`session-list__create-btn session-list__create-btn--${agentType}`}
      onClick={onClick}
      disabled={disabled}
    >
      <div className="session-list__create-icon">{renderSessionIcon(agentType)}</div>
      <div className="session-list__create-copy">
        <span className="session-list__create-title">{title}</span>
        <span className="session-list__create-desc">{description}</span>
      </div>
      <span className="session-list__create-arrow" aria-hidden="true">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
      </span>
    </MobileButton>
  );

  return (
    <section className={`session-list__panel ${!isProMode ? 'session-list__panel--assistant' : ''}`}>
      <div className="session-list__section-head">
        <div>
          <div className="session-list__section-kicker">{t('sessions.launch')}</div>
          <div className="session-list__section-title">{t('sessions.startRemoteFlow')}</div>
        </div>
      </div>
      {isProMode ? (
        hasWorkspace ? (
          <div className="session-list__create-row">
            {action('code', t('shared.agents.code'), t('sessions.codeSessionDesc'), onRequestCodeHarness)}
            {action('cowork', t('shared.agents.cowork'), t('sessions.coworkSessionDesc'), onCreateCowork)}
          </div>
        ) : null
      ) : (
        <div className="session-list__create-row">
          {action('claw', t('sessions.clawSession'), t('sessions.clawSessionDesc'), onCreateClaw)}
        </div>
      )}
    </section>
  );
};

interface SessionHistoryPanelProps {
  activeSessionId?: string | null;
  hasSearchQuery: boolean;
  isProMode: boolean;
  loading: boolean;
  loadingMore: boolean;
  menuSessionId?: string;
  searchQuery: string;
  sessions: SessionInfo[];
  totalSessionCount: number;
  targetInitializing: boolean;
  onOpenMenu: (session: SessionInfo) => void;
  onSearchQueryChange: (query: string) => void;
  onSessionClick: (session: SessionInfo, event: React.MouseEvent) => void;
  onSessionTouchEnd: () => void;
  onSessionTouchMove: (event: React.TouchEvent) => void;
  onSessionTouchStart: (session: SessionInfo, event: React.TouchEvent) => void;
  renderAgentLabel: (agentType: string) => string;
  renderSessionIcon: (agentType: string) => React.ReactNode;
  renderSessionTime: (updatedAt: string) => string;
}

export const SessionHistoryPanel: React.FC<SessionHistoryPanelProps> = ({
  activeSessionId,
  hasSearchQuery,
  isProMode,
  loading,
  loadingMore,
  menuSessionId,
  searchQuery,
  sessions,
  totalSessionCount,
  targetInitializing,
  onOpenMenu,
  onSearchQueryChange,
  onSessionClick,
  onSessionTouchEnd,
  onSessionTouchMove,
  onSessionTouchStart,
  renderAgentLabel,
  renderSessionIcon,
  renderSessionTime,
}) => {
  const { t } = useI18n();
  return (
    <section className={`session-list__panel session-list__panel--sessions ${!isProMode ? 'session-list__panel--assistant' : ''}`}>
      <div className="session-list__section-head">
        <div>
          <div className="session-list__section-kicker">{t('sessions.recent')}</div>
          <div className="session-list__section-title">{t('sessions.sessionHistory')}</div>
        </div>
        <div className="session-list__section-meta">{t('common.itemCount', { count: totalSessionCount })}</div>
      </div>

      <div className="session-list__search">
        <svg className="session-list__search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <MobileTextField
          appearance="surface"
          className="session-list__search-field"
          inputClassName="session-list__search-input"
          type="search"
          placeholder={t('sessions.searchSessions')}
          value={searchQuery}
          onChange={(event) => onSearchQueryChange(event.target.value)}
          disabled={targetInitializing}
          enterKeyHint="search"
        />
        {searchQuery && (
          <MobileIconButton
            appearance="plain"
            size="sm"
            className="session-list__search-clear"
            onClick={() => onSearchQueryChange('')}
            aria-label="Clear"
            disabled={targetInitializing}
            icon={<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>}
          />
        )}
      </div>

      {(loading || targetInitializing) && totalSessionCount === 0 && (
        <MobileStatus className="session-list__empty" loading title={t('sessions.loadingSessions')} />
      )}
      {!loading && !targetInitializing && totalSessionCount === 0 && (
        <MobileStatus
          className="session-list__empty"
          description={hasSearchQuery ? t('sessions.emptySearch') : t('sessions.noSessions')}
        />
      )}

      <div className="session-list__cards">
        {sessions.map((session) => (
          <MobileButton
            appearance="plain"
            block
            key={session.session_id}
            className={`session-list__item${menuSessionId === session.session_id ? ' session-list__item--active' : ''}${activeSessionId === session.session_id ? ' is-selected' : ''}`}
            onClick={(event) => onSessionClick(session, event)}
            onTouchStart={(event) => onSessionTouchStart(session, event)}
            onTouchMove={onSessionTouchMove}
            onTouchEnd={onSessionTouchEnd}
            onTouchCancel={onSessionTouchEnd}
            onContextMenu={(event) => { event.preventDefault(); onOpenMenu(session); }}
          >
            <div className={`session-list__item-icon session-list__item-icon--${session.agent_type}`}>
              {renderSessionIcon(session.agent_type)}
            </div>
            <div className="session-list__item-body">
              <div className="session-list__item-top">
                <div className="session-list__item-name">{session.name || t('sessions.untitledSession')}</div>
                <span className={`session-list__agent-badge session-list__agent-badge--${session.agent_type}`}>
                  {renderAgentLabel(session.agent_type)}
                </span>
              </div>
              <div className="session-list__item-time">{renderSessionTime(session.updated_at)}</div>
            </div>
          </MobileButton>
        ))}
      </div>

      {loadingMore && <MobileStatus className="session-list__load-more" loading title={t('sessions.loadingMore')} />}
    </section>
  );
};
