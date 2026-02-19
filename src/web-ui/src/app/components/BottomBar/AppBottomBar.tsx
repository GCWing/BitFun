/**
 * Application bottom bar component.
 * Matches legacy AgentBottomBar styles and interactions for consistency.
 */

import React, { useState, useRef, useEffect } from 'react';
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
} from 'lucide-react';
import { useApp } from '../../hooks/useApp';
import { PanelType } from '../../types';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { useGitBasicInfo } from '../../../tools/git/hooks/useGitState';
import { useUnreadCount, useLatestTaskNotification } from '../../../shared/notification-system/hooks/useNotificationState';
import { notificationService } from '../../../shared/notification-system/services/NotificationService';
import { BranchQuickSwitch } from './BranchQuickSwitch';
import { Tooltip } from '@/component-library';
import { useI18n } from '../../../infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './AppBottomBar.scss';

const log = createLogger('AppBottomBar');

interface AppBottomBarProps {
  className?: string;
}

const AppBottomBar: React.FC<AppBottomBarProps> = ({
  className = ''
}) => {
  const { state, switchLeftPanelTab, toggleRightPanel } = useApp();
  const { t } = useI18n('components');
  const [animatingTab, setAnimatingTab] = useState<PanelType | null>(null);
  const notificationButtonRef = useRef<HTMLButtonElement | null>(null);
  const gitBranchRef = useRef<HTMLDivElement | null>(null);
  const [tooltipOffset, setTooltipOffset] = useState(0);
  const [showBranchSwitch, setShowBranchSwitch] = useState(false);
  
  // Workspace info
  const { workspaceName, workspacePath } = useCurrentWorkspace();
  
  // Centralized Git state; subscribe to basic info only.
  // GitStateManager handles:
  // - window focus events
  // - refresh after Git operations
  // - branch change events
  const {
    isRepository: isGitRepo,
    currentBranch: gitBranch,
    refresh: refreshGitState,
  } = useGitBasicInfo(workspacePath || '');
  
  // Notification system
  const unreadCount = useUnreadCount();
  // Latest task notification (sorted by created time)
  const activeNotification = useLatestTaskNotification();
  
  // Compute tooltip position to avoid overflow
  useEffect(() => {
    if (activeNotification && activeNotification.title && notificationButtonRef.current) {
      const buttonRect = notificationButtonRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const buttonCenter = buttonRect.left + buttonRect.width / 2;
      
      // Estimate tooltip width from text length
      const textLength = activeNotification.title?.length || 0;
      const estimatedWidth = Math.min(Math.max(textLength * 8, 120), 300);
      
      const tooltipLeft = buttonCenter - estimatedWidth / 2;
      const tooltipRight = buttonCenter + estimatedWidth / 2;
      
      let offset = 0;
      const rightPadding = 16; // Keep 16px right padding
      
      // Adjust only when exceeding the right edge
      if (tooltipRight > viewportWidth - rightPadding) {
        offset = viewportWidth - rightPadding - tooltipRight;
      }
      
      // Clamp to the left edge after adjustment
      if (tooltipLeft + offset < 16) {
        offset = 16 - tooltipLeft;
      }
      
      setTooltipOffset(offset);
    }
  }, [activeNotification]);
  
  // Note: the following listeners are handled by GitStateManager:
  // - window focus events
  // - Git operation completion
  // - branch change events
  // State stays in sync; no manual refresh needed.

  // Handle tab click
  const handleTabClick = (tab: PanelType) => {
    if (tab === state.layout.leftPanelActiveTab) return;

    setAnimatingTab(tab);
    switchLeftPanelTab(tab);
    
    // Reset animation state
    setTimeout(() => {
      setAnimatingTab(null);
    }, 350);
  };


  // Terminal sessions are event-driven; no manual load needed.

  const activeTab = state.layout.leftPanelActiveTab;

  return (
    <div 
      className={`bitfun-bottom-bar ${className} ${animatingTab ? 'is-animating' : ''}`}
    >
      {/* Main container */}
      <div className="bitfun-bottom-bar__container">
        {/* Left tab buttons */}
        <div className="bitfun-bottom-bar__tabs">
          {/* Sessions tab */}
          <Tooltip content={t('bottomBar.sessions')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'sessions' ? 'is-active' : ''} ${animatingTab === 'sessions' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('sessions')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  <MessageSquare size={14} />
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  <MessageSquareText size={14} />
                </span>
              </span>
            </button>
          </Tooltip>

          {/* Workflows tab */}
          <Tooltip content={t('bottomBar.workflows')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'workflows' ? 'is-active' : ''} ${animatingTab === 'workflows' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('workflows')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  <Workflow size={14} />
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  <SquareKanban size={14} />
                </span>
              </span>
            </button>
          </Tooltip>

          {/* File tree tab */}
          <Tooltip content={t('bottomBar.files')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'files' ? 'is-active' : ''} ${animatingTab === 'files' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('files')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  <Folder size={14} />
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  <FolderOpen size={14} />
                </span>
              </span>
            </button>
          </Tooltip>

          {/* Terminal tab */}
          <Tooltip content={t('bottomBar.terminal')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'terminal' ? 'is-active' : ''} ${animatingTab === 'terminal' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('terminal')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  <Terminal size={14} />
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  <TerminalSquare size={14} />
                </span>
              </span>
            </button>
          </Tooltip>

          {/* Project context tab */}
          <Tooltip content={t('bottomBar.projectContext')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'project-context' ? 'is-active' : ''} ${animatingTab === 'project-context' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('project-context')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  <Layers2 size={14} />
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  <Layers size={14} />
                </span>
              </span>
            </button>
          </Tooltip>

          {/* Git tab */}
          <Tooltip content={t('bottomBar.git')} placement="top">
            <button
              className={`bitfun-bottom-bar__tab-button ${activeTab === 'git' ? 'is-active' : ''} ${animatingTab === 'git' ? 'is-switching' : ''}`}
              onClick={() => handleTabClick('git')}
            >
              <span className="bitfun-bottom-bar__tab-icon bitfun-bottom-bar__tab-icon--dual">
                <span className="bitfun-bottom-bar__icon-inactive">
                  {/* Single-branch icon */}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="6" y1="3" x2="6" y2="15"/>
                    <circle cx="18" cy="6" r="3"/>
                    <circle cx="6" cy="18" r="3"/>
                    <path d="M18 9a9 9 0 0 1-9 9"/>
                  </svg>
                </span>
                <span className="bitfun-bottom-bar__icon-active">
                  {/* Merge icon */}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="5" cy="6" r="3"/>
                    <circle cx="5" cy="18" r="3"/>
                    <circle cx="19" cy="6" r="3"/>
                    <circle cx="19" cy="18" r="3"/>
                    <line x1="5" y1="9" x2="5" y2="15"/>
                    <line x1="19" y1="9" x2="19" y2="15"/>
                    <path d="M8 17h8"/>
                  </svg>
                </span>
              </span>
            </button>
          </Tooltip>

        </div>


        {/* Workspace info (right) */}
        <div className="bitfun-bottom-bar__workspace-info">
          {workspaceName && (
            <Tooltip content={workspacePath} placement="top">
              <div 
                className="bitfun-bottom-bar__info-item bitfun-bottom-bar__info-item--clickable bitfun-bottom-bar__info-item--workspace"
                onClick={() => {
                  // Switch to the file tree panel
                  handleTabClick('files');
                }}
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
                onClick={() => {
                  // Open branch switch panel
                  setShowBranchSwitch(true);
                }}
              >
                <GitBranch size={12} />
                <span className="bitfun-bottom-bar__info-text">{gitBranch}</span>
              </div>
            </Tooltip>
          )}
          
          {/* Quick branch switch panel */}
          {isGitRepo && workspacePath && (
            <BranchQuickSwitch
              isOpen={showBranchSwitch}
              onClose={() => setShowBranchSwitch(false)}
              repositoryPath={workspacePath}
              currentBranch={gitBranch || ''}
              anchorRef={gitBranchRef}
              onSwitchSuccess={() => {
                // Refresh Git info after switching for immediate update.
                refreshGitState({ force: true });
              }}
            />
          )}
          
          {/* Notification center button */}
          <button
            ref={notificationButtonRef}
            className={`bitfun-bottom-bar__notification-button ${activeNotification ? 'has-progress' : ''} ${activeNotification?.variant === 'loading' ? 'has-loading' : ''}`}
            onClick={() => {
              notificationService.toggleCenter();
            }}
          >
            {activeNotification ? (
              // Show active notification state.
              <>
                <div className="bitfun-bottom-bar__notification-progress">
                  {activeNotification.variant === 'loading' ? (
                    // Loading: spinner only.
                    <div className="bitfun-bottom-bar__notification-loading-icon">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" className="bitfun-bottom-bar__spinner">
                        <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
                      </svg>
                    </div>
                  ) : (
                    // Progress: show ring.
                    <div className="bitfun-bottom-bar__notification-progress-icon">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <circle cx="12" cy="12" r="10" opacity="0.2" />
                        <path
                          d="M12 2 A 10 10 0 0 1 22 12"
                          strokeLinecap="round"
                        style={{
                          strokeDasharray: `${(activeNotification.progress || 0) * 0.628} 62.8`,
                          transform: 'rotate(-90deg)',
                          transformOrigin: 'center'
                        }}
                        />
                      </svg>
                    </div>
                  )}
                  <span className="bitfun-bottom-bar__notification-progress-text">
                    {activeNotification.variant === 'loading' ? (
                      // Loading: show message text (e.g. "cargo clippy").
                      activeNotification.message
                    ) : (
                      // Progress: show percentage.
                      (() => {
                        const mode = activeNotification.progressMode || (activeNotification.textOnly ? 'text-only' : 'percentage');
                        if (mode === 'fraction' && activeNotification.current !== undefined && activeNotification.total !== undefined) {
                          return `${activeNotification.current}/${activeNotification.total}`;
                        }
                        return `${Math.round(activeNotification.progress || 0)}%`;
                      })()
                    )}
                  </span>
                </div>
                {/* Hover tooltip */}
                <div 
                  className="bitfun-bottom-bar__notification-tooltip"
                  style={{
                    transform: `translateX(calc(-50% + ${tooltipOffset}px))`
                  }}
                >
                  <div 
                    className="bitfun-bottom-bar__notification-tooltip-content"
                    style={{
                      '--tooltip-offset': `${tooltipOffset}px`
                    } as React.CSSProperties}
                  >
                    {activeNotification.title}
                  </div>
                </div>
              </>
            ) : (
              // When idle, show icon based on unread count.
              unreadCount > 0 ? (
                <BellDot size={12} className="bitfun-bottom-bar__notification-icon--has-message" />
              ) : (
                <Bell size={12} />
              )
            )}
          </button>
        </div>
      </div>
    </div>
  );
};

export default AppBottomBar;