/**
 * Left panel component
 * Singleton display panel - only displays one active panel content
 *
 * Uses a "mount-on-first-visit" strategy: heavy panels (git, project-context)
 * are only mounted when the user first navigates to them, and stay mounted
 * afterwards so that internal state is preserved. Lightweight panels (sessions,
 * files, terminal) are always mounted for instant switching.
 */

import React, { memo, useState, useEffect } from 'react';
import { PanelType } from '../../types';

import { GitPanel } from '../../../tools/git';
import { ProjectContextPanel } from '../../../tools/project-context';
import { FilesPanel } from './';
import SessionsPanel from './SessionsPanel';
import TerminalSessionsPanel from './TerminalSessionsPanel';

import './LeftPanel.scss';

interface LeftPanelProps {
  activeTab: PanelType;
  width: number;
  isFullscreen: boolean;
  workspacePath?: string;
  onSwitchTab: (tab: PanelType) => void;
  isDragging?: boolean;
}

/** Panels that are always mounted for instant response. */
const ALWAYS_MOUNT: Set<PanelType> = new Set(['sessions', 'files', 'terminal']);

const LeftPanel: React.FC<LeftPanelProps> = ({
  activeTab,
  width: _width,
  isFullscreen,
  workspacePath,
  onSwitchTab: _onSwitchTab,
  isDragging: _isDragging = false
}) => {
  const [mountedTabs, setMountedTabs] = useState<Set<PanelType>>(
    () => new Set([...ALWAYS_MOUNT, activeTab])
  );

  useEffect(() => {
    setMountedTabs(prev => {
      if (prev.has(activeTab)) return prev;
      const next = new Set(prev);
      next.add(activeTab);
      return next;
    });
  }, [activeTab]);

  return (
    <div 
      className="bitfun-left-panel__content"
      data-fullscreen={isFullscreen}
    >
      <div style={{ display: activeTab === 'sessions' ? 'block' : 'none', height: '100%' }}>
        <SessionsPanel />
      </div>

      <div style={{ display: activeTab === 'files' ? 'block' : 'none', height: '100%' }}>
        <FilesPanel 
          workspacePath={workspacePath}
        />
      </div>

      {mountedTabs.has('git') && (
        <div style={{ display: activeTab === 'git' ? 'block' : 'none', height: '100%' }}>
          <GitPanel 
            workspacePath={workspacePath}
            isActive={activeTab === 'git'}
          />
        </div>
      )}

      {mountedTabs.has('project-context') && (
        <div style={{ display: activeTab === 'project-context' ? 'block' : 'none', height: '100%' }}>
          <ProjectContextPanel 
            workspacePath={workspacePath || ''}
            isActive={activeTab === 'project-context'}
          />
        </div>
      )}

      <div style={{ display: activeTab === 'terminal' ? 'block' : 'none', height: '100%' }}>
        <TerminalSessionsPanel />
      </div>
    </div>
  );
};

export default memo(LeftPanel);
