/**
 * ChatPane — AI Agent scene left pane.
 * Hosts FlowChat conversation panel.
 *
 * Renamed from panels/CenterPanel. All logic preserved.
 */

import React, { useCallback, memo } from 'react';
import { FlowChatContainer } from '../../../flow_chat';
import { useCanvasStore } from '../../components/panels/content-canvas/stores/canvasStore';
import type { LineRange } from '@/component-library';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';

import './ChatPane.scss';

const log = createLogger('ChatPane');

interface ChatPaneProps {
  width: number;
  isFullscreen: boolean;
  workspacePath?: string;
  isDragging?: boolean;
}

const ChatPaneInner: React.FC<ChatPaneProps> = ({
  width: _width,
  isFullscreen,
  workspacePath,
  isDragging: _isDragging = false
}) => {
  const addTab = useCanvasStore(state => state.addTab);

  const handleFileViewRequest = useCallback(async (
    filePath: string,
    fileName: string,
    lineRange?: LineRange
  ) => {
    log.info('File view request', { filePath, fileName, lineRange, workspacePath });

    if (!filePath) {
      log.warn('Invalid file path');
      return;
    }

    let absoluteFilePath = filePath;
    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);

    if (!isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
      log.debug('Converting relative path to absolute', {
        relative: filePath,
        absolute: absoluteFilePath
      });
    }

    const { fileTabManager } = await import('@/shared/services/FileTabManager');
    fileTabManager.openFile({
      filePath: absoluteFilePath,
      jumpToRange: lineRange,
      mode: 'agent'
    });
  }, [workspacePath]);

  return (
    <div
      className="bitfun-chat-pane__content"
      data-fullscreen={isFullscreen}
    >
      <FlowChatContainer
        className="bitfun-chat-pane__chat-container"
        onOpenVisualization={(type, data) => {
          log.info('Opening visualization', { type, data });
        }}
        onFileViewRequest={handleFileViewRequest}
        onTabOpen={(tabInfo) => {
          log.info('Opening tab', { tabInfo });
          if (tabInfo && tabInfo.type) {
            addTab({
              type: tabInfo.type,
              title: tabInfo.title || 'New Tab',
              data: tabInfo.data,
              metadata: tabInfo.metadata
            });
          }
        }}
        onSwitchToChatPanel={() => {}}
        config={{
          enableMarkdown: true,
          autoScroll: true,
          showTimestamps: false,
          theme: 'auto'
        }}
      />
    </div>
  );
};

const ChatPane = memo(ChatPaneInner);
ChatPane.displayName = 'ChatPane';

export default ChatPane;
