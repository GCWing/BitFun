/**
 * Welcome panel shown in the empty chat state.
 * Layout mirrors WelcomeScene: centered container, left-aligned content.
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, ChevronDown, Check } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import type { WorkspaceInfo } from '@/shared/types';
import CoworkExampleCards from './CoworkExampleCards';
import { useAgentIdentityDocument } from '@/app/scenes/my-agent/useAgentIdentityDocument';
import './WelcomePanel.css';

const log = createLogger('WelcomePanel');

interface WelcomePanelProps {
  onQuickAction?: (command: string) => void;
  className?: string;
  sessionMode?: string;
  workspacePath?: string;
}

export const WelcomePanel: React.FC<WelcomePanelProps> = ({
  onQuickAction,
  className = '',
  sessionMode,
  workspacePath = '',
}) => {
  const { t } = useTranslation('flow-chat');
  const [workspaceDropdownOpen, setWorkspaceDropdownOpen] = useState(false);
  const [isSelectingWorkspace, setIsSelectingWorkspace] = useState(false);
  const workspaceDropdownRef = useRef<HTMLDivElement>(null);

  const {
    hasWorkspace,
    currentWorkspace,
    openedWorkspacesList,
    openWorkspace,
    switchWorkspace,
  } = useWorkspaceContext();
  const sessionModeLower = (sessionMode || '').toLowerCase();
  const isCoworkSession = sessionModeLower === 'cowork';
  const isClawSession = sessionModeLower === 'claw';
  const isDispatcherSession = sessionModeLower === 'dispatcher';
  // code sessions use mode='agentic'; cowork sessions use mode='cowork'
  const showPanda = sessionModeLower !== 'code' && sessionModeLower !== 'agentic' && sessionModeLower !== 'cowork' && sessionModeLower !== 'dispatcher';

  const { document: identityDoc } = useAgentIdentityDocument(isClawSession ? workspacePath : '');
  const assistantName = isClawSession ? (identityDoc.name || '') : '';

  const greeting = useMemo(() => {
    const hour = new Date().getHours();
    const s = isCoworkSession ? 'Cowork' : isClawSession ? 'Claw' : isDispatcherSession ? 'Dispatcher' : '';
    if (hour >= 5 && hour < 12) return { title: t('welcome.greetingMorning'), subtitle: t(`welcome.subtitleMorning${s}`) };
    if (hour >= 12 && hour < 18) return { title: t('welcome.greetingAfternoon'), subtitle: t(`welcome.subtitleAfternoon${s}`) };
    if (hour >= 18 && hour < 23) return { title: t('welcome.greetingEvening'), subtitle: t(`welcome.subtitleEvening${s}`) };
    return { title: t('welcome.greetingNight'), subtitle: t(`welcome.subtitleNight${s}`) };
  }, [t, isCoworkSession, isClawSession, isDispatcherSession]);

  const tagline = greeting.subtitle;
  const aiPartnerKey = isCoworkSession ? 'welcome.aiPartnerCowork' : isClawSession ? 'welcome.aiPartnerClaw' : isDispatcherSession ? 'welcome.aiPartnerDispatcher' : 'welcome.aiPartner';

  const otherWorkspaces = useMemo(
    () => openedWorkspacesList.filter(ws => ws.id !== currentWorkspace?.id),
    [openedWorkspacesList, currentWorkspace?.id],
  );

  useEffect(() => {
    if (!workspaceDropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (workspaceDropdownRef.current && !workspaceDropdownRef.current.contains(e.target as Node)) {
        setWorkspaceDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [workspaceDropdownOpen]);

  const handleSwitchWorkspace = useCallback(async (ws: WorkspaceInfo) => {
    try { setWorkspaceDropdownOpen(false); await switchWorkspace(ws); }
    catch (err) { log.warn('Failed to switch workspace', err); }
  }, [switchWorkspace]);

  const handleOpenOtherFolder = useCallback(async () => {
    try {
      setWorkspaceDropdownOpen(false);
      setIsSelectingWorkspace(true);
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') await openWorkspace(selected);
    } catch (err) {
      log.warn('Failed to open workspace folder', err);
    } finally {
      setIsSelectingWorkspace(false);
    }
  }, [openWorkspace]);

  const handleQuickActionClick = useCallback((cmd: string) => {
    onQuickAction?.(cmd);
  }, [onQuickAction]);

  return (
    <div className={`welcome-panel ${className}`}>
      <div className="welcome-panel__content">
        {/* Greeting */}
        <div className="welcome-panel__greeting">
          <div className="welcome-panel__greeting-inner">
            {showPanda && (
              <div className="welcome-panel__panda" aria-hidden="true">
                <img src="/panda_full_1.png" className="welcome-panel__panda-frame welcome-panel__panda-frame--1" alt="" />
                <img src="/panda_full_2.png" className="welcome-panel__panda-frame welcome-panel__panda-frame--2" alt="" />
              </div>
            )}
            <div className="welcome-panel__greeting-text">
              <h1 className="welcome-panel__heading">
                {isDispatcherSession
                  ? greeting.title
                  : <>{greeting.title}，{t(aiPartnerKey)}{isClawSession && assistantName ? `，${assistantName}` : ''}</>
                }
              </h1>
              <p className="welcome-panel__tagline">{tagline}</p>
            </div>
          </div>
        </div>

        <div className="welcome-panel__divider" />

        {/* Narrative: workspace */}
        <div className="welcome-panel__narrative">
          <p className="welcome-panel__narrative-text">
            {isDispatcherSession ? (
              t('welcome.narrativeDispatcher')
            ) : isClawSession ? (
              t('welcome.narrativeClaw')
            ) : !hasWorkspace ? (
              <>
                {t('welcome.noWorkspaceHint')}
                <button
                  type="button"
                  className="welcome-panel__inline-btn welcome-panel__inline-btn--interactive"
                  onClick={() => { void handleOpenOtherFolder(); }}
                  disabled={isSelectingWorkspace}
                >
                  {t('welcome.openOne')}
                </button>
                {' '}{t('welcome.toStart')}
              </>
            ) : (
              <>
                <span className="welcome-panel__narrative-sentence">
                  <span className="welcome-panel__narrative-sentence__text">
                    {isCoworkSession ? t('welcome.workingInCowork') : t('welcome.workingIn')}
                  </span>
                  <span className="welcome-panel__context-row">
                    <span className="welcome-panel__workspace-anchor" ref={workspaceDropdownRef}>
                      <button
                        type="button"
                        className={`welcome-panel__inline-btn welcome-panel__inline-btn--interactive${workspaceDropdownOpen ? ' welcome-panel__inline-btn--active' : ''}`}
                        onClick={() => setWorkspaceDropdownOpen(v => !v)}
                        disabled={isSelectingWorkspace}
                        title={currentWorkspace?.rootPath}
                      >
                        <FolderOpen size={13} className="welcome-panel__inline-icon" />
                        {currentWorkspace?.name || t('welcome.workspace')}
                        <ChevronDown
                          size={11}
                          className={`welcome-panel__inline-chevron${workspaceDropdownOpen ? ' welcome-panel__inline-chevron--open' : ''}`}
                        />
                      </button>
                      {workspaceDropdownOpen && (
                        <div className="welcome-panel__dropdown">
                          {hasWorkspace && currentWorkspace && (
                            <div className="welcome-panel__dropdown-current">
                              <Check size={11} />
                              <FolderOpen size={12} />
                              <span className="welcome-panel__dropdown-name">{currentWorkspace.name}</span>
                            </div>
                          )}
                          {otherWorkspaces.length > 0 && (
                            <>
                              {hasWorkspace && <div className="welcome-panel__dropdown-sep" />}
                              {otherWorkspaces.map(ws => (
                                <button
                                  key={ws.id}
                                  type="button"
                                  className="welcome-panel__dropdown-item"
                                  onClick={() => { void handleSwitchWorkspace(ws); }}
                                  title={ws.rootPath}
                                >
                                  <FolderOpen size={12} />
                                  <span className="welcome-panel__dropdown-name">{ws.name}</span>
                                </button>
                              ))}
                            </>
                          )}
                        </div>
                      )}
                    </span>
                  </span>
                  <span className="welcome-panel__narrative-sentence__text">
                    {isCoworkSession ? t('welcome.projectCowork') : t('welcome.project')}
                  </span>
                </span>
              </>
            )}
          </p>
        </div>

        {/* Cowork examples */}
        {isCoworkSession && (
          <div className="welcome-panel__cowork">
            <CoworkExampleCards resetKey={0} onSelectPrompt={p => handleQuickActionClick(p)} />
          </div>
        )}
      </div>
    </div>
  );
};

export default WelcomePanel;
