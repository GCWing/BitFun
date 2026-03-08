import React, { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus, vs } from 'react-syntax-highlighter/dist/esm/styles/prism';
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
  autoFocus?: boolean;
}

// ─── Markdown ───────────────────────────────────────────────────────────────

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function truncateMiddle(str: string, maxLen: number): string {
  if (!str || str.length <= maxLen) return str;
  const keep = maxLen - 3;
  const head = Math.ceil(keep * 0.6);
  const tail = keep - head;
  return str.slice(0, head) + '...' + str.slice(-tail);
}

function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    return navigator.clipboard.writeText(text);
  }
  // Fallback for insecure contexts (HTTP)
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.style.cssText = 'position:fixed;left:-9999px;top:-9999px;opacity:0';
  document.body.appendChild(ta);
  ta.select();
  try {
    document.execCommand('copy');
  } finally {
    document.body.removeChild(ta);
  }
  return Promise.resolve();
}

const CopyButton: React.FC<{ code: string }> = ({ code }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await copyToClipboard(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* ignore */ }
  };

  return (
    <button
      className={`copy-button${copied ? ' copy-success' : ''}`}
      onClick={handleCopy}
      type="button"
    >
      {copied ? (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
    </button>
  );
};

const MarkdownContent: React.FC<{ content: string }> = ({ content }) => {
  const { isDark } = useTheme();
  const syntaxTheme = isDark ? vscDarkPlus : vs;

  const components: React.ComponentProps<typeof ReactMarkdown>['components'] = useMemo(() => ({
    code({ className, children, ...props }: any) {
      const match = /language-(\w+)/.exec(className || '');
      const codeStr = String(children).replace(/\n$/, '');
      const hasMultipleLines = codeStr.includes('\n');
      const isCodeBlock = className?.startsWith('language-') || hasMultipleLines;

      if (!isCodeBlock) {
        return (
          <code className="inline-code" {...props}>
            {children}
          </code>
        );
      }

      return (
        <div className="code-block-wrapper">
          <CopyButton code={codeStr} />
          <SyntaxHighlighter
            language={match?.[1] || 'text'}
            style={syntaxTheme}
            showLineNumbers={true}
            customStyle={{
              margin: 0,
              borderRadius: '8px',
              fontSize: '0.8rem',
              lineHeight: '1.5',
            }}
            codeTagProps={{
              style: {
                fontFamily: 'var(--font-family-mono)',
              },
            }}
            lineNumberStyle={{
              color: isDark ? '#666' : '#999',
              paddingRight: '1em',
              textAlign: 'right' as const,
              userSelect: 'none' as const,
              minWidth: '2.5em',
            }}
          >
            {codeStr}
          </SyntaxHighlighter>
        </div>
      );
    },

    table({ children }: any) {
      return (
        <div className="table-wrapper">
          <table>{children}</table>
        </div>
      );
    },

    blockquote({ children }: any) {
      return <blockquote className="custom-blockquote">{children}</blockquote>;
    },
  }), [syntaxTheme, isDark]);

  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
      {content}
    </ReactMarkdown>
  );
};

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
    : `Thought ${charCount} characters`;

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
  Task: 'Task',
  search: 'Search',
  edit_file: 'Edit',
  web_search: 'Web',
  TodoWrite: 'Todo',
};

// ─── TodoWrite card ─────────────────────────────────────────────────────────

