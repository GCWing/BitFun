import { useState, useEffect, useCallback } from 'react';
import { ToolExecutionService } from '../shared/services/tool-execution-service';
import { ToolDisplayMessage, ToolExecutionInfo } from '../shared/types/tool-display';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useToolExecution');

export interface UseToolExecutionOptions {
  autoConnect?: boolean;
  eventTypes?: string[];
  maxMessages?: number;
}

export interface UseToolExecutionReturn {
  toolMessages: ToolDisplayMessage[];
  activeExecutions: ToolExecutionInfo[];
  hasActiveExecutions: boolean;
  clearToolMessages: () => void;
  addToolMessage: (message: ToolDisplayMessage) => void;
}

export const useToolExecution = (
  options: UseToolExecutionOptions = {}
): UseToolExecutionReturn => {
  const {
    autoConnect = true,
    eventTypes = ['all'],
    maxMessages = 50
  } = options;

  const [toolMessages, setToolMessages] = useState<ToolDisplayMessage[]>([]);
  const [activeExecutions, setActiveExecutions] = useState<ToolExecutionInfo[]>([]);

  const handleToolEvent = useCallback((message: ToolDisplayMessage) => {
    if (!message.id || message.id === 'tool_exec_undefined' || message.id === 'tool_result_undefined') {
      log.warn('Ignoring invalid tool message', { messageId: message.id });
      return;
    }
    
    setToolMessages(prev => {
      const exists = prev.some(msg => msg.id === message.id);
      if (exists) {
        return prev;
      }
      
      const newMessages = [...prev, message];
      if (newMessages.length > maxMessages) {
        return newMessages.slice(-maxMessages);
      }
      
      return newMessages;
    });

    if (message.type === 'tool_use' || message.toolExecution) {
      const service = ToolExecutionService.getInstance();
      setActiveExecutions(service.getActiveExecutions());
    }
  }, [maxMessages]);

  useEffect(() => {
    if (!autoConnect) return;

    const service = ToolExecutionService.getInstance();
    const cleanupFunctions: (() => void)[] = [];

    eventTypes.forEach(eventType => {
      const cleanup = service.onToolEvent(eventType, handleToolEvent);
      cleanupFunctions.push(cleanup);
    });

    setActiveExecutions(service.getActiveExecutions());

    return () => {
      cleanupFunctions.forEach(cleanup => cleanup());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoConnect]);

  const clearToolMessages = useCallback(() => {
    setToolMessages([]);
  }, []);

  const addToolMessage = useCallback((message: ToolDisplayMessage) => {
    setToolMessages(prev => {
      const exists = prev.some(msg => msg.id === message.id);
      if (exists) return prev;
      
      return [...prev, message];
    });
  }, []);

  const hasActiveExecutions = activeExecutions.length > 0;

  return {
    toolMessages,
    activeExecutions,
    hasActiveExecutions,
    clearToolMessages,
    addToolMessage
  };
};
