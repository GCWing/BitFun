import React, { useEffect, useRef, useState, useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { RemoteSessionManager } from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';

interface ChatPageProps {
  sessionMgr: RemoteSessionManager;
  sessionId: string;
  onBack: () => void;
}

const ChatPage: React.FC<ChatPageProps> = ({ sessionMgr, sessionId, onBack }) => {
  const {
    getMessages,
    setMessages,
    appendMessage,
    updateLastMessage,
    isStreaming,
    setIsStreaming,
    setError,
  } = useMobileStore();

  const messages = getMessages(sessionId);
  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);
  // Track accumulated text for the current streaming assistant message
  const accumulatedTextRef = useRef('');

  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);

  const loadMessages = useCallback(async (beforeId?: string) => {
    if (isLoadingMore || (!hasMore && beforeId)) return;
    
    try {
      setIsLoadingMore(true);
      const resp = await sessionMgr.getSessionMessages(sessionId, 50, beforeId);
      
      if (beforeId) {
        // Prepend older messages
        const currentMsgs = getMessages(sessionId);
        setMessages(sessionId, [...resp.messages, ...currentMsgs]);
      } else {
        // Initial load
        setMessages(sessionId, resp.messages);
      }
      
      setHasMore(resp.has_more);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setIsLoadingMore(false);
    }
  }, [sessionMgr, sessionId, setMessages, setError, getMessages, isLoadingMore, hasMore]);

  // Handle scroll for lazy loading
  const handleScroll = useCallback(() => {
    const container = messagesContainerRef.current;
    if (!container) return;
    
    // If scrolled near top, load more
    if (container.scrollTop < 100 && hasMore && !isLoadingMore) {
      const msgs = getMessages(sessionId);
      if (msgs.length > 0) {
        const firstMsgId = msgs[0].id;
        loadMessages(firstMsgId);
      }
    }
  }, [hasMore, isLoadingMore, getMessages, sessionId, loadMessages]);

  useEffect(() => {
    // Subscribe to session stream when entering
    sessionMgr.subscribeSession(sessionId).catch(console.error);
    
    // Initial load
    loadMessages();

    const unsub = sessionMgr.onStreamEvent((event) => {
      if (event.session_id !== sessionId) return;

      const eventType = event.event_type;

      if (eventType === 'stream_start') {
        setIsStreaming(true);
        accumulatedTextRef.current = '';
        appendMessage(sessionId, {
          id: `stream-${Date.now()}`,
          role: 'assistant',
          content: '',
          timestamp: new Date().toISOString(),
        });
      } else if (eventType === 'text_chunk') {
        const chunk = event.payload?.text || '';
        accumulatedTextRef.current += chunk;
        updateLastMessage(sessionId, accumulatedTextRef.current);
      } else if (eventType === 'thinking_chunk') {
        // Optionally show thinking content
        const chunk = event.payload?.content || '';
        accumulatedTextRef.current += chunk;
        updateLastMessage(sessionId, accumulatedTextRef.current);
      } else if (eventType === 'stream_end') {
        setIsStreaming(false);
        accumulatedTextRef.current = '';
      } else if (eventType === 'stream_error') {
        setIsStreaming(false);
        setError(event.payload?.error || 'Stream error');
        accumulatedTextRef.current = '';
      } else if (eventType === 'stream_cancelled') {
        setIsStreaming(false);
        accumulatedTextRef.current = '';
      } else if (eventType === 'tool_event') {
        // Append tool activity as a brief system message
        const toolEvt = event.payload?.tool_event;
        if (toolEvt?.event_type === 'Started') {
          const toolInfo = `[Tool: ${toolEvt.tool_name}]`;
          accumulatedTextRef.current += `\n\n${toolInfo}\n`;
          updateLastMessage(sessionId, accumulatedTextRef.current);
        }
      } else if (eventType === 'session_title') {
        // Title was generated; could update session list
      }
    });

    return () => {
      unsub();
      // Unsubscribe from session stream when leaving
      sessionMgr.unsubscribeSession(sessionId).catch(console.error);
    };
  }, [sessionId, sessionMgr, setIsStreaming, appendMessage, updateLastMessage, setError]); // Removed loadMessages from deps to avoid re-triggering

  useEffect(() => {
    // Only scroll to bottom on initial load or new message at bottom
    if (!isLoadingMore) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, isLoadingMore]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    if (!text || isStreaming) return;

    setInput('');
    appendMessage(sessionId, {
      id: `user-${Date.now()}`,
      role: 'user',
      content: text,
      timestamp: new Date().toISOString(),
    });

    try {
      await sessionMgr.sendMessage(sessionId, text);
    } catch (e: any) {
      setError(e.message);
    }
  }, [input, isStreaming, sessionId, sessionMgr, appendMessage, setError]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleCancel = async () => {
    try {
      await sessionMgr.cancelTask(sessionId);
    } catch {
      // best effort
    }
  };

  return (
    <div className="chat-page">
      <div className="chat-page__header">
        <button className="chat-page__back" onClick={onBack}>
          &larr;
        </button>
        <span className="chat-page__title">Session</span>
        {isStreaming && (
          <button className="chat-page__cancel" onClick={handleCancel}>
            Stop
          </button>
        )}
      </div>

      <div className="chat-page__messages" ref={messagesContainerRef} onScroll={handleScroll}>
        {isLoadingMore && <div className="chat-page__loading">Loading older messages...</div>}
        {messages.map((m) => (
          <div key={m.id} className={`chat-msg chat-msg--${m.role}`}>
            <div className="chat-msg__role">{m.role === 'user' ? 'You' : 'BitFun'}</div>
            <div className="chat-msg__content">
              <ReactMarkdown
                components={{
                  code({ className, children, ...props }) {
                    const match = /language-(\w+)/.exec(className || '');
                    const codeStr = String(children).replace(/\n$/, '');
                    return match ? (
                      <SyntaxHighlighter style={oneDark} language={match[1]} PreTag="div">
                        {codeStr}
                      </SyntaxHighlighter>
                    ) : (
                      <code className={className} {...props}>
                        {children}
                      </code>
                    );
                  },
                }}
              >
                {m.content}
              </ReactMarkdown>
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-page__input-bar">
        <textarea
          ref={inputRef}
          className="chat-page__input"
          placeholder="Type a message..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={1}
        />
        <button
          className="chat-page__send"
          onClick={handleSend}
          disabled={!input.trim() || isStreaming}
        >
          Send
        </button>
      </div>
    </div>
  );
};

export default ChatPage;
