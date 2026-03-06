import React, { useEffect, useRef, useState, useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import {
  RemoteSessionManager,
  SessionPoller,
  type PollResponse,
  type ActiveTurnSnapshot,
  type RemoteToolStatus,
  type ChatMessage,
  type ChatMessageItem,
} from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';
import { useTheme } from '../theme';

interface ChatPageProps {
  sessionMgr: RemoteSessionManager;
  sessionId: string;
  sessionName?: string;
  onBack: () => void;
}

// ─── Markdown ───────────────────────────────────────────────────────────────

const MarkdownContent: React.FC<{ content: string }> = ({ content }) => (
  <ReactMarkdown
    remarkPlugins={[remarkGfm]}
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
    {content}
  </ReactMarkdown>
);

// ─── Thinking (ModelThinkingDisplay-style) ───────────────────────────────────

const ThinkingBlock: React.FC<{ thinking: string; streaming?: boolean }> = ({ thinking, streaming }) => {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ atTop: true, atBottom: true });

  const handleScroll = useCallback(() => {
    const el = wrapperRef.current;
    if (!el) return;
    setScrollState({
      atTop: el.scrollTop < 4,
      atBottom: el.scrollHeight - el.scrollTop - el.clientHeight < 4,
    });
  }, []);

  if (!thinking && !streaming) return null;

  const charCount = thinking.length;
  const label = streaming && charCount === 0
    ? 'Thinking...'
    : `Thought for ${charCount} characters`;

  return (
    <div className={`chat-thinking ${streaming ? 'chat-thinking--streaming' : ''}`}>
      <button className="chat-thinking__toggle" onClick={() => setOpen(o => !o)}>
        <span className={`chat-thinking__chevron ${open ? 'is-open' : ''}`}>
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
            <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </span>
        <span className="chat-thinking__label">{label}</span>
      </button>

      <div className={`chat-thinking__expand-container ${open ? 'is-expanded' : ''}`}>
        <div className="chat-thinking__expand-inner">
          {thinking && (
            <div
              className={`chat-thinking__content-wrapper ${scrollState.atTop ? 'at-top' : ''} ${scrollState.atBottom ? 'at-bottom' : ''}`}
              ref={wrapperRef}
              onScroll={handleScroll}
            >
              <div className="chat-thinking__content">
                <MarkdownContent content={thinking} />
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

// ─── Tool Card ──────────────────────────────────────────────────────────────

const TOOL_TYPE_MAP: Record<string, string> = {
  explore: 'Explore',
  read_file: 'Read',
  write_file: 'Write',
  list_directory: 'LS',
  bash: 'Shell',
  glob: 'Glob',
  grep: 'Grep',
  create_file: 'Write',
  delete_file: 'Delete',
  execute_subagent: 'Task',
  search: 'Search',
  edit_file: 'Edit',
  web_search: 'Web',
};

const ToolCard: React.FC<{ tool: RemoteToolStatus; now: number }> = ({ tool, now }) => {
  const toolKey = tool.name.toLowerCase().replace(/[\s-]/g, '_');
  const typeLabel = TOOL_TYPE_MAP[toolKey] || TOOL_TYPE_MAP[tool.name] || 'Tool';
  const isRunning = tool.status === 'running';
  const isCompleted = tool.status === 'completed';

  const durationLabel = isCompleted && tool.duration_ms != null
    ? `${(tool.duration_ms / 1000).toFixed(1)}s`
    : isRunning && tool.start_ms
    ? `${((now - tool.start_ms) / 1000).toFixed(1)}s`
    : '';

  const statusClass = isRunning ? 'running' : isCompleted ? 'done' : 'error';

  return (
    <div className={`chat-tool-card chat-tool-card--${statusClass}`}>
      <div className="chat-tool-card__row">
        <span className="chat-tool-card__icon">
          {isRunning ? (
            <span className="chat-tool-card__spinner" />
          ) : isCompleted ? (
            <span className="chat-tool-card__check">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M3 8.5L6.5 12L13 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>
            </span>
          ) : (
            <span className="chat-tool-card__error-icon">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/></svg>
            </span>
          )}
        </span>
        <span className="chat-tool-card__name">{tool.name}</span>
        <span className="chat-tool-card__type">{typeLabel}</span>
        {durationLabel && (
          <span className="chat-tool-card__duration">{durationLabel}</span>
        )}
      </div>
    </div>
  );
};

const TOOL_LIST_COLLAPSE_THRESHOLD = 2;

const ToolList: React.FC<{ tools: RemoteToolStatus[]; now: number }> = ({ tools, now }) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);

  useEffect(() => {
    if (tools.length > prevCountRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
    prevCountRef.current = tools.length;
  }, [tools.length]);

  if (!tools || tools.length === 0) return null;

  if (tools.length <= TOOL_LIST_COLLAPSE_THRESHOLD) {
    return (
      <div className="chat-tool-list">
        {tools.map((tc) => (
          <ToolCard key={tc.id} tool={tc} now={now} />
        ))}
      </div>
    );
  }

  const runningCount = tools.filter(t => t.status === 'running').length;
  const doneCount = tools.filter(t => t.status === 'completed').length;

  return (
    <div className="chat-tool-list chat-tool-list--collapsed">
      <div className="chat-tool-list__header">
        <span className="chat-tool-list__count">{tools.length} tool calls</span>
        <span className="chat-tool-list__stats">
          {doneCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--done">{doneCount} done</span>}
          {runningCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--running">{runningCount} running</span>}
        </span>
      </div>
      <div className="chat-tool-list__scroll" ref={scrollRef}>
        {tools.map((tc) => (
          <ToolCard key={tc.id} tool={tc} now={now} />
        ))}
      </div>
    </div>
  );
};

// ─── Typing indicator ───────────────────────────────────────────────────────

const TypingDots: React.FC = () => (
  <span className="chat-msg__typing">
    <span /><span /><span />
  </span>
);

// ─── AskUserQuestion Card ─────────────────────────────────────────────────

interface AskQuestionCardProps {
  tool: RemoteToolStatus;
  onAnswer: (toolId: string, answers: any) => void;
}

const AskQuestionCard: React.FC<AskQuestionCardProps> = ({ tool, onAnswer }) => {
  const questions: any[] = tool.tool_input?.questions || [];
  const [selected, setSelected] = useState<Record<number, string | string[]>>({});
  const [customTexts, setCustomTexts] = useState<Record<number, string>>({});
  const [submitted, setSubmitted] = useState(false);

  if (questions.length === 0) return null;

  const handleSelect = (qIdx: number, label: string, multi: boolean) => {
    setSelected(prev => {
      if (multi) {
        const arr = (prev[qIdx] as string[] | undefined) || [];
        return { ...prev, [qIdx]: arr.includes(label) ? arr.filter(l => l !== label) : [...arr, label] };
      }
      return { ...prev, [qIdx]: prev[qIdx] === label ? undefined! : label };
    });
  };

  const handleSubmit = () => {
    const answers: Record<string, any> = {};
    questions.forEach((q, idx) => {
      const sel = selected[idx];
      if (sel === '其他' || sel === 'Other') {
        answers[String(idx)] = customTexts[idx] || sel;
      } else {
        answers[String(idx)] = sel ?? '';
      }
    });
    setSubmitted(true);
    onAnswer(tool.id, answers);
  };

  const allAnswered = questions.every((q, idx) => {
    const s = selected[idx];
    if (q.multiSelect) return Array.isArray(s) && s.length > 0;
    return !!s;
  });

  return (
    <div className="chat-ask-card">
      <div className="chat-ask-card__header">
        <span className="chat-ask-card__count">{questions.length} question{questions.length > 1 ? 's' : ''}</span>
        <button
          className="chat-ask-card__submit"
          disabled={!allAnswered || submitted}
          onClick={handleSubmit}
        >
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M2 8L6 12L14 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>
          {submitted ? 'Submitted' : 'Submit'}
        </button>
        {!submitted && (
          <span className="chat-ask-card__waiting">Waiting</span>
        )}
      </div>
      {questions.map((q, qIdx) => {
        const isOtherSelected = selected[qIdx] === '其他' || selected[qIdx] === 'Other';
        return (
          <div key={qIdx} className="chat-ask-card__question">
            <div className="chat-ask-card__question-header">
              <span className="chat-ask-card__tag">{q.header}</span>
              <span className="chat-ask-card__question-text">{q.question}</span>
            </div>
            <div className="chat-ask-card__options">
              {(q.options || []).map((opt: any, oIdx: number) => {
                const isSelected = q.multiSelect
                  ? (selected[qIdx] as string[] || []).includes(opt.label)
                  : selected[qIdx] === opt.label;
                return (
                  <button
                    key={oIdx}
                    className={`chat-ask-card__option ${isSelected ? 'is-selected' : ''}`}
                    onClick={() => handleSelect(qIdx, opt.label, q.multiSelect)}
                    disabled={submitted}
                  >
                    <span className={`chat-ask-card__radio ${q.multiSelect ? 'chat-ask-card__radio--multi' : ''}`}>
                      {isSelected && (
                        <svg width="8" height="8" viewBox="0 0 16 16" fill="none">
                          <path d="M3 8L6.5 11.5L13 4.5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"/>
                        </svg>
                      )}
                    </span>
                    <span className="chat-ask-card__option-label">{opt.label}</span>
                    {opt.description && (
                      <span className="chat-ask-card__option-desc">{opt.description}</span>
                    )}
                  </button>
                );
              })}
              {/* "Other" option */}
              <button
                className={`chat-ask-card__option ${isOtherSelected ? 'is-selected' : ''}`}
                onClick={() => handleSelect(qIdx, '其他', q.multiSelect)}
                disabled={submitted}
              >
                <span className={`chat-ask-card__radio ${q.multiSelect ? 'chat-ask-card__radio--multi' : ''}`}>
                  {isOtherSelected && (
                    <svg width="8" height="8" viewBox="0 0 16 16" fill="none">
                      <path d="M3 8L6.5 11.5L13 4.5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"/>
                    </svg>
                  )}
                </span>
                <span className="chat-ask-card__option-label">Other</span>
                <span className="chat-ask-card__option-desc">Custom text input</span>
              </button>
              {isOtherSelected && (
                <input
                  className="chat-ask-card__custom-input"
                  placeholder="Type your answer..."
                  value={customTexts[qIdx] || ''}
                  onChange={(e) => setCustomTexts(prev => ({ ...prev, [qIdx]: e.target.value }))}
                  disabled={submitted}
                />
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};

// ─── Ordered Items renderer ─────────────────────────────────────────────────

function renderOrderedItems(items: ChatMessageItem[], now: number) {
  const groups: { type: string; entries: ChatMessageItem[] }[] = [];
  for (const item of items) {
    const last = groups[groups.length - 1];
    if (last && last.type === item.type) {
      last.entries.push(item);
    } else {
      groups.push({ type: item.type, entries: [item] });
    }
  }

  return groups.map((g, gi) => {
    if (g.type === 'thinking') {
      const text = g.entries.map(e => e.content || '').join('\n\n');
      return <ThinkingBlock key={`g${gi}`} thinking={text} />;
    }
    if (g.type === 'tool') {
      const tools = g.entries.map(e => e.tool!).filter(Boolean);
      return <ToolList key={`g${gi}`} tools={tools} now={now} />;
    }
    if (g.type === 'text') {
      return g.entries.map((entry, ii) =>
        entry.content ? (
          <div key={`g${gi}_${ii}`} className="chat-msg__assistant-content">
            <MarkdownContent content={entry.content} />
          </div>
        ) : null,
      );
    }
    return null;
  });
}

// ─── Active turn items renderer (with AskUserQuestion support) ─────────────

function renderActiveTurnItems(
  items: ChatMessageItem[],
  now: number,
  sessionMgr: RemoteSessionManager,
  setError: (e: string) => void,
) {
  const groups: { type: string; entries: ChatMessageItem[] }[] = [];
  for (const item of items) {
    const last = groups[groups.length - 1];
    if (last && last.type === item.type) {
      last.entries.push(item);
    } else {
      groups.push({ type: item.type, entries: [item] });
    }
  }

  return groups.map((g, gi) => {
    if (g.type === 'thinking') {
      const text = g.entries.map(e => e.content || '').join('\n\n');
      return <ThinkingBlock key={`ag${gi}`} thinking={text} />;
    }
    if (g.type === 'tool') {
      const askEntries = g.entries.filter(
        e => e.tool?.name === 'AskUserQuestion' && e.tool?.status === 'running' && e.tool?.tool_input,
      );
      const regularEntries = g.entries.filter(e => !askEntries.includes(e));
      const regularTools = regularEntries.map(e => e.tool!).filter(Boolean);

      return (
        <React.Fragment key={`ag${gi}`}>
          {regularTools.length > 0 && <ToolList tools={regularTools} now={now} />}
          {askEntries.map(e => (
            <AskQuestionCard
              key={e.tool!.id}
              tool={e.tool!}
              onAnswer={(toolId, answers) => {
                sessionMgr.answerQuestion(toolId, answers).catch(err => { setError(String(err)); });
              }}
            />
          ))}
        </React.Fragment>
      );
    }
    if (g.type === 'text') {
      return g.entries.map((entry, ii) =>
        entry.content ? (
          <div key={`ag${gi}_${ii}`} className="chat-msg__assistant-content">
            <MarkdownContent content={entry.content} />
          </div>
        ) : null,
      );
    }
    return null;
  });
}

// ─── Theme toggle icon ─────────────────────────────────────────────────────

const ThemeToggleIcon: React.FC<{ isDark: boolean }> = ({ isDark }) => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
    {isDark ? (
      <path d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM3 8a5 5 0 0 1 5-5v10a5 5 0 0 1-5-5Z" fill="currentColor"/>
    ) : (
      <path d="M8 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 1Zm0 11a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1A.5.5 0 0 1 8 12Zm7-4a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 15 8ZM3 8a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1A.5.5 0 0 1 3 8Zm9.95-3.54a.5.5 0 0 1 0 .71l-.71.7a.5.5 0 1 1-.7-.7l.7-.71a.5.5 0 0 1 .71 0ZM5.46 11.24a.5.5 0 0 1 0 .71l-.7.71a.5.5 0 0 1-.71-.71l.7-.71a.5.5 0 0 1 .71 0Zm7.08 1.42a.5.5 0 0 1-.7 0l-.71-.71a.5.5 0 0 1 .7-.7l.71.7a.5.5 0 0 1 0 .71ZM5.46 4.76a.5.5 0 0 1-.71 0l-.71-.7a.5.5 0 0 1 .71-.71l.7.7a.5.5 0 0 1 0 .71ZM8 5a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z" fill="currentColor"/>
    )}
  </svg>
);

// ─── Agent Mode ─────────────────────────────────────────────────────────────

type AgentMode = 'agentic' | 'Plan' | 'debug';

const MODE_OPTIONS: { id: AgentMode; label: string }[] = [
  { id: 'agentic', label: 'Agentic' },
  { id: 'Plan', label: 'Plan' },
  { id: 'debug', label: 'Debug' },
];

// ─── ChatPage ───────────────────────────────────────────────────────────────

const ChatPage: React.FC<ChatPageProps> = ({ sessionMgr, sessionId, sessionName, onBack }) => {
  const {
    getMessages,
    setMessages,
    appendNewMessages,
    activeTurn,
    setActiveTurn,
    setError,
    currentWorkspace,
    updateSessionName,
  } = useMobileStore();

  const { isDark, toggleTheme } = useTheme();
  const messages = getMessages(sessionId);
  const [input, setInput] = useState('');
  const [agentMode, setAgentMode] = useState<AgentMode>('agentic');
  const [liveTitle, setLiveTitle] = useState(sessionName);
  const [pendingImages, setPendingImages] = useState<{ name: string; dataUrl: string }[]>([]);
  const [inputFocused, setInputFocused] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const pollerRef = useRef<SessionPoller | null>(null);

  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);

  const isStreaming = activeTurn != null && activeTurn.status === 'active';

  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!isStreaming) return;
    const timer = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(timer);
  }, [isStreaming]);

  const loadMessages = useCallback(async (beforeId?: string) => {
    if (isLoadingMore || (!hasMore && beforeId)) return;
    try {
      setIsLoadingMore(true);
      const resp = await sessionMgr.getSessionMessages(sessionId, 50, beforeId);
      if (beforeId) {
        const currentMsgs = getMessages(sessionId);
        setMessages(sessionId, [...resp.messages, ...currentMsgs]);
      } else {
        setMessages(sessionId, resp.messages);
      }
      setHasMore(resp.has_more);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setIsLoadingMore(false);
    }
  }, [sessionMgr, sessionId, setMessages, setError, getMessages, isLoadingMore, hasMore]);

  const handleScroll = useCallback(() => {
    const container = messagesContainerRef.current;
    if (!container) return;
    if (container.scrollTop < 100 && hasMore && !isLoadingMore) {
      const msgs = getMessages(sessionId);
      if (msgs.length > 0) loadMessages(msgs[0].id);
    }
  }, [hasMore, isLoadingMore, getMessages, sessionId, loadMessages]);

  // Initial load + start poller
  useEffect(() => {
    loadMessages().then(() => {
      const initialMsgCount = useMobileStore.getState().getMessages(sessionId).length;

      const poller = new SessionPoller(sessionMgr, sessionId, (resp: PollResponse) => {
        if (resp.new_messages && resp.new_messages.length > 0) {
          appendNewMessages(sessionId, resp.new_messages);
        }
        if (resp.title) {
          setLiveTitle(resp.title);
          updateSessionName(sessionId, resp.title);
        }
        setActiveTurn(resp.active_turn ?? null);
      });

      poller.start(initialMsgCount);
      pollerRef.current = poller;
    });

    return () => {
      pollerRef.current?.stop();
      pollerRef.current = null;
      setActiveTurn(null);
    };
  }, [sessionId, sessionMgr]);

  useEffect(() => {
    if (!isLoadingMore) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, activeTurn, isLoadingMore]);

  // Reload messages when a turn completes so the messages array
  // contains the final persisted content instead of stale partial data.
  const prevActiveTurnRef = useRef<ActiveTurnSnapshot | null>(null);
  useEffect(() => {
    const prev = prevActiveTurnRef.current;
    prevActiveTurnRef.current = activeTurn;
    if (prev && !activeTurn) {
      loadMessages();
    }
  }, [activeTurn, loadMessages]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    const imgs = pendingImages;
    if ((!text && imgs.length === 0) || isStreaming) return;
    setInput('');
    setPendingImages([]);

    try {
      const imagePayload = imgs.length > 0
        ? imgs.map(i => ({ name: i.name, data_url: i.dataUrl }))
        : undefined;
      await sessionMgr.sendMessage(sessionId, text || '(see attached images)', agentMode, imagePayload);
    } catch (e: any) {
      setError(e.message);
    }
  }, [input, pendingImages, isStreaming, sessionId, sessionMgr, setError, agentMode]);

  const handleImageSelect = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    const maxImages = 5;
    const remaining = maxImages - pendingImages.length;
    const toProcess = Array.from(files).slice(0, remaining);

    toProcess.forEach((file) => {
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result as string;
        setPendingImages((prev) => {
          if (prev.length >= maxImages) return prev;
          return [...prev, { name: file.name, dataUrl }];
        });
      };
      reader.readAsDataURL(file);
    });
    e.target.value = '';
  }, [pendingImages.length]);

  const removeImage = useCallback((idx: number) => {
    setPendingImages((prev) => prev.filter((_, i) => i !== idx));
  }, []);

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

  const workspaceName = currentWorkspace?.project_name || currentWorkspace?.path?.split('/').pop() || '';
  const gitBranch = currentWorkspace?.git_branch;
  const displayName = liveTitle || sessionName || 'Session';

  return (
    <div className="chat-page page-transition">
      {/* Header */}
      <div className="chat-page__header">
        <div className="chat-page__header-row">
          <button className="chat-page__back" onClick={onBack} aria-label="Back">
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
              <path d="M12 4L6 10L12 16" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </button>
          <div className="chat-page__header-center">
            <span className="chat-page__title" title={displayName}>{displayName}</span>
          </div>
          <div className="chat-page__header-right">
            <button className="chat-page__theme-btn" onClick={toggleTheme} aria-label="Toggle theme">
              <ThemeToggleIcon isDark={isDark} />
            </button>
          </div>
        </div>
        {workspaceName && (
          <div className="chat-page__header-workspace" title={currentWorkspace?.path}>
            <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
              <path d="M2 4L8 2L14 4V12L8 14L2 12V4Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/>
              <path d="M8 2V14" stroke="currentColor" strokeWidth="1.2"/>
              <path d="M2 4L8 6L14 4" stroke="currentColor" strokeWidth="1.2"/>
            </svg>
            <span className="chat-page__workspace-name">{workspaceName}</span>
            {gitBranch && (
              <span className="chat-page__workspace-branch">
                <svg width="10" height="10" viewBox="0 0 16 16" fill="none"><circle cx="5" cy="4" r="2" stroke="currentColor" strokeWidth="1.3"/><circle cx="11" cy="4" r="2" stroke="currentColor" strokeWidth="1.3"/><circle cx="5" cy="12" r="2" stroke="currentColor" strokeWidth="1.3"/><path d="M5 6V10M11 6V8C11 9.1046 10.1046 10 9 10H5" stroke="currentColor" strokeWidth="1.3"/></svg>
                {gitBranch}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Messages */}
      <div className="chat-page__messages" ref={messagesContainerRef} onScroll={handleScroll}>
        {isLoadingMore && (
          <div className="chat-page__load-more-indicator">Loading older messages…</div>
        )}

        {messages.map((m, _idx) => {
          if (m.role === 'system' || m.role === 'tool') return null;

          if (m.role === 'user') {
            return (
              <div key={m.id} className="chat-msg chat-msg--user">
                <div className="chat-msg__user-card">
                  <div className="chat-msg__user-avatar">U</div>
                  <div className="chat-msg__user-content">{m.content}</div>
                </div>
              </div>
            );
          }

          return (
            <div key={m.id} className="chat-msg chat-msg--assistant">
              {m.items && m.items.length > 0 ? (
                renderOrderedItems(m.items, now)
              ) : (
                <>
                  {m.thinking && <ThinkingBlock thinking={m.thinking} />}
                  {m.tools && m.tools.length > 0 && <ToolList tools={m.tools} now={now} />}
                  {m.content && (
                    <div className="chat-msg__assistant-content">
                      <MarkdownContent content={m.content} />
                    </div>
                  )}
                </>
              )}
            </div>
          );
        })}

        {/* Active turn overlay (streaming content from poller) */}
        {activeTurn && (() => {
          if (activeTurn.items && activeTurn.items.length > 0) {
            return (
              <div className="chat-msg chat-msg--assistant">
                {renderActiveTurnItems(activeTurn.items, now, sessionMgr, setError)}
                {activeTurn.status === 'active' && !activeTurn.thinking && !activeTurn.text && activeTurn.tools.length === 0 && (
                  <div className="chat-msg__assistant-content"><TypingDots /></div>
                )}
              </div>
            );
          }

          const askTools = activeTurn.tools.filter(
            t => t.name === 'AskUserQuestion' && t.status === 'running' && t.tool_input,
          );
          const askToolIds = new Set(askTools.map(t => t.id));
          const regularTools = activeTurn.tools.filter(t => !askToolIds.has(t.id));

          return (
            <div className="chat-msg chat-msg--assistant">
              {(activeTurn.thinking || activeTurn.status === 'active') && (
                <ThinkingBlock
                  thinking={activeTurn.thinking}
                  streaming={activeTurn.status === 'active' && !activeTurn.thinking && !activeTurn.text}
                />
              )}
              <ToolList tools={regularTools} now={now} />
              {askTools.map(at => (
                <AskQuestionCard
                  key={at.id}
                  tool={at}
                  onAnswer={(toolId, answers) => {
                    sessionMgr.answerQuestion(toolId, answers).catch(err => { setError(String(err)); });
                  }}
                />
              ))}
              {activeTurn.text ? (
                <div className="chat-msg__assistant-content">
                  <MarkdownContent content={activeTurn.text} />
                </div>
              ) : activeTurn.status === 'active' && !activeTurn.thinking && activeTurn.tools.length === 0 ? (
                <div className="chat-msg__assistant-content"><TypingDots /></div>
              ) : null}
            </div>
          );
        })()}

        <div ref={messagesEndRef} />
      </div>

      {/* Floating Input Bar */}
      <div className={`chat-page__input-bar ${inputFocused ? 'is-focused' : ''}`}>
        <div className="chat-page__input-toolbar">
          <div className="chat-page__mode-selector">
            {MODE_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                className={`chat-page__mode-btn${agentMode === opt.id ? ' is-active' : ''}`}
                onClick={() => setAgentMode(opt.id)}
                disabled={isStreaming}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        {pendingImages.length > 0 && (
          <div className="chat-page__image-preview-row">
            {pendingImages.map((img, idx) => (
              <div key={idx} className="chat-page__image-thumb">
                <img src={img.dataUrl} alt={img.name} />
                <button className="chat-page__image-remove" onClick={() => removeImage(idx)}>×</button>
              </div>
            ))}
          </div>
        )}

        <div className="chat-page__input-row">
          <button
            className="chat-page__attach-btn"
            onClick={handleImageSelect}
            disabled={isStreaming || pendingImages.length >= 5}
            aria-label="Attach image"
          >
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
              <rect x="2" y="3" width="16" height="14" rx="2" stroke="currentColor" strokeWidth="1.5"/>
              <circle cx="7" cy="8" r="1.5" stroke="currentColor" strokeWidth="1.2"/>
              <path d="M2 14L6 10L9 13L13 9L18 14" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round"/>
            </svg>
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept="image/png,image/jpeg,image/jpg,image/gif,image/webp"
            multiple
            style={{ display: 'none' }}
            onChange={handleFileChange}
          />
          <textarea
            ref={inputRef}
            className="chat-page__input"
            placeholder="Type a message..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setInputFocused(true)}
            onBlur={() => setInputFocused(false)}
            rows={1}
            disabled={isStreaming}
          />
          {isStreaming ? (
            <button
              className="chat-page__send is-stop"
              onClick={handleCancel}
              aria-label="Stop"
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <rect x="2" y="2" width="12" height="12" rx="2" fill="currentColor"/>
              </svg>
            </button>
          ) : (
            <button
              className="chat-page__send"
              onClick={handleSend}
              disabled={!input.trim() && pendingImages.length === 0}
            >
              <svg width="16" height="16" viewBox="0 0 20 20" fill="none">
                <path d="M3 10L17 3L10 17V10H3Z" fill="currentColor"/>
              </svg>
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default ChatPage;