const TodoCard: React.FC<{ tool: RemoteToolStatus }> = ({ tool }) => {
  const [expanded, setExpanded] = useState(false);

  const todos: { id?: string; content: string; status: string }[] = useMemo(() => {
    const src = tool.tool_input;
    if (!src) return [];
    const arr = src.todos || src.result?.todos;
    return Array.isArray(arr) ? arr : [];
  }, [tool.tool_input]);

  if (todos.length === 0) return null;

  const completed = todos.filter(t => t.status === 'completed').length;
  const allDone = completed === todos.length;
  const inProgress = todos.find(t => t.status === 'in_progress');

  const statusIcon = (s: string) => {
    switch (s) {
      case 'completed':
        return <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--color-success)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><path d="m9 11 3 3L22 4"/></svg>;
      case 'in_progress':
        return <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent-500)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/><polygon points="10 8 16 12 10 16 10 8" fill="var(--color-accent-500)"/></svg>;
      case 'cancelled':
        return <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--color-error)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/></svg>;
      default:
        return <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--color-text-muted)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/></svg>;
    }
  };

  return (
    <div className="chat-todo-card">
      <div className="chat-todo-card__header" onClick={() => setExpanded(!expanded)}>
        <span className="chat-todo-card__icon">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="5" width="6" height="6" rx="1"/><path d="m3 17 2 2 4-4"/><path d="M13 6h8"/><path d="M13 12h8"/><path d="M13 18h8"/>
          </svg>
        </span>
        {allDone && !expanded ? (
          <span className="chat-todo-card__current chat-todo-card__current--done">All tasks completed</span>
        ) : inProgress && !expanded ? (
          <span className="chat-todo-card__current">{inProgress.content}</span>
        ) : null}
        <span className="chat-todo-card__right">
          <span className="chat-todo-card__dots">
            {todos.map((t, i) => (
              <span key={t.id || i} className={`chat-todo-card__dot chat-todo-card__dot--${t.status}`} />
            ))}
          </span>
          <span className="chat-todo-card__stats">{completed}/{todos.length}</span>
        </span>
        <span className={`chat-todo-card__chevron ${expanded ? 'is-expanded' : ''}`}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6"/></svg>
        </span>
      </div>
      {expanded && (
        <div className="chat-todo-card__list">
          {todos.map((t, i) => (
            <div key={t.id || i} className={`chat-todo-card__item chat-todo-card__item--${t.status}`}>
              {statusIcon(t.status)}
              <span className="chat-todo-card__item-text">{t.content}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

/**
 * Extract task description and agent type from execute_subagent tool data.
 * Prefers tool_input (full JSON from backend), falls back to input_preview (truncated).
 */
function parseTaskInfo(tool: RemoteToolStatus): { description?: string; agentType?: string } | null {
  const source = tool.tool_input ?? (() => {
    try { return JSON.parse(tool.input_preview || ''); } catch { return null; }
  })();
  if (!source) return null;
  return {
    description: source.description,
    agentType: source.subagent_type,
  };
}

/**
 * Summarize a subItem for display inside a Task card.
 */
function subItemLabel(item: ChatMessageItem): string {
  if (item.type === 'thinking') {
    const len = (item.content || '').length;
    return `Thought ${len} characters`;
  }
  if (item.type === 'tool' && item.tool) {
    const t = item.tool;
    const preview = t.input_preview ? `: ${t.input_preview}` : '';
    return `${t.name}${preview}`;
  }
  if (item.type === 'text') {
    const len = (item.content || '').length;
    return `Text ${len} characters`;
  }
  return '';
}

const TaskToolCard: React.FC<{
  tool: RemoteToolStatus;
  now: number;
  subItems?: ChatMessageItem[];
  onCancelTool?: (toolId: string) => void;
}> = ({ tool, now, subItems = [], onCancelTool }) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);
  const [stepsExpanded, setStepsExpanded] = useState(false);
  const isRunning = tool.status === 'running';
  const isCompleted = tool.status === 'completed';
  const isError = tool.status === 'failed' || tool.status === 'error';
  const showCancel = isRunning && !!onCancelTool;
  const taskInfo = parseTaskInfo(tool);

  const durationLabel = isCompleted && tool.duration_ms != null
    ? formatDuration(tool.duration_ms)
    : isRunning && tool.start_ms
    ? formatDuration(now - tool.start_ms)
    : '';

  const statusClass = isRunning ? 'running' : isCompleted ? 'done' : isError ? 'error' : 'pending';

  const subTools = subItems.filter(i => i.type === 'tool' && i.tool);
  const subToolsDone = subTools.filter(i => i.tool!.status === 'completed').length;
  const subToolsRunning = subTools.filter(i => i.tool!.status === 'running').length;

  useEffect(() => {
    if (stepsExpanded && subItems.length > prevCountRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
    prevCountRef.current = subItems.length;
  }, [subItems.length, stepsExpanded]);

  return (
    <div className={`chat-task-card chat-task-card--${statusClass}`}>
      <div className="chat-task-card__header">
        <span className="chat-tool-card__icon">
          {isRunning ? (
            <span className="chat-tool-card__spinner" />
          ) : isCompleted ? (
            <span className="chat-tool-card__check">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M3 8.5L6.5 12L13 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>
            </span>
          ) : isError ? (
            <span className="chat-tool-card__error-icon">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/></svg>
            </span>
          ) : (
            <span className="chat-tool-card__spinner" />
          )}
        </span>
        <span className="chat-tool-card__name">
          {taskInfo?.description || 'Task'}
        </span>
        {taskInfo?.agentType && (
          <span className="chat-tool-card__type">{taskInfo.agentType}</span>
        )}
        {durationLabel && (
          <span className="chat-tool-card__duration">{durationLabel}</span>
        )}
        {showCancel && (
          <button
            className="chat-tool-card__cancel"
            onClick={(e) => { e.stopPropagation(); onCancelTool?.(tool.id); }}
            aria-label="Cancel"
          >
            <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
              <rect x="3" y="3" width="10" height="10" rx="2" fill="currentColor"/>
            </svg>
          </button>
        )}
      </div>

      {subItems.length > 0 && (
        <>
          <div className="chat-task-card__summary" onClick={() => setStepsExpanded(e => !e)}>
            <span className="chat-task-card__stat">
              {subTools.length} tool call{subTools.length === 1 ? '' : 's'}
            </span>
            <span className="chat-task-card__stat-right">
              <span className="chat-task-card__stat--done">{subToolsDone} done</span>
              {subToolsRunning > 0 && <span className="chat-task-card__stat--running">{subToolsRunning} running</span>}
            </span>
            <span className={`chat-task-card__chevron ${stepsExpanded ? 'is-expanded' : ''}`}>
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6"/></svg>
            </span>
          </div>
          {stepsExpanded && (
            <div className="chat-task-card__steps" ref={scrollRef}>
              {subItems.map((item, idx) => {
                if (item.type === 'thinking') {
                  return (
                    <div key={`sub-think-${idx}`} className="chat-task-card__step chat-task-card__step--thinking">
                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                      <span>{subItemLabel(item)}</span>
                    </div>
                  );
                }
                if (item.type === 'tool' && item.tool) {
                  const t = item.tool;
                  const isDone = t.status === 'completed';
                  const isErr = t.status === 'failed' || t.status === 'error';
                  return (
                    <div key={`sub-tool-${t.id}-${idx}`} className={`chat-task-card__step chat-task-card__step--tool ${isDone ? 'is-done' : isErr ? 'is-error' : 'is-running'}`}>
                      {isDone ? (
                        <svg width="10" height="10" viewBox="0 0 16 16" fill="none"><path d="M3 8.5L6.5 12L13 4" stroke="var(--color-success)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>
                      ) : isErr ? (
                        <svg width="10" height="10" viewBox="0 0 16 16" fill="none"><path d="M4 4L12 12M12 4L4 12" stroke="var(--color-error)" strokeWidth="2" strokeLinecap="round"/></svg>
                      ) : (
                        <span className="chat-task-card__step-spinner" />
                      )}
                      <span className="chat-task-card__step-name">{t.name}</span>
                    {(() => {
                      const p = getToolPreview(t);
                      return p ? <span className="chat-task-card__step-preview">{p}</span> : null;
                    })()}
                      {isDone && t.duration_ms != null && (
                        <span className="chat-task-card__step-duration">{formatDuration(t.duration_ms)}</span>
                      )}
                    </div>
                  );
                }
                return null;
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
};

/**
 * Parse tool input_preview (slim JSON from backend) and extract a concise display text.
 * Backend sends valid JSON with large fields stripped; frontend extracts the key field
 * and truncates the resulting plain text.
 */
function getToolPreview(tool: RemoteToolStatus): string | null {
  if (!tool.input_preview) return null;
  try {
    const params = JSON.parse(tool.input_preview);
    if (!params || typeof params !== 'object') return null;

    const lastSegment = (p: string) => {
      const parts = p.replace(/\\/g, '/').split('/');
      return parts[parts.length - 1] || p;
    };

    let result: string | null = null;

    const pathVal = params.file_path || params.path;
    switch (tool.name) {
      case 'Read':
      case 'Write':
      case 'Edit':
      case 'LS':
      case 'StrReplace':
      case 'delete_file':
        result = pathVal ? lastSegment(pathVal) : null;
        break;
      case 'Glob':
      case 'Grep':
        result = params.pattern || null;
        break;
      case 'Bash':
      case 'Shell':
        result = params.description || params.command || null;
        break;
      case 'web_search':
      case 'WebSearch':
        result = params.search_term || params.query || null;
        break;
      case 'WebFetch':
        result = params.url || null;
        break;
      case 'SemanticSearch':
        result = params.query || null;
        break;
      default: {
        const first = Object.values(params).find(
          (v): v is string => typeof v === 'string' && v.length > 0 && v.length < 80,
        );
        result = first || null;
      }
    }

    if (!result) return null;
    return result.length > 60 ? result.slice(0, 60) + '…' : result;
  } catch {
    return null;
  }
}

const ToolCard: React.FC<{
  tool: RemoteToolStatus;
  now: number;
  onCancelTool?: (toolId: string) => void;
}> = ({ tool, now, onCancelTool }) => {
  const toolKey = tool.name.toLowerCase().replace(/[\s-]/g, '_');
  const typeLabel = TOOL_TYPE_MAP[toolKey] || TOOL_TYPE_MAP[tool.name] || 'Tool';
  const isRunning = tool.status === 'running';
  const isCompleted = tool.status === 'completed';
  const isError = tool.status === 'failed' || tool.status === 'error';
  const showCancel = isRunning && !!onCancelTool;
  const preview = getToolPreview(tool);

  const durationLabel = isCompleted && tool.duration_ms != null
    ? formatDuration(tool.duration_ms)
    : isRunning && tool.start_ms
    ? formatDuration(now - tool.start_ms)
    : '';

  const statusClass = isRunning ? 'running' : isCompleted ? 'done' : isError ? 'error' : 'pending';

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
          ) : isError ? (
            <span className="chat-tool-card__error-icon">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/></svg>
            </span>
          ) : (
            <span className="chat-tool-card__spinner" />
          )}
        </span>
        <span className="chat-tool-card__name">
          {tool.name}
          {preview && <span className="chat-tool-card__preview"> {preview}</span>}
        </span>
        <span className="chat-tool-card__type">{typeLabel}</span>
        {durationLabel && (
          <span className="chat-tool-card__duration">{durationLabel}</span>
        )}
        {showCancel && (
          <button
            className="chat-tool-card__cancel"
            onClick={(e) => { e.stopPropagation(); onCancelTool?.(tool.id); }}
            aria-label="Cancel"
          >
            <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
              <rect x="3" y="3" width="10" height="10" rx="2" fill="currentColor"/>
            </svg>
          </button>
        )}
      </div>
    </div>
  );
};

const READ_LIKE_TOOLS = new Set(['Read', 'Grep', 'Glob', 'SemanticSearch']);

const ReadFilesToggle: React.FC<{ tools: RemoteToolStatus[] }> = ({ tools }) => {
  const [open, setOpen] = useState(false);
  if (tools.length === 0) return null;

  const doneCount = tools.filter(t => t.status === 'completed').length;
  const allDone = doneCount === tools.length;
  const label = allDone
    ? `Read ${tools.length} file${tools.length === 1 ? '' : 's'}`
    : `Reading ${tools.length} file${tools.length === 1 ? '' : 's'} (${doneCount} done)`;

  return (
    <div className={`chat-thinking ${allDone ? '' : 'chat-thinking--streaming'}`}>
      <button className="chat-thinking__toggle" onClick={() => setOpen(o => !o)}>
        <span className={`chat-thinking__chevron ${open ? 'is-open' : ''}`}>
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
            <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </span>
        <span className="chat-thinking__label">{label}</span>
      </button>
      {open && (
        <div className="chat-thinking__content-wrapper at-top at-bottom">
          <div className="chat-thinking__content">
            {tools.map(t => {
              const preview = t.input_preview || '';
              return (
                <div key={t.id} style={{ fontSize: '12px', padding: '2px 0', opacity: 0.8 }}>
                  {t.status === 'completed' ? '✓' : '⋯'} {t.name} {preview}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};

const TOOL_LIST_COLLAPSE_THRESHOLD = 2;

const ToolList: React.FC<{
  tools: RemoteToolStatus[];
  now: number;
  onCancelTool?: (toolId: string) => void;
}> = ({ tools, now, onCancelTool }) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    if (expanded && tools.length > prevCountRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
    prevCountRef.current = tools.length;
  }, [tools.length, expanded]);

  if (!tools || tools.length === 0) return null;

  if (tools.length <= TOOL_LIST_COLLAPSE_THRESHOLD) {
    return (
      <div className="chat-tool-list">
        {tools.map((tc) => (
          <ToolCard key={tc.id} tool={tc} now={now} onCancelTool={onCancelTool} />
        ))}
      </div>
    );
  }

  const runningCount = tools.filter(t => t.status === 'running').length;
  const doneCount = tools.filter(t => t.status === 'completed').length;

  return (
    <div className="chat-tool-list chat-tool-list--collapsed">
      <div className="chat-tool-list__header" onClick={() => setExpanded(e => !e)}>
        <span className="chat-tool-list__count">{tools.length} tool call{tools.length === 1 ? '' : 's'}</span>
        <span className="chat-tool-list__stats">
          {doneCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--done">{doneCount} done</span>}
          {runningCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--running">{runningCount} running</span>}
        </span>
        <span className={`chat-tool-list__chevron ${expanded ? 'is-expanded' : ''}`}>
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6"/></svg>
        </span>
      </div>
      {expanded && (
        <div className="chat-tool-list__scroll" ref={scrollRef}>
          {tools.map((tc) => (
            <ToolCard key={tc.id} tool={tc} now={now} onCancelTool={onCancelTool} />
          ))}
        </div>
      )}
    </div>
  );
};

// ─── Typing indicator ───────────────────────────────────────────────────────

const TypingDots: React.FC = () => (
  <span className="chat-msg__typing">
    <span /><span /><span />
  </span>
);

// ─── Typewriter effect (pseudo-streaming) ──────────────────────────────────

function useTypewriter(targetText: string, animate: boolean): string {
  const [displayText, setDisplayText] = useState(animate ? '' : targetText);
  const revealedRef = useRef(animate ? 0 : targetText.length);
  const targetRef = useRef(targetText);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const speedRef = useRef(3);

  useEffect(() => {
    if (!animate) {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      revealedRef.current = targetText.length;
      targetRef.current = targetText;
      setDisplayText(targetText);
      return;
    }

    targetRef.current = targetText;

    if (targetText.length < revealedRef.current) {
      revealedRef.current = 0;
    }

    const delta = targetText.length - revealedRef.current;
    if (delta > 0) {
      const FRAME_INTERVAL = 30;
      const REVEAL_DURATION = 800;
      const totalFrames = REVEAL_DURATION / FRAME_INTERVAL;
      speedRef.current = Math.max(Math.ceil(delta / totalFrames), 2);

      if (!timerRef.current) {
        timerRef.current = setInterval(() => {
          const target = targetRef.current;
          const cur = revealedRef.current;
          if (cur >= target.length) {
            if (timerRef.current) {
              clearInterval(timerRef.current);
              timerRef.current = null;
            }
            return;
          }
          const next = Math.min(cur + speedRef.current, target.length);
          revealedRef.current = next;
          setDisplayText(target.slice(0, next));
        }, FRAME_INTERVAL);
      }
    }
  }, [targetText, animate]);

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  return displayText;
}

const TypewriterText: React.FC<{ content: string }> = ({ content }) => {
  const displayText = useTypewriter(content, true);
  return <MarkdownContent content={displayText} />;
};

// ─── AskUserQuestion Card ─────────────────────────────────────────────────

interface AskQuestionCardProps {
  tool: RemoteToolStatus;
  onAnswer: (toolId: string, answers: any) => Promise<void>;
}

const isPendingAskUserQuestion = (tool?: RemoteToolStatus | null) => {
  if (!tool || tool.name !== 'AskUserQuestion' || !tool.tool_input) return false;
  return !['completed', 'failed', 'cancelled', 'rejected'].includes(tool.status);
};

const isOtherQuestionOption = (label?: string) => {
  const normalized = (label || '').trim().toLowerCase();
  return normalized === 'other' || normalized === '其他';
};

const AskQuestionCard: React.FC<AskQuestionCardProps> = ({ tool, onAnswer }) => {
  const questions: any[] = tool.tool_input?.questions || [];
  const [selected, setSelected] = useState<Record<number, string | string[]>>({});
  const [customTexts, setCustomTexts] = useState<Record<number, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  const normalizedQuestions = useMemo(() => {
    return questions.map((q) => {
      const options = Array.isArray(q.options) ? q.options : [];
      const hasBuiltInOther = options.some((opt: any) => isOtherQuestionOption(opt?.label));
      return { ...q, options, hasBuiltInOther };
    });
  }, [questions]);

  if (normalizedQuestions.length === 0) return null;

  const handleSelect = (qIdx: number, label: string, multi: boolean) => {
    setSelected(prev => {
      if (multi) {
        const arr = (prev[qIdx] as string[] | undefined) || [];
        return { ...prev, [qIdx]: arr.includes(label) ? arr.filter(l => l !== label) : [...arr, label] };
      }
      return { ...prev, [qIdx]: prev[qIdx] === label ? undefined! : label };
    });
  };

  const handleSubmit = async () => {
    if (!allAnswered || submitting || submitted) return;

    const answers: Record<string, any> = {};
    normalizedQuestions.forEach((q, idx) => {
      const sel = selected[idx];
      const customText = (customTexts[idx] || '').trim();
      if (Array.isArray(sel)) {
        answers[String(idx)] = sel.map(value => isOtherQuestionOption(value) ? (customText || value) : value);
      } else if (isOtherQuestionOption(sel)) {
        answers[String(idx)] = customText || sel;
      } else {
        answers[String(idx)] = sel ?? '';
      }
    });

    setSubmitting(true);
    try {
      await onAnswer(tool.id, answers);
      setSubmitted(true);
    } finally {
      setSubmitting(false);
    }
  };

  const allAnswered = normalizedQuestions.every((q, idx) => {
    const s = selected[idx];
    const hasSelection = q.multiSelect ? Array.isArray(s) && s.length > 0 : !!s;
    if (!hasSelection) return false;
    const requiresCustomText = Array.isArray(s)
      ? s.some(value => isOtherQuestionOption(value))
      : isOtherQuestionOption(s);
    return !requiresCustomText || !!(customTexts[idx] || '').trim();
  });

  return (
    <div className="chat-ask-card">
      <div className="chat-ask-card__header">
        <span className="chat-ask-card__count">{questions.length} question{questions.length > 1 ? 's' : ''}</span>
        {!submitted && !submitting && (
          <span className="chat-ask-card__waiting">Waiting</span>
        )}
      </div>
      {normalizedQuestions.map((q, qIdx) => {
        const answer = selected[qIdx];
        const isOtherSelected = Array.isArray(answer)
          ? answer.some(value => isOtherQuestionOption(value))
          : isOtherQuestionOption(answer);
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
                    disabled={submitted || submitting}
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
              {!q.hasBuiltInOther && (
                <button
                  className={`chat-ask-card__option ${isOtherSelected ? 'is-selected' : ''}`}
                  onClick={() => handleSelect(qIdx, 'Other', q.multiSelect)}
                  disabled={submitted || submitting}
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
              )}
              {isOtherSelected && (
                <input
                  className="chat-ask-card__custom-input"
                  placeholder="Type your answer..."
                  value={customTexts[qIdx] || ''}
                  onChange={(e) => setCustomTexts(prev => ({ ...prev, [qIdx]: e.target.value }))}
                  disabled={submitted || submitting}
                />
              )}
            </div>
          </div>
        );
      })}
      <button
        className="chat-ask-card__submit chat-ask-card__submit--bottom"
        disabled={!allAnswered || submitted || submitting}
        onClick={handleSubmit}
      >
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M2 8L6 12L14 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>
        {submitted ? 'Submitted' : submitting ? 'Submitting...' : 'Submit'}
      </button>
    </div>
  );
};

/**
 * Collect subagent internal items into the Task item's subItems field.
 * When a Task tool appears, all subsequent items until the next non-subagent
 * item (or a completed Task) are its internal output. We attach them as
 * subItems on the Task ChatMessageItem for nested rendering.
 */
function filterSubagentItems(items: ChatMessageItem[]): ChatMessageItem[] {
  const result: ChatMessageItem[] = [];
  let currentTaskItem: ChatMessageItem | null = null;

  for (const item of items) {
    if (item.type === 'tool' && item.tool?.name === 'Task') {
      const taskCopy: ChatMessageItem = { ...item, subItems: [] };
      result.push(taskCopy);
      currentTaskItem = taskCopy;
      continue;
    }

    if (item.is_subagent && currentTaskItem) {
      currentTaskItem.subItems!.push(item);
      continue;
    }

    if (item.is_subagent) {
      continue;
    }

    // Don't reset currentTaskItem — when the agent calls tools in
    // parallel (e.g. Explore + 3 Reads), direct tools interleave with
    // the subagent's internal tools.  Keeping currentTaskItem alive
    // ensures later is_subagent items still get grouped correctly.
    result.push(item);
  }

  return result;
}

function groupChatItems(items: ChatMessageItem[]) {
  const groups: { type: string; entries: ChatMessageItem[] }[] = [];
  for (const item of items) {
    const last = groups[groups.length - 1];
    if (last && last.type === item.type) {
      last.entries.push(item);
    } else {
      groups.push({ type: item.type, entries: [item] });
    }
  }
  return groups;
}

function renderQuestionEntries(
  entries: ChatMessageItem[],
  keyPrefix: string,
  onAnswer?: (toolId: string, answers: any) => Promise<void>,
) {
  if (!onAnswer) return null;
  return entries.map((entry, idx) => (
    <AskQuestionCard
      key={`${keyPrefix}-ask-${entry.tool!.id}-${idx}`}
      tool={entry.tool!}
      onAnswer={onAnswer}
    />
  ));
}

function renderStandardGroups(
  groups: { type: string; entries: ChatMessageItem[] }[],
  keyPrefix: string,
  now: number,
  onCancelTool?: (toolId: string) => void,
  animate?: boolean,
) {
  return groups.map((g, gi) => {
    if (g.type === 'thinking') {
      const text = g.entries.map(e => e.content || '').join('\n\n');
      return <ThinkingBlock key={`${keyPrefix}-thinking-${gi}`} thinking={text} />;
    }
    if (g.type === 'tool') {
      const rendered: React.ReactNode[] = [];
      let regularBuf: RemoteToolStatus[] = [];
      let readBuf: RemoteToolStatus[] = [];

      const flushRead = () => {
        if (readBuf.length > 0) {
          rendered.push(
            <ReadFilesToggle key={`${keyPrefix}-read-${gi}-${rendered.length}`} tools={readBuf} />,
          );
          readBuf = [];
        }
      };

      const flushRegular = () => {
        if (regularBuf.length > 0) {
          rendered.push(
            <ToolList key={`${keyPrefix}-tl-${gi}-${rendered.length}`} tools={regularBuf} now={now} onCancelTool={onCancelTool} />,
          );
          regularBuf = [];
        }
      };

      const flushAll = () => { flushRead(); flushRegular(); };

      for (const entry of g.entries) {
        if (entry.tool?.name === 'Task') {
          flushAll();
          rendered.push(
            <TaskToolCard key={`${keyPrefix}-task-${gi}-${rendered.length}`} tool={entry.tool!} now={now} subItems={entry.subItems} onCancelTool={onCancelTool} />,
          );
        } else if (entry.tool?.name === 'TodoWrite') {
          flushAll();
          rendered.push(<TodoCard key={`${keyPrefix}-todo-${gi}-${rendered.length}`} tool={entry.tool!} />);
        } else if (entry.tool && READ_LIKE_TOOLS.has(entry.tool.name)) {
          flushRegular();
          readBuf.push(entry.tool);
        } else if (entry.tool) {
          flushRead();
          regularBuf.push(entry.tool);
        }
      }
      flushAll();

      return <React.Fragment key={`${keyPrefix}-tool-${gi}`}>{rendered}</React.Fragment>;
    }
    if (g.type === 'text') {
      const text = g.entries.map(e => e.content || '').join('');
      return text ? (
        <div key={`${keyPrefix}-text-${gi}`} className="chat-msg__assistant-content">
          {animate ? <TypewriterText content={text} /> : <MarkdownContent content={text} />}
        </div>
      ) : null;
    }
    return null;
  });
}

// ─── Ordered Items renderer ─────────────────────────────────────────────────

function renderOrderedItems(
  rawItems: ChatMessageItem[],
  now: number,
  onCancelTool?: (toolId: string) => void,
  onAnswer?: (toolId: string, answers: any) => Promise<void>,
) {
  const items = filterSubagentItems(rawItems);
  const askEntries = items.filter(item => isPendingAskUserQuestion(item.tool));
  if (askEntries.length === 0) {
    return renderStandardGroups(groupChatItems(items), 'ordered', now, onCancelTool);
  }

  const beforeAskItems: ChatMessageItem[] = [];
  const afterAskItems: ChatMessageItem[] = [];
  let foundFirstAsk = false;
  for (const item of items) {
    if (isPendingAskUserQuestion(item.tool)) {
      foundFirstAsk = true;
    } else if (!foundFirstAsk) {
      beforeAskItems.push(item);
    } else {
      afterAskItems.push(item);
    }
  }

  return (
    <>
      {renderStandardGroups(groupChatItems(beforeAskItems), 'ordered-before', now, onCancelTool)}
      {renderQuestionEntries(askEntries, 'ordered', onAnswer)}
      {renderStandardGroups(groupChatItems(afterAskItems), 'ordered-after', now, onCancelTool)}
    </>
  );
}

// ─── Active turn items renderer (with AskUserQuestion support) ─────────────

function renderActiveTurnItems(
  rawItems: ChatMessageItem[],
  now: number,
  sessionMgr: RemoteSessionManager,
  setError: (e: string) => void,
  onAnswer: (toolId: string, answers: any) => Promise<void>,
) {
  const items = filterSubagentItems(rawItems);
  const askEntries = items.filter(item => isPendingAskUserQuestion(item.tool));
  const onCancel = (toolId: string) => {
    sessionMgr.cancelTool(toolId, 'User cancelled').catch(err => { setError(String(err)); });
  };

  if (askEntries.length === 0) {
    return renderStandardGroups(groupChatItems(items), 'active', now, onCancel, true);
  }

  const beforeAskItems: ChatMessageItem[] = [];
  const afterAskItems: ChatMessageItem[] = [];
  let foundFirstAsk = false;
  for (const item of items) {
    if (isPendingAskUserQuestion(item.tool)) {
      foundFirstAsk = true;
    } else if (!foundFirstAsk) {
      beforeAskItems.push(item);
    } else {
      afterAskItems.push(item);
    }
  }

  return (
    <>
      {renderStandardGroups(groupChatItems(beforeAskItems), 'active-before', now, onCancel, true)}
      {renderQuestionEntries(askEntries, 'active', onAnswer)}
      {renderStandardGroups(groupChatItems(afterAskItems), 'active-after', now, onCancel, true)}
    </>
  );
}

// ─── Theme toggle icon ─────────────────────────────────────────────────────

const ThemeToggleIcon: React.FC<{ isDark: boolean }> = ({ isDark }) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
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

const ChatPage: React.FC<ChatPageProps> = ({ sessionMgr, sessionId, sessionName, onBack, autoFocus }) => {
  const {
    getMessages,
    setMessages,
    appendNewMessages,
    activeTurn,
    setActiveTurn,
    error,
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
  const [imageAnalyzing, setImageAnalyzing] = useState(false);
  const [optimisticMsg, setOptimisticMsg] = useState<{
    id: string; text: string; images: { name: string; data_url: string }[];
  } | null>(null);
  const [inputExpanded, setInputExpanded] = useState(!!autoFocus);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const inputBarRef = useRef<HTMLDivElement>(null);
  const pollerRef = useRef<SessionPoller | null>(null);

  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const [expandedMsgIds, setExpandedMsgIds] = useState<Set<string>>(new Set());

  const isStreaming = activeTurn != null && activeTurn.status === 'active';

  const [now, setNow] = useState(() => Date.now());
  const handleAnswerQuestion = useCallback(async (toolId: string, answers: any) => {
    try {
      await sessionMgr.answerQuestion(toolId, answers);
    } catch (err) {
      setError(String(err));
      throw err;
    }
  }, [sessionMgr, setError]);

  useEffect(() => {
    if (!isStreaming) return;
    const timer = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(timer);
  }, [isStreaming]);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), 5000);
    return () => clearTimeout(t);
  }, [error, setError]);

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
  const initialScrollDone = useRef(false);
  useEffect(() => {
    initialScrollDone.current = false;
    loadMessages().then(() => {
      const initialMsgCount = useMobileStore.getState().getMessages(sessionId).length;

      // Scroll to bottom after initial load — use rAF + setTimeout to ensure
      // the DOM has finished laying out the newly rendered messages.
      requestAnimationFrame(() => {
        setTimeout(() => {
          messagesEndRef.current?.scrollIntoView({ behavior: 'auto' });
          initialScrollDone.current = true;
          prevMsgCountRef.current = useMobileStore.getState().getMessages(sessionId).length;
        }, 50);
      });

      const poller = new SessionPoller(sessionMgr, sessionId, (resp: PollResponse) => {
        if (resp.new_messages && resp.new_messages.length > 0) {
          appendNewMessages(sessionId, resp.new_messages);
        }

        // Detect count mismatch (messages inserted in the middle due to
        // persistence race).  When the local count doesn't match the server
        // total, do a full reload to pick up all messages.
        if (resp.total_msg_count != null) {
          const localCount = useMobileStore.getState().getMessages(sessionId).length;
          if (localCount !== resp.total_msg_count) {
            sessionMgr.getSessionMessages(sessionId, 200).then(fresh => {
              useMobileStore.getState().setMessages(sessionId, fresh.messages);
            }).catch(() => {});
          }
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

  const prevMsgCountRef = useRef(0);
  useEffect(() => {
    if (!initialScrollDone.current) return;
    if (messages.length !== prevMsgCountRef.current) {
      const isNewAppend = messages.length > prevMsgCountRef.current;
      prevMsgCountRef.current = messages.length;
      if (isNewAppend && !isLoadingMore) {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
      }
    }
  }, [messages.length, isLoadingMore]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    messagesEndRef.current?.scrollIntoView({ behavior: 'auto' });
  }, [activeTurn, isStreaming]);

  useEffect(() => {
    if (optimisticMsg) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [optimisticMsg]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    const container = messagesContainerRef.current;
    if (!container) return;
    const tid = setInterval(() => {
      const gap = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (gap > 10 && gap < 300) {
        container.scrollTop = container.scrollHeight;
      }
    }, 200);
    return () => clearInterval(tid);
  }, [isStreaming]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    const imgs = pendingImages;
    if ((!text && imgs.length === 0) || isStreaming || imageAnalyzing) return;
    setInput('');
    setPendingImages([]);
    setInputExpanded(false);

    const hasImages = imgs.length > 0;
    const imageContexts = hasImages
      ? imgs.map((img, idx) => {
          const mimeType = img.dataUrl.split(';')[0]?.replace('data:', '') || 'image/png';
          return {
            id: `mobile_img_${Date.now()}_${idx}`,
            data_url: img.dataUrl,
            mime_type: mimeType,
            metadata: { name: img.name, source: 'remote' },
          };
        })
      : undefined;

    if (hasImages) {
      setOptimisticMsg({
        id: `opt_${Date.now()}`,
        text: text || '',
        images: imgs.map(i => ({ name: i.name, data_url: i.dataUrl })),
      });
      setImageAnalyzing(true);
    }

    try {
      await sessionMgr.sendMessage(sessionId, text || '(see attached images)', agentMode, imageContexts);
      pollerRef.current?.nudge();
    } catch (e: any) {
      setError(e.message);
    } finally {
      setImageAnalyzing(false);
      setOptimisticMsg(null);
    }
  }, [input, pendingImages, isStreaming, sessionId, sessionMgr, setError, agentMode]);

  const handleImageSelect = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    const maxImages = 5;
    const remaining = maxImages - pendingImages.length;
    const toProcess = Array.from(files).slice(0, remaining);

    const { compressImageFile } = await import('../services/imageCompressor');
    for (const file of toProcess) {
      try {
        const compressed = await compressImageFile(file);
        setPendingImages((prev) => {
          if (prev.length >= maxImages) return prev;
          return [...prev, { name: compressed.name, dataUrl: compressed.dataUrl }];
        });
      } catch {
        const reader = new FileReader();
        reader.onload = () => {
          const dataUrl = reader.result as string;
          setPendingImages((prev) => {
            if (prev.length >= maxImages) return prev;
            return [...prev, { name: file.name, dataUrl }];
          });
        };
        reader.readAsDataURL(file);
      }
    }
    e.target.value = '';
  }, [pendingImages.length]);

  const removeImage = useCallback((idx: number) => {
    setPendingImages((prev) => prev.filter((_, i) => i !== idx));
  }, []);

  const expandInput = useCallback(() => {
    setInputExpanded(true);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  useEffect(() => {
    if (autoFocus) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [autoFocus]);

  useEffect(() => {
    if (!inputExpanded) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (inputBarRef.current && !inputBarRef.current.contains(e.target as Node)) {
        if (!input.trim() && pendingImages.length === 0) {
          setInputExpanded(false);
        }
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [inputExpanded, input, pendingImages.length]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleCancel = async () => {
    try {
      await sessionMgr.cancelTask(sessionId, activeTurn?.turn_id);
    } catch {
      // best effort
    }
  };

  const workspaceName = currentWorkspace?.project_name || currentWorkspace?.path?.split('/').pop() || '';
  const gitBranch = currentWorkspace?.git_branch;
  const displayName = liveTitle || sessionName || 'Session';

  return (
    <div className="chat-page">
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
            {workspaceName && (
              <div className="chat-page__header-workspace" title={currentWorkspace?.path}>
                <span className="chat-page__workspace-name">{workspaceName}</span>
                {gitBranch && (
                  <span className="chat-page__workspace-branch" title={gitBranch}>
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
                    {truncateMiddle(gitBranch, 28)}
                  </span>
                )}
              </div>
            )}
          </div>
          <div className="chat-page__header-right">
            <button className="chat-page__theme-btn" onClick={toggleTheme} aria-label="Toggle theme">
              <ThemeToggleIcon isDark={isDark} />
            </button>
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="chat-page__messages" ref={messagesContainerRef} onScroll={handleScroll}>
        {isLoadingMore && (
          <div className="chat-page__load-more-indicator">Loading older messages…</div>
        )}

        {(() => {
          // Find the last user message index to determine which assistant
          // responses are "old" and can be collapsed.
          const lastUserIdx = messages.reduceRight(
            (found, m, i) => (found < 0 && m.role === 'user' ? i : found),
            -1,
          );

          return messages.map((m, idx) => {
            if (m.role === 'system' || m.role === 'tool') return null;

            if (m.role === 'user') {
              const userText = m.content
                .replace(/#img:\S+\s*/g, '')
                .replace(/\[Image:.*?\]\n(?:Path:.*?\n|Image ID:.*?\n)?/g, '')
                .trim();
              return (
                <div key={m.id} className="chat-msg chat-msg--user">
                  <div className="chat-msg__user-card">
                    <div className="chat-msg__user-avatar">U</div>
                    <div className="chat-msg__user-content">
                      {userText}
                      {m.images && m.images.length > 0 && (
                        <div className="chat-msg__user-images">
                          {m.images.map((img, imgIdx) => (
                            <img
                              key={imgIdx}
                              src={img.data_url}
                              alt={img.name}
                              className="chat-msg__user-image"
                            />
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              );
            }

            const hasItems = m.items && m.items.length > 0;
            const hasContent = m.thinking || (m.tools && m.tools.length > 0) || m.content;
            if (!hasItems && !hasContent) return null;

            const isOldResponse = idx < lastUserIdx;
            const isExpanded = expandedMsgIds.has(m.id);

            if (isOldResponse && !isExpanded) {
              return (
                <div key={m.id} className="chat-msg chat-msg--assistant chat-msg--collapsed">
                  <button
                    className="chat-msg__response-toggle"
                    onClick={() => setExpandedMsgIds(prev => {
                      const next = new Set(prev);
                      next.add(m.id);
                      return next;
                    })}
                  >
                    <span className="chat-msg__response-chevron">
                      <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                        <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                      </svg>
                    </span>
                    <span className="chat-msg__response-label">Show response</span>
                  </button>
                </div>
              );
            }

            return (
              <div key={m.id} className="chat-msg chat-msg--assistant">
                {isOldResponse && isExpanded && (
                  <button
                    className="chat-msg__response-toggle"
                    onClick={() => setExpandedMsgIds(prev => {
                      const next = new Set(prev);
                      next.delete(m.id);
                      return next;
                    })}
                  >
                    <span className="chat-msg__response-chevron is-open">
                      <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                        <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                      </svg>
                    </span>
                    <span className="chat-msg__response-label">Hide response</span>
                  </button>
                )}
                {hasItems ? (
                  renderOrderedItems(m.items!, now, undefined, handleAnswerQuestion)
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
          });
        })()}

        {/* Active turn overlay (streaming or completed-pending-persist) */}
        {activeTurn && (() => {
          const turn = activeTurn;
          const turnIsActive = turn.status === 'active';

          if (turn.items && turn.items.length > 0) {
            return (
              <div className="chat-msg chat-msg--assistant">
                {turnIsActive
                  ? renderActiveTurnItems(turn.items, now, sessionMgr, setError, handleAnswerQuestion)
                  : renderOrderedItems(turn.items, now)}
                {turnIsActive && !turn.thinking && !turn.text && turn.tools.length === 0 && (
                  <div className="chat-msg__assistant-content"><TypingDots /></div>
                )}
              </div>
            );
          }

          const taskTools = turn.tools.filter(t => t.name === 'Task');
          const hasRunningSubagent = taskTools.some(t => t.status === 'running');
          const askTools = turn.tools.filter(
            t => t.name === 'AskUserQuestion' && t.status === 'running' && t.tool_input,
          );
          const askToolIds = new Set(askTools.map(t => t.id));
          const regularTools = turn.tools.filter(t => t.name !== 'Task' && !askToolIds.has(t.id));
          const subItemsForTask: ChatMessageItem[] = hasRunningSubagent
            ? [
                ...(turn.thinking ? [{ type: 'thinking' as const, content: turn.thinking }] : []),
                ...regularTools.map(t => ({ type: 'tool' as const, tool: t })),
              ]
            : [];
          const onCancel = (toolId: string) => {
            sessionMgr.cancelTool(toolId, 'User cancelled').catch(err => { setError(String(err)); });
          };

          return (
            <div className="chat-msg chat-msg--assistant">
              {!hasRunningSubagent && (turn.thinking || turnIsActive) && (
                <ThinkingBlock
                  thinking={turn.thinking}
                  streaming={turnIsActive && !turn.thinking && !turn.text}
                />
              )}
              {taskTools.map(t => (
                <TaskToolCard
                  key={t.id}
                  tool={t}
                  now={now}
                  subItems={t.status === 'running' ? subItemsForTask : undefined}
                  onCancelTool={onCancel}
                />
              ))}
              {!hasRunningSubagent && regularTools.length > 0 && (
                <ToolList tools={regularTools} now={now} onCancelTool={onCancel} />
              )}
              {turnIsActive && askTools.map(at => (
                <AskQuestionCard
                  key={at.id}
                  tool={at}
                  onAnswer={handleAnswerQuestion}
                />
              ))}
              {!hasRunningSubagent && turn.text ? (
                <div className="chat-msg__assistant-content">
                  {turnIsActive ? <TypewriterText content={turn.text} /> : <MarkdownContent content={turn.text} />}
                </div>
              ) : turnIsActive && !turn.thinking && turn.tools.length === 0 ? (
                <div className="chat-msg__assistant-content"><TypingDots /></div>
              ) : null}
            </div>
          );
        })()}

        {/* Optimistic user message with images (shown immediately before server responds) */}
        {optimisticMsg && (
          <div className="chat-msg chat-msg--user">
            <div className="chat-msg__user-card">
              <div className="chat-msg__user-avatar">U</div>
              <div className="chat-msg__user-content">
                {optimisticMsg.text}
                {optimisticMsg.images.length > 0 && (
                  <div className="chat-msg__user-images">
                    {optimisticMsg.images.map((img, i) => (
                      <img key={i} src={img.data_url} alt={img.name} className="chat-msg__user-image" />
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Image analysis indicator */}
        {imageAnalyzing && (
          <div className="chat-msg chat-msg--assistant">
            <div className="chat-msg__assistant-card">
              <div className="chat-msg__image-analyzing">
                <div className="chat-msg__image-analyzing-icon">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="3"/>
                    <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
                  </svg>
                </div>
                <span>Analyzing image with vision model...</span>
                <TypingDots />
              </div>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Floating Input Bar — two-stage (matches desktop ChatInput) */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/jpg,image/gif,image/webp"
        multiple
        style={{ display: 'none' }}
        onChange={handleFileChange}
      />
      <div
        className={`chat-page__input-wrap ${inputExpanded ? 'is-expanded' : ''}`}
        ref={inputBarRef}
      >
        <div
          className="chat-page__input-box"
          onClick={!inputExpanded && !isStreaming ? expandInput : undefined}
        >
          {/* Input area */}
          <div className="chat-page__input-area">
            {inputExpanded ? (
              <textarea
                ref={inputRef}
                className="chat-page__input"
                placeholder="How can I help you..."
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                rows={1}
                disabled={isStreaming || imageAnalyzing}
              />
            ) : (
              <span className="chat-page__input-placeholder">
                {imageAnalyzing ? 'Analyzing image...' : isStreaming ? 'BitFun is working...' : 'How can I help you...'}
              </span>
            )}
          </div>

          {/* Actions bar */}
          <div className="chat-page__input-actions">
            <div className="chat-page__input-actions-left">
              {inputExpanded && pendingImages.length > 0 && (
                <div className="chat-page__image-preview-row">
                  {pendingImages.map((img, idx) => (
                    <div key={idx} className="chat-page__image-thumb">
                      <img src={img.dataUrl} alt={img.name} />
                      <button className="chat-page__image-remove" onClick={() => removeImage(idx)}>×</button>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="chat-page__input-actions-right">
              {inputExpanded && (
                <>
                  <button
                    className={`chat-page__mode-pill${agentMode !== 'agentic' ? ` chat-page__mode-pill--${agentMode}` : ''}`}
                    onClick={() => {
                      const modes: AgentMode[] = ['agentic', 'Plan', 'debug'];
                      const idx = modes.indexOf(agentMode);
                      setAgentMode(modes[(idx + 1) % modes.length]);
                    }}
                    disabled={isStreaming}
                  >
                    {MODE_OPTIONS.find(m => m.id === agentMode)?.label}
                  </button>
                  <button
                    className="chat-page__action-btn"
                    onClick={handleImageSelect}
                    disabled={isStreaming || pendingImages.length >= 5}
                    aria-label="Attach image"
                  >
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect width="18" height="18" x="3" y="3" rx="2" ry="2"/>
                      <circle cx="9" cy="9" r="2"/>
                      <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>
                    </svg>
                  </button>
                </>
              )}
              {isStreaming || imageAnalyzing ? (
                <button className="chat-page__send-btn is-stop" onClick={imageAnalyzing ? undefined : handleCancel} aria-label="Stop" disabled={imageAnalyzing}>
                  {imageAnalyzing ? (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ animation: 'analyzeSpin 2s linear infinite' }}>
                      <circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2"/>
                    </svg>
                  ) : (
                    <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                      <rect x="3" y="3" width="10" height="10" rx="2" fill="currentColor"/>
                    </svg>
                  )}
                </button>
              ) : (
                <button
                  className="chat-page__send-btn"
                  onClick={inputExpanded ? handleSend : undefined}
                  disabled={!input.trim() && pendingImages.length === 0}
                >
                  <svg width="12" height="12" viewBox="0 0 20 20" fill="none">
                    <path d="M10 3L10 17M10 3L5 8M10 3L15 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {error && (
        <div className="chat-page__toast" onClick={() => setError(null)}>
          {error}
        </div>
      )}
    </div>
  );
};

export default ChatPage;
