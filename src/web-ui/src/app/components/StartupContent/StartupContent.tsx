/**
 * Startup Content Component
 * Displays brand section and action area content when no workspace is open
 * Integrated into AppLayout to avoid page switching
 */

import React, { useState, useCallback, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { 
  Folder, 
  FolderOpen,
  Code, 
  FileText, 
  Clock, 
  Plus,
  ChevronRight,
  ChevronDown,
  ChevronUp
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { WorkspaceInfo, WorkspaceType } from '../../../shared/types/global-state';
import { systemAPI } from '../../../infrastructure/api';
import { getVersionInfo, formatVersion } from '../../../shared/utils/version';
import { createLogger } from '@/shared/utils/logger';
import RubiksCube3D from './RubiksCube3D';
import './StartupContent.scss';

const log = createLogger('StartupContent');

interface StartupContentProps {
  onWorkspaceSelected: (workspacePath: string, projectDescription?: string) => void;
  isTransitioning?: boolean;
}

/**
 * Startup Content Component
 * Displays brand section and action area
 */
const StartupContent: React.FC<StartupContentProps> = ({ 
  onWorkspaceSelected,
  isTransitioning = false
}) => {
  const { t } = useTranslation();
  const {
    recentWorkspaces,
    loading
  } = useWorkspaceContext();

  const [isSelecting, setIsSelecting] = useState(false);
  const [workspacePathExists, setWorkspacePathExists] = useState<Map<string, boolean>>(new Map());
  const [isHistoryExpanded, setIsHistoryExpanded] = useState(false);


  const historyWorkspaces = recentWorkspaces.slice(1);
  const hasMoreWorkspaces = historyWorkspaces.length > 6;
  const displayedWorkspaces = isHistoryExpanded ? historyWorkspaces : historyWorkspaces.slice(0, 6);

  useEffect(() => {
    const checkWorkspacePaths = async () => {
      if (recentWorkspaces.length === 0) {
        setWorkspacePathExists(new Map());
        return;
      }

      const existsMap = new Map<string, boolean>();
      recentWorkspaces.forEach(workspace => {
        existsMap.set(workspace.rootPath, true);
      });
      setWorkspacePathExists(existsMap);

      Promise.all(
        recentWorkspaces.map(async (workspace) => {
          try {
            const exists = await systemAPI.checkPathExists(workspace.rootPath);
            existsMap.set(workspace.rootPath, exists);
          } catch (error) {
            existsMap.set(workspace.rootPath, false);
          }
        })
      ).then(() => {
        setWorkspacePathExists(new Map(existsMap));
      });
    };

    checkWorkspacePaths();
  }, [recentWorkspaces]);

  const handleContinueLastWork = useCallback(async () => {
    if (recentWorkspaces.length > 0) {
      const lastWorkspace = recentWorkspaces[0];
      onWorkspaceSelected(lastWorkspace.rootPath);
    }
  }, [recentWorkspaces, onWorkspaceSelected]);

  const handleWorkspaceClick = useCallback(async (workspace: WorkspaceInfo) => {
    try {
      if (!workspace.rootPath || workspace.rootPath.trim() === '') {
        throw new Error(t('startup.invalidWorkspacePath'));
      }
      onWorkspaceSelected(workspace.rootPath);
    } catch (error) {
      log.error('Failed to open workspace', error);
    }
  }, [onWorkspaceSelected, t]);

  const handleOpenNewWorkspace = useCallback(async () => {
    try {
      setIsSelecting(true);
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('startup.selectWorkspaceDirectory')
      });

      if (selected && typeof selected === 'string') {
        onWorkspaceSelected(selected);
      }
    } catch (error) {
      log.error('Failed to select directory', error);
    } finally {
      setIsSelecting(false);
    }
  }, [onWorkspaceSelected, t]);

  const getWorkspaceTypeIcon = (type: WorkspaceType) => {
    switch (type) {
      case WorkspaceType.SingleProject:
        return <Code size={18} />;
      case WorkspaceType.MultiProject:
        return <Folder size={18} />;
      case WorkspaceType.Documentation:
        return <FileText size={18} />;
      default:
        return <FolderOpen size={18} />;
    }
  };

  const formatDate = (dateString: string) => {
    try {
      const date = new Date(dateString);
      const now = new Date();
      const diffTime = Math.abs(now.getTime() - date.getTime());
      const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
      
      if (diffDays === 1) return t('time.yesterday');
      if (diffDays < 7) return t('startup.daysAgo', { count: diffDays });
      if (diffDays < 30) return t('startup.weeksAgo', { count: Math.ceil(diffDays / 7) });
      return date.toLocaleDateString();
    } catch {
      return t('startup.unknownTime');
    }
  };

  const containerClassName = [
    'startup-content',
    isTransitioning ? 'startup-content--transitioning' : ''
  ].filter(Boolean).join(' ');

  return (
    <div className={containerClassName} data-testid="startup-container">
      {/* Split layout */}
      <div className="startup-content__split-layout">
        {/* ========== Left Brand Section - Hero Style ========== */}
        <div className="startup-content__brand-section">
          {/* Divider line */}
          <div className="startup-content__divider-line"></div>
          
          {/* Cube background */}
          <div className="startup-content__cube-bg">
            <RubiksCube3D />
          </div>

          {/* Text content layer */}
          <div className="startup-content__hero-content">
            {/* Main title */}
            <h1 className="startup-content__hero-title">BitFun</h1>
            
            {/* Subtitle */}
            <p className="startup-content__hero-subtitle">{t('startup.subtitle')}</p>
          </div>

          {/* Version info */}
          <div className="startup-content__version">
            {(() => {
              const versionInfo = getVersionInfo();
              return `Version ${formatVersion(versionInfo.version, versionInfo.isDev)}`;
            })()}
          </div>
        </div>

        {/* ========== Right Action Section ========== */}
        <div className="startup-content__actions-section">
          <div className="startup-content__actions-container">
            
            {/* Main action area - Quick start (only shown when workspace exists) */}
            {recentWorkspaces.length > 0 && (
              <div className="startup-content__quick-actions">
                {/* Continue last work */}
                {(() => {
                  const lastWorkspace = recentWorkspaces[0];
                  const pathExists = workspacePathExists.get(lastWorkspace.rootPath) ?? true;
                  return (
                    <button 
                      className="startup-content__continue-btn"
                      onClick={handleContinueLastWork}
                      disabled={loading || !pathExists}
                      style={{ opacity: !pathExists ? 0.5 : 1 }}
                      title={`${lastWorkspace.rootPath}${!pathExists ? ` (${t('startup.pathNotExist')})` : ''}`}
                    >
                      <div className="startup-content__continue-icon">
                        <FolderOpen size={24} />
                      </div>
                      <div className="startup-content__continue-content">
                        <span className="startup-content__continue-title">
                          {t('startup.continueLastWork')}
                        </span>
                        <span className="startup-content__continue-project">
                          {lastWorkspace.name}
                        </span>
                        <span className="startup-content__continue-path">
                          {lastWorkspace.rootPath}
                        </span>
                      </div>
                      <ChevronRight size={20} className="startup-content__continue-arrow" />
                    </button>
                  );
                })()}

                {/* Open new workspace */}
                <button 
                  className="startup-content__open-btn"
                  onClick={handleOpenNewWorkspace}
                  disabled={isSelecting || loading}
                  data-testid="startup-open-folder-btn"
                >
                  <Plus size={18} />
                  <span>{isSelecting ? t('startup.selecting') : t('startup.openFolder')}</span>
                </button>
              </div>
            )}

            {/* Recent workspaces list */}
            {historyWorkspaces.length > 0 && (
              <div className={`startup-content__history-section ${isHistoryExpanded ? 'startup-content__history-section--expanded' : ''}`}>
                <h3 className="startup-content__history-title">
                  <Clock size={14} />
                  {t('startup.recentlyOpened')}
                  {hasMoreWorkspaces && (
                    <span className="startup-content__history-count">
                      {historyWorkspaces.length} {t('startup.projects')}
                    </span>
                  )}
                </h3>
                <div className="startup-content__history-scroll-container">
                  <div className="startup-content__history-grid">
                    {displayedWorkspaces.map((workspace) => {
                      const pathExists = workspacePathExists.get(workspace.rootPath) ?? true;
                      return (
                        <button
                          key={workspace.id}
                          className="startup-content__history-item"
                          onClick={() => handleWorkspaceClick(workspace)}
                          disabled={loading || !pathExists}
                          style={{ opacity: !pathExists ? 0.5 : 1 }}
                          title={workspace.rootPath}
                        >
                          <div className="startup-content__history-item-icon">
                            {getWorkspaceTypeIcon(workspace.workspaceType)}
                          </div>
                          <div className="startup-content__history-item-info">
                            <span className="startup-content__history-item-name">
                              {workspace.name}
                            </span>
                            <span className="startup-content__history-item-time">
                              {formatDate(workspace.lastAccessed)}
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </div>
                {/* Show more/collapse button */}
                {hasMoreWorkspaces && (
                  <button 
                    className="startup-content__history-toggle"
                    onClick={() => setIsHistoryExpanded(!isHistoryExpanded)}
                  >
                    {isHistoryExpanded ? (
                      <>
                        <ChevronUp size={14} />
                        <span>{t('startup.collapse')}</span>
                      </>
                    ) : (
                      <>
                        <ChevronDown size={14} />
                        <span>{t('startup.showMore', { count: historyWorkspaces.length - 6 })}</span>
                      </>
                    )}
                  </button>
                )}
              </div>
            )}

            {/* Empty state - consistent dashed/solid line style with workspace state */}
            {recentWorkspaces.length === 0 && (
              <div className="startup-content__empty-state">
                <div className="startup-content__empty-icon">
                  <Folder size={28} />
                </div>
                <div className="startup-content__empty-content">
                  <p className="startup-content__empty-text">
                    {t('startup.noProjectsYet')}
                  </p>
                  <p className="startup-content__empty-hint">
                    {t('startup.startYourJourney')}
                  </p>
                </div>
                <button 
                  className="startup-content__empty-btn"
                  onClick={handleOpenNewWorkspace}
                  disabled={isSelecting || loading}
                  data-testid="startup-open-folder-btn"
                >
                  <Plus size={16} />
                  <span>{isSelecting ? t('startup.selecting') : t('startup.openFolder')}</span>
                </button>
              </div>
            )}
            
          </div>
        </div>
      </div>
    </div>
  );
};

export default StartupContent;
export { StartupContent };
