 
import { createContext, useContext } from 'react';

export type ChatMessage = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  metadata?: Record<string, any>;
}

export interface ChatState {
  messages: ChatMessage[];
  input: string;
  isProcessing: boolean;
  error: string | null;
}

export interface ChatActions {
  addMessage: (message: ChatMessage) => void;
  updateMessage: (messageId: string, updater: (message: ChatMessage) => ChatMessage) => void;
  setInput: (input: string) => void;
  setProcessing: (processing: boolean) => void;
  setError: (error: string | null) => void;
  clearChat: () => void;
  clearError: () => void;
}

export interface ChatContextType {
  state: ChatState;
  actions: ChatActions;
}

const ChatContext = createContext<ChatContextType | undefined>(undefined);

export const useChat = () => {
  const context = useContext(ChatContext);
  if (!context) {
    throw new Error('useChat must be used within a ChatProvider');
  }
  return context;
};
export { ChatContext };
