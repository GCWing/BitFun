import React, { ReactNode, useCallback, useState } from 'react';
import {
  ChatContext,
  type ChatActions,
  type ChatContextType,
  type ChatMessage,
  type ChatState,
} from './ChatContext';

interface ChatProviderProps {
  children: ReactNode;
}

export const ChatProvider: React.FC<ChatProviderProps> = ({ children }) => {
  const [state, setState] = useState<ChatState>({
    messages: [],
    input: '',
    isProcessing: false,
    error: null,
  });

  const addMessage = useCallback((message: ChatMessage) => {
    setState((prev) => ({
      ...prev,
      messages: [...prev.messages, message],
    }));
  }, []);

  const updateMessage = useCallback((messageId: string, updater: (message: ChatMessage) => ChatMessage) => {
    setState((prev) => ({
      ...prev,
      messages: prev.messages.map((msg) => (msg.id === messageId ? updater(msg) : msg)),
    }));
  }, []);

  const setInput = useCallback((input: string) => {
    setState((prev) => ({
      ...prev,
      input,
    }));
  }, []);

  const setProcessing = useCallback((processing: boolean) => {
    setState((prev) => ({
      ...prev,
      isProcessing: processing,
    }));
  }, []);

  const setError = useCallback((error: string | null) => {
    setState((prev) => ({
      ...prev,
      error,
    }));
  }, []);

  const clearChat = useCallback(() => {
    setState((prev) => ({
      ...prev,
      messages: [],
      input: '',
      error: null,
    }));
  }, []);

  const clearError = useCallback(() => {
    setState((prev) => ({
      ...prev,
      error: null,
    }));
  }, []);

  const actions: ChatActions = {
    addMessage,
    updateMessage,
    setInput,
    setProcessing,
    setError,
    clearChat,
    clearError,
  };

  const contextValue: ChatContextType = {
    state,
    actions,
  };

  return <ChatContext.Provider value={contextValue}>{children}</ChatContext.Provider>;
};
