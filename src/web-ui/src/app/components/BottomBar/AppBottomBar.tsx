/**
 * Application bottom bar — dual-anchor + expandable wings.
 *
 * Layout per domain:
 *   [anchor]  [pinned-active-wing-item?]  [toggle]  [collapsible-grid]
 *
 * The active wing item is ALWAYS visible (pinned slot, outside the collapsible grid).
 * The collapsible grid contains only the NON-active wing items.
 *
 * Toggle visual states:
 *   • Collapsed, no pinned item  →  › (ChevronRight)
 *   • Collapsed, has pinned item →  ··· (MoreHorizontal, "more items hidden")
 *   • Expanded                   →  ‹ (ChevronLeft)
 */

import React, { useState, useRef, useEffect, useCallback } from 'react';
import {
  Folder,
  FolderOpen,
  GitBranch,
  Bell,
  BellDot,
  Layers,
  Layers2,
  MessageSquare,
  MessageSquareText,
  Terminal,
  TerminalSquare,
  Workflow,
  SquareKanban,
  ChevronRight,
  ChevronLeft,
  MoreHorizontal,
  Blocks,
  Package2,
} from 'lucide-react';
import { useApp } from '../../hooks/useApp';
import { PanelType } from '../../types';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { useGitBasicInfo } from '../../../tools/git/hooks/useGitState';
import {
  useUnreadCount,
  useLatestTaskNotification,
} from '../../../shared/notification-system/hooks/useNotificationState';
import { notificationService } from '../../../shared/notification-system/services/NotificationService';
import { BranchQuickSwitch } from './BranchQuickSwitch';
import { Tooltip } from '@/component-library';
import { useI18n } from '../../../infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './AppBottomBar.scss';

const log = createLogger('AppBottomBar');

const AI_WING_TABS: PanelType[] = ['workflows', 'project-context', 'capabilities'];
const DEV_WING_TABS: PanelType[] = ['terminal', 'git'];

/** Wing item icon content — dual-icon where applicable. */
function WingItemContent({ tab }: { tab: PanelType }) {
  switch (tab) {
    case 'workflows':
      return (
        <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
          <span className="bitfun-bottom-bar__icon-inactive"><Workflow size={13} /></span>
          <span className="bitfun-bottom-bar__icon-active"><SquareKanban size={13} /></span>
        </span>
      );
    case 'project-context':
      return (
        <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
          <span className="bitfun-bottom-bar__icon-inactive"><Layers2 size={13} /></span>
          <span className="bitfun-bottom-bar__icon-active"><Layers size={13} /></span>
        </span>
      );
    case 'capabilities':
      return (
        <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
          <span className="bitfun-bottom-bar__icon-inactive"><Blocks size={13} /></span>
          <span className="bitfun-bottom-bar__icon-active"><Package2 size={13} /></span>
        </span>
      );
    case 'terminal':
      return (
        <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
          <span className="bitfun-bottom-bar__icon-inactive"><Terminal size={13} /></span>
          <span className="bitfun-bottom-bar__icon-active"><TerminalSquare size={13} /></span>
        </span>
      );
    case 'git':
      return (
        <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
          <span className="bitfun-bottom-bar__icon-inactive">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="6" y1="3" x2="6" y2="15" />
              <circle cx="18" cy="6" r="3" />
              <circle cx="6" cy="18" r="3" />
              <path d="M18 9a9 9 0 0 1-9 9" />
            </svg>
          </span>
          <span className="bitfun-bottom-bar__icon-active">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="5" cy="6" r="3" />
              <circle cx="5" cy="18" r="3" />
              <circle cx="19" cy="6" r="3" />
              <circle cx="19" cy="18" r="3" />
              <line x1="5" y1="9" x2="5" y2="15" />
              <line x1="19" y1="9" x2="19" y2="15" />
              <path d="M8 17h8" />
            </svg>
          </span>
        </span>
      );
    default:
      return null;
  }
}

interface AppBottomBarProps {
  className?: string;
}

