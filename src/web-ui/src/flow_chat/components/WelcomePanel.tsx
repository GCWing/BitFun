/**
 * Welcome panel shown in the empty chat state.
 * Layout mirrors WelcomeScene: centered container, left-aligned content.
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, ChevronDown, Check, GitBranch } from 'lucide-react';
import { gitAPI } from '../../infrastructure/api';
import type { GitWorkState } from '../../infrastructure/api/service-api/StartchatAgentAPI';
import { useApp } from '../../app/hooks/useApp';
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
  const [gitState, setGitState] = useState<GitWorkState | null>(null);
  const [workspaceDropdownOpen, setWorkspaceDropdownOpen] = useState(false);
  const [isSelectingWorkspace, setIsSelectingWorkspace] = useState(false);
  const workspaceDropdownRef = useRef<HTMLDivElement>(null);

  const { switchLeftPanelTab } = useApp();
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
  // code sessions use mode='agentic'; cowork sessions use mode='cowork'
  const showPanda = sessionModeLower !== 'code' && sessionModeLower !== 'agentic' && sessionModeLower !== 'cowork';

  const { document: identityDoc } = useAgentIdentityDocument(isClawSession ? workspacePath : '');
  const assistantName = isClawSession ? (identityDoc.name || '') : '';

  const greeting = useMemo(() => {
    const hour = new Date().getHours();
    const s = isCoworkSession ? 'Cowork' : isClawSession ? 'Claw' : '';
    if (hour >= 5 && hour < 12) return { title: t('welcome.greetingMorning'), subtitle: t(`welcome.subtitleMorning${s}`) };
    if (hour >= 12 && hour < 18) return { title: t('welcome.greetingAfternoon'), subtitle: t(`welcome.subtitleAfternoon${s}`) };
    if (hour >= 18 && hour < 23) return { title: t('welcome.greetingEvening'), subtitle: t(`welcome.subtitleEvening${s}`) };
    return { title: t('welcome.greetingNight'), subtitle: t(`welcome.subtitleNight${s}`) };
  }, [t, isCoworkSession, isClawSession]);

  const tagline = greeting.subtitle;
  const aiPartnerKey = isCoworkSession ? 'welcome.aiPartnerCowork' : isClawSession ? 'welcome.aiPartnerClaw' : 'welcome.aiPartner';

  const otherWorkspaces = useMemo(
    () => openedWorkspacesList.filter(ws => ws.id !== currentWorkspace?.id),
    [openedWorkspacesList, currentWorkspace?.id],
  );

  const handleGitClick = useCallback(() => {
    switchLeftPanelTab('git');
  }, [switchLeftPanelTab]);

  const isGitClean = useMemo(
    () => !!gitState && gitState.unstagedFiles === 0 && gitState.stagedFiles === 0 && gitState.unpushedCommits === 0,
    [gitState],
  );

  const buildGitNarrative = useCallback((): React.ReactNode => {
    if (!gitState) return null;
    const parts: { key: string; label: string; suffix: string }[] = [];
    if (gitState.unstagedFiles > 0)
      parts.push({ key: 'unstaged', label: t('welcome.gitUnstaged', { count: gitState.unstagedFiles }), suffix: t('welcome.waitingToStage') });
    if (gitState.stagedFiles > 0)
      parts.push({ key: 'staged', label: t('welcome.gitStaged', { count: gitState.stagedFiles }), suffix: t('welcome.stagedReady') });
    if (gitState.unpushedCommits > 0)
      parts.push({ key: 'unpushed', label: t('welcome.gitUnpushed', { count: gitState.unpushedCommits }), suffix: t('welcome.toPush') });
    if (parts.length === 0) return null;
    return (
      <>
        {t('welcome.currentlyHas')}
        {parts.map(({ key, label, suffix }, i) => (
          <React.Fragment key={key}>
            {i > 0 && t('welcome.commaSeparator')}
            <button type="button" className="welcome-panel__inline-btn" onClick={handleGitClick}>
              {label}
            </button>
            {' '}{suffix}
          </React.Fragment>
        ))}
        {t('welcome.period')}
      </>
    );
  }, [gitState, handleGitClick, t]);

  const loadGitState = useCallback(async (workspacePath: string) => {
    try {
      const isGitRepo = await gitAPI.isGitRepository(workspacePath);
      if (!isGitRepo) { setGitState(null); return; }
      const s = await gitAPI.getStatus(workspacePath);
      setGitState({
        currentBranch: s.current_branch,
        unstagedFiles: s.unstaged.length + s.untracked.length,
        stagedFiles: s.staged.length,
        unpushedCommits: s.ahead,
        aheadBehind: { ahead: s.ahead, behind: s.behind },
        modifiedFiles: [],
      });
    } catch (err) {
      log.warn('Failed to load git state', err);
      setGitState(null);
    }
  }, []);

  useEffect(() => {
    if (isCoworkSession || isClawSession || !currentWorkspace?.rootPath) { setGitState(null); return; }
    void loadGitState(currentWorkspace.rootPath);
  }, [currentWorkspace?.rootPath, isCoworkSession, isClawSession, loadGitState]);

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
      let path_manager = "/data/storage/el2/base/files/test";
      await openWorkspace(path_manager);
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
                {greeting.title}，{t(aiPartnerKey)}{isClawSession && assistantName ? `，${assistantName}` : ''}
              </h1>
              <p className="welcome-panel__tagline">{tagline}</p>
            </div>
          </div>
        </div>

        <div className="welcome-panel__divider" />

        {/* Narrative: workspace + git in natural language */}
        <div className="welcome-panel__narrative">
          <p className="welcome-panel__narrative-text">
            {isClawSession ? (
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
                    {!isCoworkSession && gitState && (
                      <>
                        <span className="welcome-panel__context-sep">/</span>
                        <button type="button" className="welcome-panel__inline-btn" onClick={handleGitClick}>
                          <GitBranch size={13} className="welcome-panel__inline-icon" />
                          {gitState.currentBranch}
                        </button>
                      </>
                    )}
                  </span>
                  <span className="welcome-panel__narrative-sentence__text">
                    {!isCoworkSession && gitState ? t('welcome.project') : t('welcome.projectCowork')}
                  </span>
                </span>
                {!isCoworkSession && gitState ? (
                  <span className="welcome-panel__narrative-git">
                    {isGitClean
                      ? <span className="welcome-panel__narrative-clean">{t('welcome.gitClean')}</span>
                      : buildGitNarrative()}
                  </span>
                ) : null}
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
