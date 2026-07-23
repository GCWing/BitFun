/**
 * File navigation actions for Modern FlowChat.
 */

import { useCallback } from 'react';
import path from 'path-browserify';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import { fileTabManager } from '@/shared/services/FileTabManager';
import type { LineRange } from '@/component-library';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';

const log = createLogger('useFlowChatFileActions');

interface UseFlowChatFileActionsOptions {
  workspacePath?: string;
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
}

export function useFlowChatFileActions({
  workspacePath,
  onFileViewRequest,
}: UseFlowChatFileActionsOptions) {
  const handleFileViewRequest = useCallback((
    filePath: string,
    fileName: string,
    lineRange?: LineRange,
  ) => {
    log.debug('File view request', {
      filePath,
      fileName,
      hasLineRange: !!lineRange,
      hasExternalCallback: !!onFileViewRequest,
    });

    if (onFileViewRequest) {
      onFileViewRequest(filePath, fileName, lineRange);
      return;
    }

    let absoluteFilePath = filePath;
    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);
    const isProtocolPath = hasNonFileUriScheme(filePath);

    if (!isProtocolPath && !isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
      log.debug('Converted relative path to absolute', {
        relative: filePath,
        absolute: absoluteFilePath,
      });
    }

    try {
      fileTabManager.openFile({
        filePath: absoluteFilePath,
        fileName,
        workspacePath,
        jumpToRange: lineRange,
        mode: 'agent',
      });
    } catch (error) {
      log.error('File navigation failed', error);
      notificationService.error(`Unable to open file: ${absoluteFilePath}`);
    }
  }, [onFileViewRequest, workspacePath]);

  return {
    handleFileViewRequest,
  };
}
