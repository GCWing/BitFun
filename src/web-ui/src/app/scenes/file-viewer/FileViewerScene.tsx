/**
 * FileViewerScene — standalone file viewing scene.
 *
 * Uses ContentCanvas in project mode so file tabs are managed independently
 * from the AI Agent AuxPane tab set.
 */

import React from 'react';
import { ContentCanvas } from '../../components/panels/content-canvas';
import { CanvasStoreModeContext } from '../../components/panels/content-canvas/stores';
import './FileViewerScene.scss';

interface FileViewerSceneProps {
  workspacePath?: string;
}

const FileViewerScene: React.FC<FileViewerSceneProps> = ({ workspacePath }) => {
  return (
    <CanvasStoreModeContext.Provider value="project">
      <div className="bitfun-file-viewer-scene">
        <ContentCanvas workspacePath={workspacePath} mode="project" />
      </div>
    </CanvasStoreModeContext.Provider>
  );
};

export default FileViewerScene;
