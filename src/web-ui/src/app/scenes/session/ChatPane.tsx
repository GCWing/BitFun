/**
 * ChatPane — AI Agent scene left pane.
 * Hosts FlowChat conversation panel.
 *
 * Renamed from panels/CenterPanel. All logic preserved.
 */

import React, { useCallback, memo, useEffect, useRef } from 'react';
import { FlowChatContainer, ChatInput } from '../../../flow_chat';
import { useCanvasStore } from '../../components/panels/content-canvas/stores/canvasStore';
import type { LineRange } from '@/component-library';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';

import './ChatPane.scss';

const log = createLogger('ChatPane');
const TASK_DETAIL_PANEL_EXPAND_DEFER_MS = 520;
const TASK_DETAIL_IDLE_TIMEOUT_MS = 300;

const preloadTaskDetailPanel = () => import('@/flow_chat/components/TaskDetailPanel');

interface ChatPaneProps {
  width: number;
  isFullscreen: boolean;
  workspacePath?: string;
  isDragging?: boolean;
  showChatInput?: boolean;
}

const ChatPaneInner: React.FC<ChatPaneProps> = ({
  width: _width,
  isFullscreen,
  workspacePath,
  isDragging: _isDragging = false,
  showChatInput = false,
}) => {
  const addTab = useCanvasStore(state => state.addTab);
  const deferredTaskDetailTimersRef = useRef<number[]>([]);
  const deferredTaskDetailIdleCallbacksRef = useRef<number[]>([]);

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
    const isProtocolPath = hasNonFileUriScheme(filePath);

    if (!isProtocolPath && !isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
      log.debug('Converting relative path to absolute', {
        relative: filePath,
        absolute: absoluteFilePath
      });
    }

    const { fileTabManager } = await import('@/shared/services/FileTabManager');
    fileTabManager.openFile({
      filePath: absoluteFilePath,
      fileName,
      workspacePath,
      jumpToRange: lineRange,
      mode: 'agent'
    });
  }, [workspacePath]);

  useEffect(() => {
    return () => {
      deferredTaskDetailTimersRef.current.forEach(timerId => window.clearTimeout(timerId));
      deferredTaskDetailTimersRef.current = [];
      if ('cancelIdleCallback' in window) {
        deferredTaskDetailIdleCallbacksRef.current.forEach(id => {
          window.cancelIdleCallback(id);
        });
      }
      deferredTaskDetailIdleCallbacksRef.current = [];
    };
  }, []);

  const addPanelTab = useCallback((tabInfo: any) => {
    addTab({
      type: tabInfo.type,
      title: tabInfo.title || 'New Tab',
      data: tabInfo.data,
      metadata: tabInfo.metadata
    });
  }, [addTab]);

  const handleTabOpen = useCallback((tabInfo: any) => {
    log.info('Opening tab', { tabInfo });
    if (!tabInfo || !tabInfo.type) {
      return;
    }

    if (tabInfo.type !== 'task-detail') {
      addPanelTab(tabInfo);
      return;
    }

    void preloadTaskDetailPanel();
    window.dispatchEvent(new CustomEvent('expand-right-panel'));

    const timerId = window.setTimeout(() => {
      deferredTaskDetailTimersRef.current = deferredTaskDetailTimersRef.current.filter(id => id !== timerId);

      const mountDetail = () => {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            addPanelTab(tabInfo);
          });
        });
      };

      if ('requestIdleCallback' in window) {
        const idleId = window.requestIdleCallback(() => {
          deferredTaskDetailIdleCallbacksRef.current = deferredTaskDetailIdleCallbacksRef.current.filter(id => id !== idleId);
          mountDetail();
        }, { timeout: TASK_DETAIL_IDLE_TIMEOUT_MS });
        deferredTaskDetailIdleCallbacksRef.current.push(idleId);
        return;
      }

      mountDetail();
    }, TASK_DETAIL_PANEL_EXPAND_DEFER_MS);

    deferredTaskDetailTimersRef.current.push(timerId);
  }, [addPanelTab]);

  return (
    <div
      className="bitfun-chat-pane__content"
      data-shortcut-scope="chat"
      data-fullscreen={isFullscreen}
    >
      <FlowChatContainer
        className="bitfun-chat-pane__chat-container"
        onOpenVisualization={(type, data) => {
          log.info('Opening visualization', { type, data });
        }}
        onFileViewRequest={handleFileViewRequest}
        onTabOpen={handleTabOpen}
        onSwitchToChatPanel={() => {}}
        config={{
          enableMarkdown: true,
          autoScroll: true,
          showTimestamps: false,
          theme: 'auto'
        }}
      />
      {showChatInput && <ChatInput onSendMessage={(_message: string) => {}} />}
    </div>
  );
};

const ChatPane = memo(ChatPaneInner);
ChatPane.displayName = 'ChatPane';

export default ChatPane;
