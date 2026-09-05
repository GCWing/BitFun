/**
 * Welcome panel shown in the empty chat state.
 * Layout mirrors WelcomeScene: centered container, left-aligned content.
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { FolderOpen, FolderPlus } from 'lucide-react';
import { Menu, MenuItem, MenuSeparator, Icon } from '@openbitfun/ui';
import { gitAPI } from '../../infrastructure/api';
import { useApp } from '../../app/hooks/useApp';
import { createLogger } from '@/shared/utils/logger';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import type { WorkspaceInfo } from '@/shared/types';
import CoworkExampleCards from './CoworkExampleCards';
import { useAgentIdentityDocument } from '@/app/scenes/my-agent/useAgentIdentityDocument';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import './WelcomePanel.css';
import './WelcomePanelSurface.scss';

const log = createLogger('WelcomePanel');

interface GitWorkState {
  currentBranch: string;
  unstagedFiles: number;
  stagedFiles: number;
  unpushedCommits: number;
}

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
  const { t: tCommon } = useTranslation('common');
  const [gitState, setGitState] = useState<GitWorkState | null>(null);
  const [workspaceDropdownOpen, setWorkspaceDropdownOpen] = useState(false);
  const [isSelectingWorkspace, setIsSelectingWorkspace] = useState(false);
  const workspaceDropdownRef = useRef<HTMLDivElement>(null);
  const workspaceTriggerRef = useRef<HTMLButtonElement>(null);
  const workspaceMenuRef = useRef<HTMLDivElement>(null);

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
  const workspaceMenuLayout = useAnchoredPopoverPosition({
    open: workspaceDropdownOpen,
    anchorRef: workspaceTriggerRef,
    popoverRef: workspaceMenuRef,
    preferredPlacement: 'bottom',
    alignment: 'start',
    gap: 4,
    layoutRevision: otherWorkspaces.length,
  });

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
            <button
              type="button"
              data-openbitfun-component="welcome-panel"
              data-openbitfun-part="gitAction"
              className="welcome-panel__inline-btn"
              onClick={handleGitClick}
            >
              {label}
            </button>
            {' '}{suffix}
          </React.Fragment>
        ))}
        {t('welcome.period')}
      </>
    );
  }, [gitState, handleGitClick, t]);

  const loadGitState = useCallback(async (
    workspacePath: string,
    shouldCancel: () => boolean = () => false,
  ) => {
    try {
      const isGitRepo = await gitAPI.isGitRepository(workspacePath);
      if (shouldCancel()) return;
      if (!isGitRepo) { setGitState(null); return; }
      const s = await gitAPI.getStatus(workspacePath, 'welcome_panel');
      if (shouldCancel()) return;
      setGitState({
        currentBranch: s.current_branch,
        unstagedFiles: s.unstaged.length + s.untracked.length,
        stagedFiles: s.staged.length,
        unpushedCommits: s.ahead,
      });
    } catch (err) {
      log.warn('Failed to load git state', err);
      setGitState(null);
    }
  }, []);

  useEffect(() => {
    if (isCoworkSession || isClawSession || !currentWorkspace?.rootPath) { setGitState(null); return; }
    let cancelled = false;
    void loadGitState(currentWorkspace.rootPath, () => cancelled);
    return () => {
      cancelled = true;
    };
  }, [currentWorkspace?.rootPath, isCoworkSession, isClawSession, loadGitState]);

  useEffect(() => {
    if (!workspaceDropdownOpen) return;
    const handlePointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        workspaceDropdownRef.current &&
        !workspaceDropdownRef.current.contains(target) &&
        !workspaceMenuRef.current?.contains(target)
      ) {
        setWorkspaceDropdownOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      setWorkspaceDropdownOpen(false);
      workspaceTriggerRef.current?.focus();
    };
    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [workspaceDropdownOpen]);

  const handleSwitchWorkspace = useCallback(async (ws: WorkspaceInfo) => {
    try { setWorkspaceDropdownOpen(false); await switchWorkspace(ws); }
    catch (err) { log.warn('Failed to switch workspace', err); }
  }, [switchWorkspace]);

  const handleOpenOtherFolder = useCallback(async () => {
    try {
      setWorkspaceDropdownOpen(false);
      setIsSelectingWorkspace(true);
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: tCommon('header.selectProjectDirectory'),
      });
      if (selected) await openWorkspace(selected);
    } catch (err) {
      log.warn('Failed to open workspace folder', err);
    } finally {
      setIsSelectingWorkspace(false);
    }
  }, [openWorkspace, tCommon]);

  const handleCreateWorkspace = useCallback(() => {
    setWorkspaceDropdownOpen(false);
    window.dispatchEvent(new Event('nav:new-project'));
  }, []);

  const handleQuickActionClick = useCallback((cmd: string) => {
    onQuickAction?.(cmd);
  }, [onQuickAction]);

  return (
    <div data-openbitfun-component="welcome-panel" data-openbitfun-part="root" className={`welcome-panel ${className}`}>
      <div data-openbitfun-component="welcome-panel" data-openbitfun-part="content" className="welcome-panel__content">
        {/* Greeting */}
        <div data-openbitfun-component="welcome-panel" data-openbitfun-part="greeting" className="welcome-panel__greeting">
          <div className="welcome-panel__greeting-inner">
            <div className="welcome-panel__greeting-text">
              <h1 data-openbitfun-component="welcome-panel" data-openbitfun-part="heading" className="welcome-panel__heading">
                {greeting.title}，{t(aiPartnerKey)}{isClawSession && assistantName ? `，${assistantName}` : ''}
              </h1>
              <p data-openbitfun-component="welcome-panel" data-openbitfun-part="tagline" className="welcome-panel__tagline">{tagline}</p>
            </div>
          </div>
        </div>

        <div data-openbitfun-component="welcome-panel" data-openbitfun-part="divider" className="welcome-panel__divider" />

        {/* Narrative: workspace + git in natural language */}
        <div data-openbitfun-component="welcome-panel" data-openbitfun-part="narrative" className="welcome-panel__narrative">
          <p className="welcome-panel__narrative-text">
            {isClawSession ? (
              t('welcome.narrativeClaw')
            ) : !hasWorkspace ? (
              <>
                {t('welcome.noWorkspaceHint')}
                <button
                  type="button"
                  data-openbitfun-component="welcome-panel"
                  data-openbitfun-part="openWorkspaceAction"
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
                        ref={workspaceTriggerRef}
                        type="button"
                        data-openbitfun-component="welcome-panel"
                        data-openbitfun-part="workspaceAction"
                        data-openbitfun-state={workspaceDropdownOpen ? 'open' : undefined}
                        className={`welcome-panel__inline-btn welcome-panel__inline-btn--interactive${workspaceDropdownOpen ? ' welcome-panel__inline-btn--active' : ''}`}
                        onClick={() => setWorkspaceDropdownOpen(v => !v)}
                        disabled={isSelectingWorkspace}
                        title={currentWorkspace?.rootPath}
                        aria-haspopup="menu"
                        aria-expanded={workspaceDropdownOpen}
                      >
                        <FolderOpen size={13} className="welcome-panel__inline-icon" />
                        {currentWorkspace?.name || t('shared:features.workspace')}
                        <Icon name="chevron-down" size="lg" style={{ width: 11, height: 11 }} className={`welcome-panel__inline-chevron${workspaceDropdownOpen ? ' welcome-panel__inline-chevron--open' : ''}`} />
                      </button>
                      {workspaceDropdownOpen && createPortal(
                        <Menu
                          ref={workspaceMenuRef}
                          data-openbitfun-component="welcome-panel"
                          data-openbitfun-part="workspaceMenu"
                          data-openbitfun-placement={workspaceMenuLayout?.placement ?? 'bottom'}
                          className="welcome-panel__dropdown"
                          style={{
                            top: `${workspaceMenuLayout?.top ?? 0}px`,
                            left: `${workspaceMenuLayout?.left ?? 0}px`,
                            visibility: workspaceMenuLayout ? 'visible' : 'hidden',
                          }}
                          autoFocusFirstItem
                          aria-label={t('shared:features.workspace')}
                        >
                          <MenuItem
                            data-openbitfun-component="welcome-panel"
                            data-openbitfun-part="workspaceItem"
                            leading={<FolderPlus size={12} />}
                            onClick={() => { void handleCreateWorkspace(); }}
                          >
                            {tCommon('header.newProject')}
                          </MenuItem>
                          {(hasWorkspace || otherWorkspaces.length > 0) && <MenuSeparator />}
                          {hasWorkspace && currentWorkspace && (
                            <MenuItem
                              role="menuitemradio"
                              checked
                              aria-disabled="true"
                              leading={<FolderOpen size={12} />}
                              metadata={<Icon name="check-line" size="lg" style={{ width: 11, height: 11 }} />}
                            >
                              {currentWorkspace.name}
                            </MenuItem>
                          )}
                          {otherWorkspaces.length > 0 && (
                            <>
                              {hasWorkspace && currentWorkspace && <MenuSeparator />}
                              {otherWorkspaces.map(ws => (
                                <MenuItem
                                  key={ws.id}
                                  data-openbitfun-component="welcome-panel"
                                  data-openbitfun-part="workspaceItem"
                                  leading={<FolderOpen size={12} />}
                                  onClick={() => { void handleSwitchWorkspace(ws); }}
                                  title={ws.rootPath}
                                >
                                  {ws.name}
                                </MenuItem>
                              ))}
                            </>
                          )}
                        </Menu>,
                        getAppearanceOverlayHost(),
                      )}
                    </span>
                    {!isCoworkSession && gitState && (
                      <>
                        <span className="welcome-panel__context-sep">/</span>
                        <button
                          type="button"
                          data-openbitfun-component="welcome-panel"
                          data-openbitfun-part="gitAction"
                          className="welcome-panel__inline-btn"
                          onClick={handleGitClick}
                        >
                          <Icon name="git" size="lg" style={{ width: 13, height: 13 }} className="welcome-panel__inline-icon" />
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
          <div data-openbitfun-component="welcome-panel" data-openbitfun-part="cowork" className="welcome-panel__cowork">
            <CoworkExampleCards resetKey={0} onSelectPrompt={p => handleQuickActionClick(p)} />
          </div>
        )}
      </div>
    </div>
  );
};

export default WelcomePanel;
