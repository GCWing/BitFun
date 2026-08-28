/**
 * FileViewerNav — scene-specific navigation for the file viewer scene.
 *
 * Header mirrors the directory NavItem (Folder icon + label, same font-size /
 * height / padding) so the transition from MainNav feels like the item
 * "expanded in-place". Navigation back is handled by NavBar's back button.
 */

import React, { useState, useCallback } from 'react';
import { IconButton, Tooltip } from '@bitfun/ui';
import { Folder, Search as SearchIcon, List, FilePlus, FolderPlus, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { useI18n } from '@/infrastructure/i18n';

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
    <div data-bf-component="file-viewer-nav" data-bf-part="root" className="bitfun-file-viewer-nav">
      <div className="bitfun-file-viewer-nav__header" data-bf-component="file-viewer-nav" data-bf-part="header">
        <span className="bitfun-file-viewer-nav__icon" data-bf-component="file-viewer-nav" data-bf-part="icon" aria-hidden="true">
          <Folder size={15} />
        </span>
        <span className="bitfun-file-viewer-nav__label" data-bf-component="file-viewer-nav" data-bf-part="label">
          {t('nav.items.project')}
        </span>
        {currentWorkspace?.rootPath && (
        <span className="bitfun-file-viewer-nav__actions" data-bf-component="file-viewer-nav" data-bf-part="actions">
            {viewMode === 'tree' && explorerToolbar && (
              <>
                <Tooltip content={tTools('fileTree.newFile')} placement="bottom">
                  <IconButton
                    size="sm"
                    aria-label={tTools('fileTree.newFile')}
                    icon={<FilePlus />}
                    onClick={explorerToolbar.onNewFile}
                  />
                </Tooltip>
                <Tooltip content={tTools('fileTree.newFolder')} placement="bottom">
                  <IconButton
                    size="sm"
                    aria-label={tTools('fileTree.newFolder')}
                    icon={<FolderPlus />}
                    onClick={explorerToolbar.onNewFolder}
                  />
                </Tooltip>
                <Tooltip content={tTools('fileTree.refresh')} placement="bottom">
                  <IconButton
                    size="sm"
                    aria-label={tTools('fileTree.refresh')}
                    icon={<RefreshCw />}
                    onClick={explorerToolbar.onRefresh}
                  />
                </Tooltip>
              </>
            )}
            <Tooltip
              content={viewMode === 'tree' ? tFiles('actions.switchToSearch') : tFiles('actions.switchToTree')}
              placement="bottom"
            >
              <IconButton
                size="sm"
                aria-label={viewMode === 'tree' ? tFiles('actions.switchToSearch') : tFiles('actions.switchToTree')}
                icon={viewMode === 'tree' ? <SearchIcon /> : <List />}
                onClick={handleToggleViewMode}
              />
            </Tooltip>
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
