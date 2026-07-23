/**
 * FileViewerNav — scene-specific navigation for the file viewer scene.
 *
 * Header mirrors the directory NavItem (Folder icon + label, same font-size /
 * height / padding) so the transition from MainNav feels like the item
 * "expanded in-place". Navigation back is handled by NavBar's back button.
 */

import React, { useState, useCallback } from 'react';
import { Folder, Search as SearchIcon, List, FilePlus, FolderPlus, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { useI18n } from '@/infrastructure/i18n';
import { IconButton } from '@/component-library';
import type { FileExplorerToolbarHandlers } from '@/tools/file-system';
import FilesPanel from '../../components/panels/FilesPanel';
import './FileViewerNav.scss';

const FileViewerNav: React.FC = () => {
  const { workspace: currentWorkspace } = useCurrentWorkspace();
  const { t } = useI18n('common');
  const { t: tTools } = useI18n('tools');
  const { t: tFiles } = useTranslation('panels/files');
  const [viewMode, setViewMode] = useState<'tree' | 'search'>('tree');
  const [explorerToolbar, setExplorerToolbar] = useState<FileExplorerToolbarHandlers | null>(null);

  const handleToggleViewMode = useCallback(() => {
    setViewMode(prev => prev === 'tree' ? 'search' : 'tree');
  }, []);

  return (
    <div className="bitfun-file-viewer-nav">
      <div className="bitfun-file-viewer-nav__header">
        <span className="bitfun-file-viewer-nav__icon" aria-hidden="true">
          <Folder size={15} />
        </span>
        <span className="bitfun-file-viewer-nav__label">
          {t('nav.items.project')}
        </span>
        {currentWorkspace?.rootPath && (
          <span className="bitfun-file-viewer-nav__actions">
            {viewMode === 'tree' && explorerToolbar && (
              <>
                <IconButton
                  size="xs"
                  variant="ghost"
                  onClick={explorerToolbar.onNewFile}
                  tooltip={tTools('fileTree.newFile')}
                  tooltipPlacement="bottom"
                >
                  <FilePlus size={14} />
                </IconButton>
                <IconButton
                  size="xs"
                  variant="ghost"
                  onClick={explorerToolbar.onNewFolder}
                  tooltip={tTools('fileTree.newFolder')}
                  tooltipPlacement="bottom"
                >
                  <FolderPlus size={14} />
                </IconButton>
                <IconButton
                  size="xs"
                  variant="ghost"
                  onClick={explorerToolbar.onRefresh}
                  tooltip={tTools('fileTree.refresh')}
                  tooltipPlacement="bottom"
                >
                  <RefreshCw size={14} />
                </IconButton>
              </>
            )}
            <IconButton
              size="xs"
              onClick={handleToggleViewMode}
              tooltip={viewMode === 'tree' ? tFiles('actions.switchToSearch') : tFiles('actions.switchToTree')}
              tooltipPlacement="bottom"
            >
              {viewMode === 'tree' ? <SearchIcon size={14} /> : <List size={14} />}
            </IconButton>
          </span>
        )}
      </div>
      <FilesPanel
        workspacePath={currentWorkspace?.rootPath}
        hideHeader
        hideExplorerToolbar
        onExplorerToolbarApi={setExplorerToolbar}
        viewMode={viewMode}
        onViewModeChange={setViewMode}
      />
    </div>
  );
};

export default FileViewerNav;