const AppBottomBar: React.FC<AppBottomBarProps> = ({ className = '' }) => {
  const { state, switchLeftPanelTab } = useApp();
  const { t } = useI18n('components');
  const [animatingTab, setAnimatingTab] = useState<PanelType | null>(null);
  const [aiWingExpanded, setAiWingExpanded] = useState(false);
  const [devWingExpanded, setDevWingExpanded] = useState(false);
  const bottomBarRef = useRef<HTMLDivElement | null>(null);
  const notificationButtonRef = useRef<HTMLButtonElement | null>(null);
  const gitBranchRef = useRef<HTMLDivElement | null>(null);
  const [tooltipOffset, setTooltipOffset] = useState(0);
  const [showBranchSwitch, setShowBranchSwitch] = useState(false);

  const { workspaceName, workspacePath } = useCurrentWorkspace();
  const { isRepository: isGitRepo, currentBranch: gitBranch, refresh: refreshGitState } =
    useGitBasicInfo(workspacePath || '');
  const unreadCount = useUnreadCount();
  const activeNotification = useLatestTaskNotification();

  const activeTab = state.layout.leftPanelActiveTab;

  // Which wing tab is currently active (pinned) for each domain
  const aiPinnedTab = AI_WING_TABS.includes(activeTab) ? activeTab : null;
  const devPinnedTab = DEV_WING_TABS.includes(activeTab) ? activeTab : null;

  // Non-active items go into the collapsible grid
  const aiGridTabs = AI_WING_TABS.filter(t => t !== activeTab);
  const devGridTabs = DEV_WING_TABS.filter(t => t !== activeTab);

  // Auto-expand wing when a wing tab becomes active
  useEffect(() => {
    if (aiPinnedTab) { setAiWingExpanded(true); setDevWingExpanded(false); }
    if (devPinnedTab) { setDevWingExpanded(true); setAiWingExpanded(false); }
  }, [aiPinnedTab, devPinnedTab]);

  // Collapse wings when focus moves to center or right panel
  useEffect(() => {
    if (!aiWingExpanded && !devWingExpanded) return;

    const handleOutsideClick = (e: MouseEvent) => {
      const target = e.target as Node;
      // Stay open when clicking inside the bottom bar itself
      if (bottomBarRef.current?.contains(target)) return;
      // Stay open when clicking inside the left panel (the user is still "on the left side")
      if (document.querySelector('.bitfun-left-panel')?.contains(target)) return;
      // Click landed in center or right area → collapse both wings
      setAiWingExpanded(false);
      setDevWingExpanded(false);
    };

    document.addEventListener('mousedown', handleOutsideClick);
    return () => document.removeEventListener('mousedown', handleOutsideClick);
  }, [aiWingExpanded, devWingExpanded]);

  // Tooltip overflow avoidance for notification
  useEffect(() => {
    if (activeNotification?.title && notificationButtonRef.current) {
      const rect = notificationButtonRef.current.getBoundingClientRect();
      const vw = window.innerWidth;
      const center = rect.left + rect.width / 2;
      const len = activeNotification.title.length || 0;
      const estW = Math.min(Math.max(len * 8, 120), 300);
      const right = center + estW / 2;
      let offset = 0;
      if (right > vw - 16) offset = vw - 16 - right;
      if (center - estW / 2 + offset < 16) offset = 16 - (center - estW / 2);
      setTooltipOffset(offset);
    }
  }, [activeNotification]);

  const handleTabClick = useCallback((tab: PanelType) => {
    if (tab === activeTab) return;
    setAnimatingTab(tab);
    switchLeftPanelTab(tab);
    setTimeout(() => setAnimatingTab(null), 350);
  }, [activeTab, switchLeftPanelTab]);

  const handleSessionsAnchorClick = useCallback(() => {
    if (activeTab !== 'sessions') {
      setAnimatingTab('sessions');
      switchLeftPanelTab('sessions');
      setTimeout(() => setAnimatingTab(null), 350);
    }
    setAiWingExpanded(true);
    setDevWingExpanded(false);
  }, [activeTab, switchLeftPanelTab]);

  const handleFilesAnchorClick = useCallback(() => {
    if (activeTab !== 'files') {
      setAnimatingTab('files');
      switchLeftPanelTab('files');
      setTimeout(() => setAnimatingTab(null), 350);
    }
    setDevWingExpanded(true);
    setAiWingExpanded(false);
  }, [activeTab, switchLeftPanelTab]);

  const toggleAiWing = useCallback(() => {
    setAiWingExpanded(prev => {
      const next = !prev;
      if (next) setDevWingExpanded(false);
      return next;
    });
  }, []);

  const toggleDevWing = useCallback(() => {
    setDevWingExpanded(prev => {
      const next = !prev;
      if (next) setAiWingExpanded(false);
      return next;
    });
  }, []);

  const getTabTooltip = useCallback((tab: PanelType): string => {
    switch (tab) {
      case 'workflows': return t('bottomBar.workflows');
      case 'project-context': return t('bottomBar.projectContext');
      case 'capabilities': return t('bottomBar.capabilities');
      case 'terminal': return t('bottomBar.terminal');
      case 'git': return t('bottomBar.git');
      default: return '';
    }
  }, [t]);

  /** Render a single wing item button. */
  const renderWingItem = useCallback((tab: PanelType, pinned = false) => {
    const isActive = activeTab === tab;
    const isAnimating = animatingTab === tab;
    return (
      <Tooltip key={`${tab}-${pinned ? 'pinned' : 'grid'}`} content={getTabTooltip(tab)} placement="top">
        <button
          className={[
            'bitfun-bottom-bar__wing-item',
            isActive ? 'is-active' : '',
            isAnimating ? 'is-switching' : '',
            pinned ? 'bitfun-bottom-bar__wing-item--pinned' : '',
          ].filter(Boolean).join(' ')}
          onClick={() => handleTabClick(tab)}
        >
          <WingItemContent tab={tab} />
        </button>
      </Tooltip>
    );
  }, [activeTab, animatingTab, getTabTooltip, handleTabClick]);

  /** Wing toggle button — three visual states. */
  const WingToggle = ({
    expanded, hasPinned, domain, onClick,
  }: {
    expanded: boolean;
    hasPinned: boolean;
    domain: 'ai' | 'dev';
    onClick: () => void;
  }) => {
    const tooltip = expanded
      ? t('bottomBar.aiWingExpand')
      : hasPinned
        ? t('bottomBar.aiWingExpand')
        : t(domain === 'ai' ? 'bottomBar.aiWingExpand' : 'bottomBar.devWingExpand');

    return (
      <Tooltip content={tooltip} placement="top">
        <button
          className={[
            'bitfun-bottom-bar__wing-toggle',
            `bitfun-bottom-bar__wing-toggle--${domain}`,
            expanded ? 'is-expanded' : '',
            hasPinned && !expanded ? 'is-pinned' : '',
          ].filter(Boolean).join(' ')}
          onClick={onClick}
          aria-label={tooltip}
        >
          {expanded ? (
            <ChevronLeft size={8} />
          ) : hasPinned ? (
            <MoreHorizontal size={9} />
          ) : (
            <ChevronRight size={9} />
          )}
        </button>
      </Tooltip>
    );
  };

  return (
    <div ref={bottomBarRef} className={`bitfun-bottom-bar ${className} ${animatingTab ? 'is-animating' : ''}`}>
      <div className="bitfun-bottom-bar__container">
        <div className="bitfun-bottom-bar__tabs">

          {/* ── AI domain ── */}

          {/* Anchor: Sessions */}
          <Tooltip content={t('bottomBar.sessions')} placement="top">
            <button
              className={[
                'bitfun-bottom-bar__anchor-button',
                'bitfun-bottom-bar__anchor-button--ai',
                activeTab === 'sessions' ? 'is-active' : '',
                animatingTab === 'sessions' ? 'is-switching' : '',
              ].filter(Boolean).join(' ')}
              onClick={handleSessionsAnchorClick}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive"><MessageSquare size={15} /></span>
                <span className="bitfun-bottom-bar__icon-active"><MessageSquareText size={15} /></span>
              </span>
            </button>
          </Tooltip>

          {/* AI pinned active wing item — always visible when a wing tab is active */}
          {aiPinnedTab && (
            <div
              key={aiPinnedTab}
              className="bitfun-bottom-bar__wing-pinned bitfun-bottom-bar__wing-pinned--ai"
            >
              {renderWingItem(aiPinnedTab, true)}
            </div>
          )}

          {/* AI wing toggle */}
          <WingToggle
            expanded={aiWingExpanded}
            hasPinned={!!aiPinnedTab}
            domain="ai"
            onClick={toggleAiWing}
          />

          {/* AI collapsible grid — only non-active items */}
          {aiGridTabs.length > 0 && (
            <div className={`bitfun-bottom-bar__wing ${aiWingExpanded ? 'is-expanded' : ''}`}>
              <div className="bitfun-bottom-bar__wing-inner">
                {aiGridTabs.map((tab, i) => (
                  <React.Fragment key={tab}>
                    {/* Dot separator before capabilities (last AI item) */}
                    {tab === 'capabilities' && i > 0 && (
                      <span className="bitfun-bottom-bar__wing-dot-sep" />
                    )}
                    {renderWingItem(tab, false)}
                  </React.Fragment>
                ))}
              </div>
            </div>
          )}

          {/* Domain separator */}
          <span className="bitfun-bottom-bar__domain-sep" />

          {/* ── Dev domain ── */}

          {/* Anchor: Files */}
          <Tooltip content={t('bottomBar.files')} placement="top">
            <button
              className={[
                'bitfun-bottom-bar__anchor-button',
                'bitfun-bottom-bar__anchor-button--dev',
                activeTab === 'files' ? 'is-active' : '',
                animatingTab === 'files' ? 'is-switching' : '',
              ].filter(Boolean).join(' ')}
              onClick={handleFilesAnchorClick}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive"><Folder size={15} /></span>
                <span className="bitfun-bottom-bar__icon-active"><FolderOpen size={15} /></span>
              </span>
            </button>
          </Tooltip>

          {/* Dev pinned active wing item */}
          {devPinnedTab && (
            <div
              key={devPinnedTab}
              className="bitfun-bottom-bar__wing-pinned bitfun-bottom-bar__wing-pinned--dev"
            >
              {renderWingItem(devPinnedTab, true)}
            </div>
          )}

          {/* Dev wing toggle */}
          <WingToggle
            expanded={devWingExpanded}
            hasPinned={!!devPinnedTab}
            domain="dev"
            onClick={toggleDevWing}
          />

          {/* Dev collapsible grid — only non-active items */}
          {devGridTabs.length > 0 && (
            <div className={`bitfun-bottom-bar__wing ${devWingExpanded ? 'is-expanded' : ''}`}>
              <div className="bitfun-bottom-bar__wing-inner">
                {devGridTabs.map(tab => renderWingItem(tab, false))}
              </div>
            </div>
          )}

        </div>

        {/* ── Workspace info (right) ── */}
        <div className="bitfun-bottom-bar__workspace-info">
          {workspaceName && (
            <Tooltip content={workspacePath} placement="top">
              <div
                className="bitfun-bottom-bar__info-item bitfun-bottom-bar__info-item--clickable bitfun-bottom-bar__info-item--workspace"
                onClick={() => handleTabClick('files')}
              >
                <Folder size={12} />
                <span className="bitfun-bottom-bar__info-text">{workspaceName}</span>
              </div>
            </Tooltip>
          )}

          {isGitRepo && gitBranch && (
            <Tooltip content={t('bottomBar.clickToSelectBranch')} placement="top">
              <div
                ref={gitBranchRef}
                className="bitfun-bottom-bar__info-item bitfun-bottom-bar__info-item--clickable bitfun-bottom-bar__info-item--git"
                onClick={() => setShowBranchSwitch(true)}
              >
                <GitBranch size={12} />
                <span className="bitfun-bottom-bar__info-text">{gitBranch}</span>
              </div>
            </Tooltip>
          )}

          {isGitRepo && workspacePath && (
            <BranchQuickSwitch
              isOpen={showBranchSwitch}
              onClose={() => setShowBranchSwitch(false)}
              repositoryPath={workspacePath}
              currentBranch={gitBranch || ''}
              anchorRef={gitBranchRef}
              onSwitchSuccess={() => refreshGitState({ force: true })}
            />
          )}

          {/* Notification center button */}
          <button
            ref={notificationButtonRef}
            className={[
              'bitfun-bottom-bar__notification-button',
              activeNotification ? 'has-progress' : '',
              activeNotification?.variant === 'loading' ? 'has-loading' : '',
            ].filter(Boolean).join(' ')}
            onClick={() => notificationService.toggleCenter()}
          >
            {activeNotification ? (
              <>
                <div className="bitfun-bottom-bar__notification-progress">
                  {activeNotification.variant === 'loading' ? (
                    <div className="bitfun-bottom-bar__notification-loading-icon">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
                        stroke="currentColor" strokeWidth="2.5"
                        className="bitfun-bottom-bar__spinner">
                        <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
                      </svg>
                    </div>
                  ) : (
                    <div className="bitfun-bottom-bar__notification-progress-icon">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
                        stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10" opacity="0.2" />
                        <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round"
                          style={{
                            strokeDasharray: `${(activeNotification.progress || 0) * 0.628} 62.8`,
                            transform: 'rotate(-90deg)',
                            transformOrigin: 'center',
                          }}
                        />
                      </svg>
                    </div>
                  )}
                  <span className="bitfun-bottom-bar__notification-progress-text">
                    {activeNotification.variant === 'loading'
                      ? activeNotification.message
                      : (() => {
                          const mode = activeNotification.progressMode ||
                            (activeNotification.textOnly ? 'text-only' : 'percentage');
                          if (mode === 'fraction' &&
                            activeNotification.current !== undefined &&
                            activeNotification.total !== undefined) {
                            return `${activeNotification.current}/${activeNotification.total}`;
                          }
                          return `${Math.round(activeNotification.progress || 0)}%`;
                        })()}
                  </span>
                </div>
                <div className="bitfun-bottom-bar__notification-tooltip"
                  style={{ transform: `translateX(calc(-50% + ${tooltipOffset}px))` }}>
                  <div className="bitfun-bottom-bar__notification-tooltip-content"
                    style={{ '--tooltip-offset': `${tooltipOffset}px` } as React.CSSProperties}>
                    {activeNotification.title}
                  </div>
                </div>
              </>
            ) : (
              unreadCount > 0
                ? <BellDot size={12} className="bitfun-bottom-bar__notification-icon--has-message" />
                : <Bell size={12} />
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default AppBottomBar;
