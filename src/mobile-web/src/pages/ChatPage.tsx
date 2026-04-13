import React, { useEffect, useLayoutEffect, useRef, useState, useCallback, useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus, vs } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { useI18n } from '../i18n';
import {
  RemoteSessionManager,
  SessionPoller,
  type PollResponse,
  type ActiveTurnSnapshot,
  type RemoteToolStatus,
  type ChatMessage,
  type ChatMessageItem,
  type RemoteModelCatalog,
  type RemoteModelConfig,
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

const COMPUTER_LINK_PREFIX = 'computer://';
const FILE_LINK_PREFIX = 'file://';
const WORKSPACE_FOLDER_PLACEHOLDER = '{{workspaceFolder}}';
const MOBILE_LAST_SELECTED_MODEL_ID_KEY = 'bitfun.mobile.last_selected_model_id';

const CODE_FILE_EXTENSIONS = new Set([
  'js', 'jsx', 'ts', 'tsx', 'mjs', 'cjs', 'mts', 'cts',
  'py', 'pyw', 'pyi',
  'rs', 'go', 'java', 'kt', 'kts', 'scala', 'groovy',
  'c', 'cpp', 'cc', 'cxx', 'h', 'hpp', 'hxx', 'hh',
  'cs', 'rb', 'php', 'swift',
  'vue', 'svelte',
  'css', 'scss', 'less', 'sass',
  'json', 'jsonc', 'yaml', 'yml', 'toml', 'xml',
  'md', 'mdx', 'rst', 'txt',
  'sh', 'bash', 'zsh', 'fish', 'ps1', 'bat', 'cmd',
  'sql', 'graphql', 'gql', 'proto',
  'lock', 'env', 'ini', 'cfg', 'conf',
  'cj', 'ets',
  'editorconfig', 'gitignore',
  'log',
]);

const DOWNLOADABLE_EXTENSIONS = new Set([
  'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx',
  'odt', 'ods', 'odp', 'rtf', 'pages', 'numbers', 'key',
  'png', 'jpg', 'jpeg', 'gif', 'bmp', 'svg', 'webp', 'ico', 'tiff', 'tif',
  'zip', 'tar', 'gz', 'bz2', '7z', 'rar', 'dmg', 'iso', 'xz',
  'mp3', 'wav', 'ogg', 'flac', 'aac', 'm4a', 'wma',
  'mp4', 'avi', 'mkv', 'mov', 'webm', 'wmv', 'flv',
  'csv', 'tsv', 'sqlite', 'db', 'parquet',
  'epub', 'mobi',
  'apk', 'ipa', 'exe', 'msi', 'deb', 'rpm',
  'ttf', 'otf', 'woff', 'woff2',
]);

function normalizeFileLikeHref(rawHref: string): string {
  let filePath = rawHref;

  if (rawHref.startsWith(COMPUTER_LINK_PREFIX)) {
    filePath = rawHref.slice(COMPUTER_LINK_PREFIX.length);
  } else if (rawHref.startsWith(FILE_LINK_PREFIX)) {
    filePath = rawHref.slice(FILE_LINK_PREFIX.length);
  } else if (rawHref.startsWith('file:')) {
    filePath = rawHref.slice('file:'.length);
  }

  if (filePath.startsWith(WORKSPACE_FOLDER_PLACEHOLDER)) {
    filePath = filePath.slice(WORKSPACE_FOLDER_PLACEHOLDER.length);
    if (filePath.startsWith('/')) {
      filePath = filePath.slice(1);
    }
  }

  // Normalize URI-like Windows absolute paths such as `/C:/Users/...`.
  if (/^\/[A-Za-z]:[\\/]/.test(filePath)) {
    filePath = filePath.slice(1);
  }

  try {
    return decodeURIComponent(filePath);
  } catch {
    return filePath;
  }
}

/**
 * Detect local file links: absolute paths, file:// URLs, computer:// URLs, and
 * relative paths pointing to downloadable files. Returns the normalized file
 * path or null.
 *
 * - Absolute paths (`/Users/.../file.pdf`): use CODE_FILE_EXTENSIONS blacklist
 * - Relative paths (`report.pptx`, `./output.pdf`): use DOWNLOADABLE_EXTENSIONS whitelist
 */
function isLocalFileLink(href: string): string | null {
  if (!href || href === '/') return null;

  let filePath: string;
  if (
    href.startsWith(COMPUTER_LINK_PREFIX) ||
    href.startsWith(FILE_LINK_PREFIX) ||
    href.startsWith('file:')
  ) {
    filePath = normalizeFileLikeHref(href);
  } else if (href.includes('://') || href.startsWith('#') || href.startsWith('//')) {
    return null;
  } else {
    filePath = normalizeFileLikeHref(href);
  }

  if (filePath.startsWith('/')) {
    const segments = filePath.split('/').filter(Boolean);
    if (segments.length < 2) return null;
  }

  const fileName = filePath.split('/').pop() || '';
  const dotIdx = fileName.lastIndexOf('.');
  if (dotIdx <= 0) return null;

  const ext = fileName.slice(dotIdx + 1).toLowerCase();
  if (!ext) return null;

  if (filePath.startsWith('/')) {
    if (CODE_FILE_EXTENSIONS.has(ext)) return null;
  } else {
    if (!DOWNLOADABLE_EXTENSIONS.has(ext)) return null;
  }

  return filePath;
}

function formatFileSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

const FileTextIcon: React.FC<{ size?: number; style?: React.CSSProperties }> = ({ size = 20, style }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    style={style}
    aria-hidden="true"
  >
    <path d="M15.3929 4.05365L14.8912 4.61112L15.3929 4.05365ZM19.3517 7.61654L18.85 8.17402L19.3517 7.61654ZM21.654 10.1541L20.9689 10.4592V10.4592L21.654 10.1541ZM3.17157 20.8284L3.7019 20.2981H3.7019L3.17157 20.8284ZM20.8284 20.8284L20.2981 20.2981L20.2981 20.2981L20.8284 20.8284ZM14 21.25H10V22.75H14V21.25ZM2.75 14V10H1.25V14H2.75ZM21.25 13.5629V14H22.75V13.5629H21.25ZM14.8912 4.61112L18.85 8.17402L19.8534 7.05907L15.8947 3.49618L14.8912 4.61112ZM22.75 13.5629C22.75 11.8745 22.7651 10.8055 22.3391 9.84897L20.9689 10.4592C21.2349 11.0565 21.25 11.742 21.25 13.5629H22.75ZM18.85 8.17402C20.2034 9.3921 20.7029 9.86199 20.9689 10.4592L22.3391 9.84897C21.9131 8.89241 21.1084 8.18853 19.8534 7.05907L18.85 8.17402ZM10.0298 2.75C11.6116 2.75 12.2085 2.76158 12.7405 2.96573L13.2779 1.5653C12.4261 1.23842 11.498 1.25 10.0298 1.25V2.75ZM15.8947 3.49618C14.8087 2.51878 14.1297 1.89214 13.2779 1.5653L12.7405 2.96573C13.2727 3.16993 13.7215 3.55836 14.8912 4.61112L15.8947 3.49618ZM10 21.25C8.09318 21.25 6.73851 21.2484 5.71085 21.1102C4.70476 20.975 4.12511 20.7213 3.7019 20.2981L2.64124 21.3588C3.38961 22.1071 4.33855 22.4392 5.51098 22.5969C6.66182 22.7516 8.13558 22.75 10 22.75V21.25ZM1.25 14C1.25 15.8644 1.24841 17.3382 1.40313 18.489C1.56076 19.6614 1.89288 20.6104 2.64124 21.3588L3.7019 20.2981C3.27869 19.8749 3.02502 19.2952 2.88976 18.2892C2.75159 17.2615 2.75 15.9068 2.75 14H1.25ZM14 22.75C15.8644 22.75 17.3382 22.7516 18.489 22.5969C19.6614 22.4392 20.6104 22.1071 21.3588 21.3588L20.2981 20.2981C19.8749 20.7213 19.2952 20.975 18.2892 21.1102C17.2615 21.2484 15.9068 21.25 14 21.25V22.75ZM21.25 14C21.25 15.9068 21.2484 17.2615 21.1102 18.2892C20.975 19.2952 20.7213 19.8749 20.2981 20.2981L21.3588 21.3588C22.1071 20.6104 22.4392 19.6614 22.5969 18.489C22.7516 17.3382 22.75 15.8644 22.75 14H21.25ZM2.75 10C2.75 8.09318 2.75159 6.73851 2.88976 5.71085C3.02502 4.70476 3.27869 4.12511 3.7019 3.7019L2.64124 2.64124C1.89288 3.38961 1.56076 4.33855 1.40313 5.51098C1.24841 6.66182 1.25 8.13558 1.25 10H2.75ZM10.0298 1.25C8.15538 1.25 6.67442 1.24842 5.51887 1.40307C4.34232 1.56054 3.39019 1.8923 2.64124 2.64124L3.7019 3.7019C4.12453 3.27928 4.70596 3.02525 5.71785 2.88982C6.75075 2.75158 8.11311 2.75 10.0298 2.75V1.25Z" fill="currentColor"/>
    <path d="M6 14.5H14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
    <path d="M6 18H11.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
    <path d="M13 2.5V5C13 7.35702 13 8.53553 13.7322 9.26777C14.4645 10 15.643 10 18 10H22" stroke="currentColor" strokeWidth="1.5"/>
  </svg>
);

type FileCardState =
  | { status: 'loading' }
  | { status: 'ready'; name: string; size: number; mimeType: string }
  | { status: 'downloading'; name: string; size: number; mimeType: string; progress: number }
  | { status: 'done'; name: string; size: number; mimeType: string }
  | { status: 'error'; message: string };

interface FileCardProps {
  path: string;
  onGetFileInfo: (path: string) => Promise<{ name: string; size: number; mimeType: string }>;
  onDownload: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>;
}

const FileCard: React.FC<FileCardProps> = ({ path, onGetFileInfo, onDownload }) => {
  const { isDark } = useTheme();
  const { t } = useI18n();
  const [state, setState] = useState<FileCardState>({ status: 'loading' });
  const onGetFileInfoRef = useRef(onGetFileInfo);
  onGetFileInfoRef.current = onGetFileInfo;

  useEffect(() => {
    let cancelled = false;
    onGetFileInfoRef.current(path)
      .then(({ name, size, mimeType }) => {
        if (!cancelled) setState({ status: 'ready', name, size, mimeType });
      })
      .catch((err) => {
        if (!cancelled)
          setState({ status: 'error', message: err instanceof Error ? err.message : String(err) });
      });
    return () => { cancelled = true; };
  }, [path]);

  const handleClick = useCallback(async () => {
    if (state.status !== 'ready' && state.status !== 'done') return;
    const info = state as { status: 'ready' | 'done'; name: string; size: number; mimeType: string };
    setState({ status: 'downloading', name: info.name, size: info.size, mimeType: info.mimeType, progress: 0 });
    try {
      await onDownload(path, (downloaded, total) => {
        setState(prev => {
          if (prev.status !== 'downloading') return prev;
          return { ...prev, progress: total > 0 ? downloaded / total : 0 };
        });
      });
      setState({ status: 'done', name: info.name, size: info.size, mimeType: info.mimeType });
    } catch {
      setState({ status: 'ready', name: info.name, size: info.size, mimeType: info.mimeType });
    }
  }, [state, path, onDownload]);

  const cardStyle: React.CSSProperties = {
    display: 'inline-flex',
    alignItems: 'center',
    gap: '10px',
    padding: '10px 14px',
    border: `1px solid ${isDark ? 'rgba(255,255,255,0.12)' : 'rgba(0,0,0,0.12)'}`,
    borderRadius: '10px',
    background: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.03)',
    cursor: state.status === 'ready' || state.status === 'done' ? 'pointer' : 'default',
    maxWidth: '300px',
    verticalAlign: 'middle',
    transition: 'background 0.15s',
  };

  const iconColor = isDark ? 'rgba(255,255,255,0.6)' : 'rgba(0,0,0,0.5)';

  if (state.status === 'loading') {
    return (
      <span className="file-card" style={cardStyle}>
        <FileTextIcon size={20} style={{ color: iconColor, flexShrink: 0 }} />
        <span style={{ fontSize: '0.8rem', opacity: 0.5 }}>{t('chat.fileLoading')}</span>
      </span>
    );
  }
  if (state.status === 'error') {
    return (
      <span className="file-card" style={{ ...cardStyle, cursor: 'default', opacity: 0.5 }} title={state.message}>
        <FileTextIcon size={20} style={{ color: iconColor, flexShrink: 0 }} />
        <span style={{ fontSize: '0.8rem' }}>{t('chat.fileUnavailable')}</span>
      </span>
    );
  }

  const { name, size } = state as { name: string; size: number; mimeType: string; status: string };
  const isDownloading = state.status === 'downloading';
  const isDone = state.status === 'done';

  return (
    <span
      className="file-card"
      style={cardStyle}
      onClick={handleClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(); }}
      title={isDownloading ? t('chat.fileDownloading') : isDone ? t('chat.fileDownloaded') : t('chat.clickToDownload')}
    >
      <FileTextIcon size={20} style={{ color: iconColor, flexShrink: 0 }} />
      <span style={{ minWidth: 0, overflow: 'hidden' }}>
        <span style={{
          display: 'block',
          fontSize: '0.85rem',
          fontWeight: 500,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          color: isDark ? 'rgba(255,255,255,0.9)' : 'rgba(0,0,0,0.85)',
        }}>
          {name}
        </span>
        <span style={{
          display: 'block',
          fontSize: '0.75rem',
          color: isDark ? 'rgba(255,255,255,0.45)' : 'rgba(0,0,0,0.45)',
          marginTop: '2px',
        }}>
          {formatFileSize(size)}
        </span>
      </span>
      <span style={{
        flexShrink: 0,
        fontSize: '0.75rem',
        color: isDone
          ? (isDark ? '#4ade80' : '#16a34a')
          : (isDark ? 'rgba(255,255,255,0.55)' : 'rgba(0,0,0,0.45)'),
      }}>
        {isDownloading ? `${Math.round((state as any).progress * 100)}%` : isDone ? '✓' : '↓'}
      </span>
    </span>
  );
};

interface MarkdownContentProps {
  content: string;
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>;
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>;
}

const MarkdownContent: React.FC<MarkdownContentProps> = ({ content, onFileDownload, onGetFileInfo }) => {
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

    a({ href, children }: any) {
      const isComputerLink =
        typeof href === 'string' && href.startsWith(COMPUTER_LINK_PREFIX);

      if (isComputerLink && onGetFileInfo && onFileDownload) {
        const filePath = normalizeFileLikeHref(href);
        return (
          <FileCard
            path={filePath}
            onGetFileInfo={onGetFileInfo}
            onDownload={onFileDownload}
          />
        );
      }
      // Fallback: plain clickable link when only onFileDownload is available.
      if (isComputerLink && onFileDownload) {
        const filePath = normalizeFileLikeHref(href);
        return (
          <button
            className="file-link"
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); onFileDownload(filePath); }}
            type="button"
            style={{
              cursor: 'pointer',
              color: 'var(--color-accent, #3b82f6)',
              textDecoration: 'underline',
              background: 'none',
              border: 'none',
              font: 'inherit',
              padding: 0,
            }}
          >
            {children}
          </button>
        );
      }

      // Local file path (e.g. /Users/.../report.pdf) → FileCard, excluding code files
      if (onGetFileInfo && onFileDownload) {
        const localPath = typeof href === 'string' ? isLocalFileLink(href) : null;
        if (localPath) {
          return (
            <FileCard
              path={localPath}
              onGetFileInfo={onGetFileInfo}
              onDownload={onFileDownload}
            />
          );
        }
      }

      // Fallback: render as plain text for computer:// links without handler,
      // or as a regular link for http(s) links.
      if (typeof href === 'string') {
        // 所有外部链接都在新标签页打开
        const isExternalLink = href.startsWith('http://') || href.startsWith('https://');
        if (isExternalLink) {
          return (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: 'var(--color-accent, #3b82f6)', textDecoration: 'underline' }}
            >
              {children}
            </a>
          );
        }
      }

      return <span style={{ textDecoration: 'underline', opacity: 0.7 }}>{children}</span>;
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
  }), [syntaxTheme, isDark, onFileDownload, onGetFileInfo]);

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={components}
      urlTransform={(url) => {
        if (url.startsWith('computer://')) return url;
        if (/^(https?|mailto|tel|file):/i.test(url) || url.startsWith('#') || url.startsWith('/')) {
          return url;
        }
        // Preserve relative paths without a protocol (e.g. "report.pptx",
        // "./output.pdf").  Content is from our own AI so javascript:/data:
        // injection is not a concern; those contain ':' and are blocked above.
        if (!url.includes(':')) return url;
        return '';
      }}
    >
      {content}
    </ReactMarkdown>
  );
};

// ─── Thinking (ModelThinkingDisplay-style) ───────────────────────────────────

const ThinkingBlock: React.FC<{
  thinking: string;
  streaming?: boolean;
  isLastItem?: boolean;
}> = ({ thinking, streaming, isLastItem = false }) => {
  const { t } = useI18n();
  const [open, setOpen] = useState(!!streaming);
  const userToggledRef = useRef(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ atTop: true, atBottom: true });
  const displayedThinking = useTypewriter(thinking, !!streaming);

  useEffect(() => {
    if (userToggledRef.current) return;
    if (streaming) {
      setOpen(true);
    } else if (!isLastItem) {
      setOpen(false);
    }
  }, [streaming, isLastItem]);

  useEffect(() => {
    if (!streaming || !open) return;
    const el = wrapperRef.current;
    if (!el) return;
    const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (gap < 80) {
      el.scrollTop = el.scrollHeight;
    }
  }, [displayedThinking, streaming, open]);

  const handleScroll = useCallback(() => {
    const el = wrapperRef.current;
    if (!el) return;
    setScrollState({
      atTop: el.scrollTop < 4,
      atBottom: el.scrollHeight - el.scrollTop - el.clientHeight < 4,
    });
  }, []);

  const handleToggle = useCallback(() => {
    userToggledRef.current = true;
    setOpen(o => !o);
  }, []);

  if (!thinking && !streaming) return null;

  const charCount = thinking.length;
  const label = streaming && charCount === 0
    ? t('chat.thinking')
    : t('chat.thoughtCharacters', { count: charCount });

  return (
    <div className={`chat-thinking ${streaming ? 'chat-thinking--streaming' : ''}`}>
      <button className="chat-thinking__toggle" onClick={handleToggle}>
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
                <MarkdownContent content={streaming ? displayedThinking : thinking} />
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
  explore: 'tools.explore',
  read_file: 'tools.read',
  write_file: 'tools.write',
  list_directory: 'tools.ls',
  bash: 'tools.shell',
  glob: 'tools.glob',
  grep: 'tools.grep',
  create_file: 'tools.write',
  delete_file: 'tools.delete',
  Task: 'tools.task',
  search: 'tools.search',
  edit_file: 'tools.edit',
  web_search: 'tools.web',
  TodoWrite: 'tools.todo',
};

// ─── TodoWrite card ─────────────────────────────────────────────────────────

const TodoCard: React.FC<{ tool: RemoteToolStatus }> = ({ tool }) => {
  const { t } = useI18n();
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
          <span className="chat-todo-card__current chat-todo-card__current--done">{t('chat.allTasksCompleted')}</span>
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
function subItemLabel(
  item: ChatMessageItem,
  t: (key: string, params?: Record<string, string | number>) => string,
): string {
  if (item.type === 'thinking') {
    const len = (item.content || '').length;
    return t('chat.thoughtCharacters', { count: len });
  }
  if (item.type === 'tool' && item.tool) {
    const t = item.tool;
    const preview = t.input_preview ? `: ${t.input_preview}` : '';
    return `${t.name}${preview}`;
  }
  if (item.type === 'text') {
    const len = (item.content || '').length;
    return t('chat.textCharacters', { count: len });
  }
  return '';
}

const TaskToolCard: React.FC<{
  tool: RemoteToolStatus;
  now: number;
  subItems?: ChatMessageItem[];
  onCancelTool?: (toolId: string) => void;
}> = ({ tool, now, subItems = [], onCancelTool }) => {
  const { t } = useI18n();
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
          {taskInfo?.description || t('chat.task')}
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
            aria-label={t('common.cancel')}
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
              {t('chat.toolCalls', { count: subTools.length, suffix: subTools.length === 1 ? '' : 's' })}
            </span>
            <span className="chat-task-card__stat-right">
              <span className="chat-task-card__stat--done">{t('chat.done', { count: subToolsDone })}</span>
              {subToolsRunning > 0 && <span className="chat-task-card__stat--running">{t('chat.running', { count: subToolsRunning })}</span>}
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
                      <span>{subItemLabel(item, t)}</span>
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
  const { t } = useI18n();
  const toolKey = tool.name.toLowerCase().replace(/[\s-]/g, '_');
  const typeLabelKey = TOOL_TYPE_MAP[toolKey] || TOOL_TYPE_MAP[tool.name];
  const typeLabel = typeLabelKey ? t(typeLabelKey) : 'Tool';
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
            aria-label={t('common.cancel')}
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

function getToolSummaryLabel(toolName: string): string {
  return toolName;
}

function buildGroupedToolSummary(
  tools: RemoteToolStatus[],
  t: (key: string, params?: Record<string, string | number>) => string,
): string {
  const counts = new Map<string, { label: string; count: number }>();
  const order: string[] = [];

  for (const tool of tools) {
    const toolKey = tool.name.toLowerCase().replace(/[\s-]/g, '_');
    const typeLabelKey = TOOL_TYPE_MAP[toolKey] || TOOL_TYPE_MAP[tool.name];
    const label = typeLabelKey ? t(typeLabelKey) : getToolSummaryLabel(tool.name);
    const key = label.toLowerCase();
    const existing = counts.get(key);
    if (existing) {
      existing.count += 1;
      continue;
    }
    counts.set(key, { label, count: 1 });
    order.push(key);
  }

  return order
    .map((key) => {
      const entry = counts.get(key)!;
      return `${entry.label} ${entry.count}`;
    })
    .join(', ');
}

const ReadFilesToggle: React.FC<{ tools: RemoteToolStatus[] }> = ({ tools }) => {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  if (tools.length === 0) return null;

  const doneCount = tools.filter(t => t.status === 'completed').length;
  const allDone = doneCount === tools.length;
  const summary = buildGroupedToolSummary(tools, t);
  const label = allDone
    ? t('chat.readToolsDone', { summary })
    : t('chat.readToolsRunning', { summary, doneCount });

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
  const { t } = useI18n();
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
        <span className="chat-tool-list__count">{t('chat.toolCalls', { count: tools.length, suffix: tools.length === 1 ? '' : 's' })}</span>
        <span className="chat-tool-list__stats">
          {doneCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--done">{t('chat.done', { count: doneCount })}</span>}
          {runningCount > 0 && <span className="chat-tool-list__stat chat-tool-list__stat--running">{t('chat.running', { count: runningCount })}</span>}
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

const TypewriterText: React.FC<{
  content: string;
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>;
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>;
}> = ({ content, onFileDownload, onGetFileInfo }) => {
  const displayText = useTypewriter(content, true);
  return <MarkdownContent content={displayText} onFileDownload={onFileDownload} onGetFileInfo={onGetFileInfo} />;
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
  const { t } = useI18n();
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
        <span className="chat-ask-card__count">{t('chat.askQuestionCount', { count: questions.length, suffix: questions.length > 1 ? 's' : '' })}</span>
        {!submitted && !submitting && (
          <span className="chat-ask-card__waiting">{t('chat.waiting')}</span>
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
                  <span className="chat-ask-card__option-label">{t('common.other')}</span>
                  <span className="chat-ask-card__option-desc">{t('common.customTextInput')}</span>
                </button>
              )}
              {isOtherSelected && (
                <input
                  className="chat-ask-card__custom-input"
                  placeholder={t('common.typeYourAnswer')}
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
        {submitted ? t('common.submitted') : submitting ? t('common.submitting') : t('common.submit')}
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
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>,
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>,
  isActiveTurn?: boolean,
) {
  return groups.map((g, gi) => {
    if (g.type === 'thinking') {
      const text = g.entries.map(e => e.content || '').join('\n\n');
      const isLast = isActiveTurn && gi === groups.length - 1;
      return <ThinkingBlock key={`${keyPrefix}-thinking-${gi}`} thinking={text} streaming={isLast} isLastItem={isLast} />;
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
          {animate
            ? <TypewriterText content={text} onFileDownload={onFileDownload} onGetFileInfo={onGetFileInfo} />
            : <MarkdownContent content={text} onFileDownload={onFileDownload} onGetFileInfo={onGetFileInfo} />}
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
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>,
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>,
) {
  const items = filterSubagentItems(rawItems);
  const askEntries = items.filter(item => isPendingAskUserQuestion(item.tool));
  if (askEntries.length === 0) {
    return renderStandardGroups(groupChatItems(items), 'ordered', now, onCancelTool, false, onFileDownload, onGetFileInfo);
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
      {renderStandardGroups(groupChatItems(beforeAskItems), 'ordered-before', now, onCancelTool, false, onFileDownload, onGetFileInfo)}
      {renderQuestionEntries(askEntries, 'ordered', onAnswer)}
      {renderStandardGroups(groupChatItems(afterAskItems), 'ordered-after', now, onCancelTool, false, onFileDownload, onGetFileInfo)}
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
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>,
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>,
) {
  const items = filterSubagentItems(rawItems);
  const askEntries = items.filter(item => isPendingAskUserQuestion(item.tool));
  const onCancel = (toolId: string) => {
    sessionMgr.cancelTool(toolId, 'User cancelled').catch(err => { setError(String(err)); });
  };

  if (askEntries.length === 0) {
    return renderStandardGroups(groupChatItems(items), 'active', now, onCancel, true, onFileDownload, onGetFileInfo, true);
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
      {renderStandardGroups(groupChatItems(beforeAskItems), 'active-before', now, onCancel, true, onFileDownload, onGetFileInfo, true)}
      {renderQuestionEntries(askEntries, 'active', onAnswer)}
      {renderStandardGroups(groupChatItems(afterAskItems), 'active-after', now, onCancel, true, onFileDownload, onGetFileInfo, true)}
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

const SparklesIcon: React.FC<{ className?: string; size?: number }> = ({ className, size = 10 }) => (
  <svg
    className={className}
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.937A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .962 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.582a.5.5 0 0 1 0 .962L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.962 0z" />
    <path d="M20 3v4" />
    <path d="M22 5h-4" />
    <path d="M4 17v2" />
    <path d="M5 18H3" />
  </svg>
);

type ModelSelectionValue = 'auto' | 'primary' | 'fast' | string;

function formatProviderName(provider: string): string {
  const normalized = provider.trim();
  if (!normalized) return 'Unknown';
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function getModelProviderLabel(model: RemoteModelConfig): string {
  const configuredName = model.name?.trim();
  if (configuredName) return configuredName;
  return formatProviderName(model.provider);
}

function formatContextWindow(contextWindow?: number): string | null {
  if (!contextWindow) return null;
  return `${Math.round(contextWindow / 1000)}k`;
}

function isChatCapableModel(model: RemoteModelConfig): boolean {
  return model.enabled && Array.isArray(model.capabilities) && model.capabilities.includes('text_chat');
}

function normalizeSelectedModelId(
  selectedModelId: string | null | undefined,
  catalog: RemoteModelCatalog | null,
): string {
  const value = selectedModelId?.trim();
  if (!value || value === 'auto' || value === 'default') return 'auto';
  if (value === 'primary' || value === 'fast') {
    const defaultId = value === 'primary'
      ? catalog?.default_models?.primary
      : catalog?.default_models?.fast;
    return defaultId && resolveModelSelection(defaultId, catalog) ? value : 'auto';
  }
  return resolveModelSelection(value, catalog) ? value : 'auto';
}

function loadLastSelectedModelId(): string | null {
  if (typeof window === 'undefined') return null;
  const value = window.localStorage.getItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY)?.trim();
  return value || null;
}

function persistLastSelectedModelId(modelId: string): void {
  if (typeof window === 'undefined') return;
  const value = modelId.trim();
  if (!value) {
    window.localStorage.removeItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY);
    return;
  }
  window.localStorage.setItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY, value);
}

function resolvePreferredModelSelection(
  preferredModelId: string | null,
  catalog: RemoteModelCatalog | null,
): { modelId: string | null; fellBackToAuto: boolean } {
  const value = preferredModelId?.trim();
  if (!value) {
    return { modelId: null, fellBackToAuto: false };
  }

  const normalizedModelId = normalizeSelectedModelId(value, catalog);
  const fellBackToAuto = normalizedModelId === 'auto' && value !== 'auto' && value !== 'default';
  return {
    modelId: normalizedModelId,
    fellBackToAuto,
  };
}

function resolveModelSelection(
  modelId: string,
  catalog: RemoteModelCatalog | null,
): RemoteModelConfig | null {
  if (!catalog) return null;
  return catalog.models.find(model => model.id === modelId) || null;
}

function buildModelProviderMeta(model: RemoteModelConfig | null): string | null {
  if (!model) return null;
  const parts = [getModelProviderLabel(model)];
  const context = formatContextWindow(model.context_window);
  if (context) parts.push(context);
  return parts.join(' · ');
}

function getModelDisplayName(model: RemoteModelConfig | null): string {
  if (!model) return '';
  return model.model_name || model.name || '';
}

function isReasoningEnabled(model: RemoteModelConfig | null): boolean {
  if (!model) return false;
  if (model.reasoning_mode) {
    return model.reasoning_mode === 'enabled' || model.reasoning_mode === 'adaptive';
  }
  return !!model.enable_thinking_process;
}

function getSelectedModelInfo(
  selectedModelId: string,
  catalog: RemoteModelCatalog | null,
  t: (key: string, params?: Record<string, string | number>) => string,
): {
  label: string;
  meta: string | null;
  enableThinking: boolean;
  reasoningEffort?: string;
} {
  if (selectedModelId === 'auto') {
    return {
      label: t('chat.modelAuto'),
      meta: t('chat.modelAutoDesc'),
      enableThinking: false,
    };
  }

  if (selectedModelId === 'primary' || selectedModelId === 'fast') {
    const defaultId = selectedModelId === 'primary'
      ? catalog?.default_models?.primary
      : catalog?.default_models?.fast;
    const resolved = defaultId ? resolveModelSelection(defaultId, catalog) : null;
    return {
      label: resolved
        ? (selectedModelId === 'primary' ? t('chat.modelPrimary') : t('chat.modelFast'))
        : t('chat.modelAuto'),
      meta: buildModelProviderMeta(resolved) || t('chat.modelAutoDesc'),
      enableThinking: isReasoningEnabled(resolved),
      reasoningEffort: resolved?.reasoning_effort,
    };
  }

  const resolved = resolveModelSelection(selectedModelId, catalog);
  if (!resolved) {
    return {
      label: t('chat.modelAuto'),
      meta: t('chat.modelAutoDesc'),
      enableThinking: false,
    };
  }

  return {
    label: getModelDisplayName(resolved),
    meta: buildModelProviderMeta(resolved),
    enableThinking: isReasoningEnabled(resolved),
    reasoningEffort: resolved.reasoning_effort,
  };
}

const ModelSelectorPill: React.FC<{
  catalog: RemoteModelCatalog | null;
  selectedModelId: string;
  disabled?: boolean;
  onSelect: (modelId: string) => void | Promise<void>;
}> = ({ catalog, selectedModelId, disabled, onSelect }) => {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const normalizedSelectedModelId = useMemo(
    () => normalizeSelectedModelId(selectedModelId, catalog),
    [catalog, selectedModelId],
  );

  const availableModels = useMemo(
    () => (catalog?.models || []).filter(isChatCapableModel),
    [catalog],
  );
  const resolvedPrimaryModel = useMemo(
    () => resolveModelSelection(catalog?.default_models?.primary || '', catalog),
    [catalog],
  );
  const resolvedFastModel = useMemo(
    () => resolveModelSelection(catalog?.default_models?.fast || '', catalog),
    [catalog],
  );
  const selectedInfo = useMemo(
    () => getSelectedModelInfo(normalizedSelectedModelId, catalog, t),
    [catalog, normalizedSelectedModelId, t],
  );

  useEffect(() => {
    if (!open) return;
    const handleOutside = (event: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [open]);

  if (!catalog) return null;

  const handleSelect = async (modelId: string) => {
    await onSelect(modelId);
    setOpen(false);
  };

  return (
    <div className="chat-model-selector" ref={rootRef}>
      <button
        className={`chat-model-selector__trigger${open ? ' chat-model-selector__trigger--open' : ''}`}
        type="button"
        onClick={() => setOpen(prev => !prev)}
        disabled={disabled}
        aria-label={t('chat.modelSelection')}
      >
        <span className="chat-model-selector__icon" aria-hidden="true">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none">
            <rect x="4" y="4" width="6" height="6" rx="1.5" stroke="currentColor" strokeWidth="1.7" />
            <rect x="14" y="4" width="6" height="6" rx="1.5" stroke="currentColor" strokeWidth="1.7" />
            <rect x="9" y="14" width="6" height="6" rx="1.5" stroke="currentColor" strokeWidth="1.7" />
            <path d="M10 7h4M12 10v4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
          </svg>
        </span>
        <span className="chat-model-selector__name">
          <span className="chat-model-selector__name-text">{selectedInfo.label}</span>
          {selectedInfo.enableThinking && (
            <SparklesIcon className="chat-model-selector__thinking" size={9} />
          )}
        </span>
        {selectedInfo.reasoningEffort && (
          <span className="chat-model-selector__effort">{selectedInfo.reasoningEffort}</span>
        )}
        <span className="chat-model-selector__chevron" aria-hidden="true">
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
            <path d="M4 6L8 10L12 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </button>

      {open && (
        <div className="chat-model-selector__dropdown">
          <div className="chat-model-selector__header">{t('chat.modelSelection')}</div>
          <button
            className={`chat-model-selector__option${normalizedSelectedModelId === 'auto' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('auto')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelAuto')}</span>
              <span className="chat-model-selector__option-meta">{t('chat.modelAutoDesc')}</span>
            </span>
          </button>
          <button
            className={`chat-model-selector__option${normalizedSelectedModelId === 'primary' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('primary')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelPrimary')}</span>
              <span className="chat-model-selector__option-meta chat-model-selector__option-meta--stacked">
                <span className="chat-model-selector__option-meta-line">
                  {getModelDisplayName(resolvedPrimaryModel) || t('chat.modelAuto')}
                </span>
                <span className="chat-model-selector__option-meta-line">
                  {buildModelProviderMeta(resolvedPrimaryModel) || t('chat.modelAutoDesc')}
                </span>
              </span>
            </span>
          </button>
          <button
            className={`chat-model-selector__option${normalizedSelectedModelId === 'fast' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('fast')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelFast')}</span>
              <span className="chat-model-selector__option-meta chat-model-selector__option-meta--stacked">
                <span className="chat-model-selector__option-meta-line">
                  {getModelDisplayName(resolvedFastModel) || t('chat.modelAuto')}
                </span>
                <span className="chat-model-selector__option-meta-line">
                  {buildModelProviderMeta(resolvedFastModel) || t('chat.modelAutoDesc')}
                </span>
              </span>
            </span>
          </button>
          <div className="chat-model-selector__divider" />
          <div className="chat-model-selector__list">
            {availableModels.map(model => {
              const isSelected = normalizedSelectedModelId === model.id;
              return (
                <button
                  key={model.id}
                  className={`chat-model-selector__option${isSelected ? ' is-selected' : ''}`}
                  type="button"
                  onClick={() => void handleSelect(model.id)}
                >
                  <span className="chat-model-selector__option-main">
                    <span className="chat-model-selector__option-name">
                      <span className="chat-model-selector__option-name-text">
                        {getModelDisplayName(model)}
                      </span>
                      {isReasoningEnabled(model) && (
                        <SparklesIcon className="chat-model-selector__option-thinking" size={10} />
                      )}
                    </span>
                    <span className="chat-model-selector__option-meta">
                      {buildModelProviderMeta(model) || formatProviderName(model.provider)}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};

// ─── Agent Mode ─────────────────────────────────────────────────────────────

type AgentMode = 'agentic' | 'Plan' | 'debug';

// ─── ChatPage ───────────────────────────────────────────────────────────────

const ChatPage: React.FC<ChatPageProps> = ({ sessionMgr, sessionId, sessionName, onBack, autoFocus }) => {
  const { t } = useI18n();
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
  const modeOptions: { id: AgentMode; label: string }[] = useMemo(() => ([
    { id: 'agentic', label: t('chat.modeAgentic') },
    { id: 'Plan', label: t('chat.modePlan') },
    { id: 'debug', label: t('chat.modeDebug') },
  ]), [t]);
  const messages = getMessages(sessionId);
  const [input, setInput] = useState('');
  const [agentMode, setAgentMode] = useState<AgentMode>('agentic');
  const [liveTitle, setLiveTitle] = useState(sessionName);
  const [modelCatalog, setModelCatalog] = useState<RemoteModelCatalog | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string>('auto');
  const [modelUpdating, setModelUpdating] = useState(false);
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
  const isLoadingMoreRef = useRef(false);
  const hasMoreRef = useRef(true);
  const modelSelectionInitializedRef = useRef(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const [expandedMsgIds, setExpandedMsgIds] = useState<Set<string>>(new Set());
  const [infoToast, setInfoToast] = useState<string | null>(null);

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

  /** Fetch metadata for a workspace file before the user confirms the download. */
  const handleGetFileInfo = useCallback(
    (filePath: string) => sessionMgr.getFileInfo(filePath, sessionId),
    [sessionId, sessionMgr],
  );

  /** Download a workspace file referenced by a `computer://` link. */
  const handleFileDownload = useCallback(async (
    filePath: string,
    onProgress?: (downloaded: number, total: number) => void,
  ) => {
    try {
      const { name, contentBase64, mimeType } = await sessionMgr.readFile(
        filePath,
        sessionId,
        onProgress,
      );
      const byteCharacters = atob(contentBase64);
      const byteNumbers = new Uint8Array(byteCharacters.length);
      for (let i = 0; i < byteCharacters.length; i++) {
        byteNumbers[i] = byteCharacters.charCodeAt(i);
      }
      const blob = new Blob([byteNumbers], { type: mimeType });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = name;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);
      URL.revokeObjectURL(url);
    } catch (err) {
      // Use the backend's message directly; it's already user-readable.
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    }
  }, [sessionId, sessionMgr, setError]);

  const loadModelCatalog = useCallback(async () => {
    try {
      const catalog = await sessionMgr.getModelCatalog(sessionId);
      setModelCatalog(catalog);
      if (!modelSelectionInitializedRef.current) {
        const preferredSelection = resolvePreferredModelSelection(loadLastSelectedModelId(), catalog);
        const sessionModelId = normalizeSelectedModelId(catalog.session_model_id || 'auto', catalog);
        const nextModelId = preferredSelection.modelId || sessionModelId;

        if (preferredSelection.modelId && preferredSelection.modelId !== sessionModelId) {
          const normalizedModelId = await sessionMgr.setSessionModel(sessionId, preferredSelection.modelId);
          setSelectedModelId(normalizedModelId || 'auto');
          if (preferredSelection.fellBackToAuto && (!normalizedModelId || normalizedModelId === 'auto')) {
            persistLastSelectedModelId('auto');
          }
        } else {
          setSelectedModelId(nextModelId || 'auto');
          if (preferredSelection.fellBackToAuto && nextModelId === 'auto') {
            persistLastSelectedModelId('auto');
          }
        }
        modelSelectionInitializedRef.current = true;
      }
      return catalog;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return null;
    }
  }, [sessionId, sessionMgr, setError]);

  const handleSelectModel = useCallback(async (modelId: string) => {
    if (modelUpdating || isStreaming || imageAnalyzing) return;
    setModelUpdating(true);
    try {
      const normalizedModelId = await sessionMgr.setSessionModel(sessionId, modelId);
      setSelectedModelId(normalizedModelId || 'auto');
      persistLastSelectedModelId(normalizedModelId || 'auto');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setModelUpdating(false);
    }
  }, [imageAnalyzing, isStreaming, modelUpdating, sessionId, sessionMgr, setError]);

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

  useEffect(() => {
    if (!infoToast) return;
    const timer = setTimeout(() => setInfoToast(null), 3200);
    return () => clearTimeout(timer);
  }, [infoToast]);

  const loadMessages = useCallback(async (beforeId?: string) => {
    if (isLoadingMoreRef.current || (!hasMoreRef.current && beforeId)) return;
    try {
      isLoadingMoreRef.current = true;
      setIsLoadingMore(true);
      const resp = await sessionMgr.getSessionMessages(sessionId, 50, beforeId);
      if (beforeId) {
        const currentMsgs = getMessages(sessionId);
        setMessages(sessionId, [...resp.messages, ...currentMsgs]);
      } else {
        setMessages(sessionId, resp.messages);
      }
      setHasMore(resp.has_more);
      hasMoreRef.current = resp.has_more;
    } catch (e: any) {
      setError(e.message);
    } finally {
      isLoadingMoreRef.current = false;
      setIsLoadingMore(false);
    }
  }, [sessionMgr, sessionId, setMessages, setError, getMessages]);

  const isNearBottomRef = useRef(true);
  const BOTTOM_THRESHOLD = 80;

  const handleScroll = useCallback(() => {
    const container = messagesContainerRef.current;
    if (!container) return;

    const gap = container.scrollHeight - container.scrollTop - container.clientHeight;
    isNearBottomRef.current = gap < BOTTOM_THRESHOLD;

    if (container.scrollTop < 100 && hasMore && !isLoadingMore) {
      const msgs = getMessages(sessionId);
      if (msgs.length > 0) loadMessages(msgs[0].id);
    }
  }, [hasMore, isLoadingMore, getMessages, sessionId, loadMessages]);

  // Initial load + start poller
  const initialScrollDone = useRef(false);
  const pendingInitialScroll = useRef(false);
  useEffect(() => {
    modelSelectionInitializedRef.current = false;
    hasMoreRef.current = true;
    isLoadingMoreRef.current = false;
    setHasMore(true);
    setIsLoadingMore(false);
    setModelCatalog(null);
    setSelectedModelId('auto');
  }, [sessionId]);

  useEffect(() => {
    initialScrollDone.current = false;
    pendingInitialScroll.current = false;
    Promise.all([loadMessages(), loadModelCatalog()]).then(([_, initialCatalog]) => {
      const initialMsgCount = useMobileStore.getState().getMessages(sessionId).length;
      pendingInitialScroll.current = true;

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
        if (resp.model_catalog) {
          setModelCatalog(resp.model_catalog);
        }
        setActiveTurn(resp.active_turn ?? null);
      }, initialCatalog?.version || 0);

      poller.start(initialMsgCount);
      pollerRef.current = poller;
    });

    return () => {
      pollerRef.current?.stop();
      pollerRef.current = null;
      setActiveTurn(null);
    };
  }, [sessionId, sessionMgr, loadMessages, loadModelCatalog, appendNewMessages, setActiveTurn, updateSessionName]);

  const prevMsgCountRef = useRef(0);

  // Scroll to bottom BEFORE paint on initial message load,
  // so the user never sees the list at scroll-top then flash to bottom.
  useLayoutEffect(() => {
    if (!pendingInitialScroll.current || messages.length === 0) return;
    pendingInitialScroll.current = false;
    const container = messagesContainerRef.current;
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
    initialScrollDone.current = true;
    prevMsgCountRef.current = messages.length;
  }, [messages]);

  useEffect(() => {
    if (!initialScrollDone.current) return;
    if (messages.length !== prevMsgCountRef.current) {
      const isNewAppend = messages.length > prevMsgCountRef.current;
      prevMsgCountRef.current = messages.length;
      if (isNewAppend && !isLoadingMore && isNearBottomRef.current) {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
      }
    }
  }, [messages.length, isLoadingMore]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    if (!isNearBottomRef.current) return;
    messagesEndRef.current?.scrollIntoView({ behavior: 'auto' });
  }, [activeTurn, isStreaming]);

  useEffect(() => {
    if (optimisticMsg) {
      isNearBottomRef.current = true;
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [optimisticMsg]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    const container = messagesContainerRef.current;
    if (!container) return;
    const tid = setInterval(() => {
      if (!isNearBottomRef.current) return;
      const gap = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (gap > 10 && gap < 400) {
        container.scrollTo({ top: container.scrollHeight, behavior: 'smooth' });
      }
    }, 300);
    return () => clearInterval(tid);
  }, [isStreaming]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    const imgs = pendingImages;
    if ((!text && imgs.length === 0) || imageAnalyzing) return;
    const wasStreaming = isStreaming;
    setInput('');
    setPendingImages([]);
    if (!wasStreaming) {
      setInputExpanded(false);
    }

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
      await sessionMgr.sendMessage(
        sessionId,
        text || t('chat.imageAttachmentFallback'),
        agentMode,
        imageContexts,
      );
      pollerRef.current?.nudge();
      if (wasStreaming) {
        setInfoToast(t('chat.messageQueued'));
      }
    } catch (e: any) {
      setError(e.message);
    } finally {
      setImageAnalyzing(false);
      setOptimisticMsg(null);
    }
  }, [agentMode, imageAnalyzing, input, isStreaming, pendingImages, sessionId, sessionMgr, setError, t]);

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

  const isComposingRef = useRef(false);

  const handleCompositionStart = useCallback(() => {
    isComposingRef.current = true;
  }, []);

  const handleCompositionEnd = useCallback(() => {
    // Delay clearing to handle Safari's event ordering where
    // compositionend fires before the final keydown(Enter)
    setTimeout(() => {
      isComposingRef.current = false;
    }, 0);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      if ((e.nativeEvent as KeyboardEvent).isComposing || isComposingRef.current) {
        return;
      }
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
  const displayName = liveTitle || sessionName || t('chat.session');

  return (
    <div className="chat-page">
      {/* Header */}
      <div className="chat-page__header">
        <div className="chat-page__header-row">
          <button className="chat-page__back" onClick={onBack} aria-label={t('common.back')}>
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
            <button className="chat-page__theme-btn" onClick={toggleTheme} aria-label={t('common.toggleTheme')}>
              <ThemeToggleIcon isDark={isDark} />
            </button>
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="chat-page__messages" ref={messagesContainerRef} onScroll={handleScroll}>
        {isLoadingMore && (
          <div className="chat-page__load-more-indicator">{t('chat.loadingOlderMessages')}</div>
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
                    <span className="chat-msg__response-label">{t('chat.showResponse')}</span>
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
                    <span className="chat-msg__response-label">{t('chat.hideResponse')}</span>
                  </button>
                )}
                {hasItems ? (
                  renderOrderedItems(m.items!, now, undefined, handleAnswerQuestion, handleFileDownload, handleGetFileInfo)
                ) : (
                  <>
                    {m.thinking && <ThinkingBlock thinking={m.thinking} />}
                    {m.tools && m.tools.length > 0 && <ToolList tools={m.tools} now={now} />}
                    {m.content && (
                      <div className="chat-msg__assistant-content">
                        <MarkdownContent content={m.content} onFileDownload={handleFileDownload} onGetFileInfo={handleGetFileInfo} />
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
                  ? renderActiveTurnItems(turn.items, now, sessionMgr, setError, handleAnswerQuestion, handleFileDownload, handleGetFileInfo)
                  : renderOrderedItems(turn.items, now, undefined, undefined, handleFileDownload, handleGetFileInfo)}
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
            sessionMgr.cancelTool(toolId, t('common.cancel')).catch(err => { setError(String(err)); });
          };

          return (
            <div className="chat-msg chat-msg--assistant">
              {!hasRunningSubagent && (turn.thinking || turnIsActive) && (
                <ThinkingBlock
                  thinking={turn.thinking}
                  streaming={turnIsActive}
                  isLastItem={turnIsActive}
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
                  {turnIsActive
                    ? <TypewriterText content={turn.text} onFileDownload={handleFileDownload} onGetFileInfo={handleGetFileInfo} />
                    : <MarkdownContent content={turn.text} onFileDownload={handleFileDownload} onGetFileInfo={handleGetFileInfo} />}
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
                <span>{t('chat.analyzingImage')}</span>
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
          onClick={!inputExpanded ? expandInput : undefined}
        >
          {/* Input area */}
          <div className="chat-page__input-area">
            {inputExpanded ? (
              <textarea
                ref={inputRef}
                className="chat-page__input"
                placeholder={t('chat.inputPlaceholder')}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                onCompositionStart={handleCompositionStart}
                onCompositionEnd={handleCompositionEnd}
                rows={1}
                disabled={imageAnalyzing}
              />
            ) : (
              <span className="chat-page__input-placeholder">
                {imageAnalyzing
                  ? t('chat.imageAnalyzingPlaceholder')
                  : isStreaming
                    ? t('chat.collapsedStreamingPlaceholder')
                    : t('chat.collapsedInputPlaceholder')}
              </span>
            )}
          </div>

          {/* Actions bar */}
          <div className="chat-page__input-actions">
            <div className="chat-page__input-actions-left">
                {inputExpanded && (
                  <ModelSelectorPill
                    catalog={modelCatalog}
                    selectedModelId={selectedModelId}
                    disabled={imageAnalyzing || modelUpdating}
                    onSelect={handleSelectModel}
                  />
                )}
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
                    disabled={imageAnalyzing}
                  >
                    {modeOptions.find(m => m.id === agentMode)?.label}
                  </button>
                  <button
                    className="chat-page__action-btn"
                    onClick={handleImageSelect}
                    disabled={imageAnalyzing || pendingImages.length >= 5}
                    aria-label={t('common.attachImage')}
                  >
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect width="18" height="18" x="3" y="3" rx="2" ry="2"/>
                      <circle cx="9" cy="9" r="2"/>
                      <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>
                    </svg>
                  </button>
                </>
              )}
              {imageAnalyzing ? (
                <button className="chat-page__send-btn is-stop" aria-label={t('common.stop')} disabled>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ animation: 'analyzeSpin 2s linear infinite' }}>
                    <circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2"/>
                  </svg>
                </button>
              ) : isStreaming ? (
                <div className="chat-page__stream-actions">
                  <button
                    type="button"
                    className="chat-page__send-btn is-stop"
                    onClick={handleCancel}
                    aria-label={t('common.stop')}
                  >
                    <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
                      <rect x="3" y="3" width="10" height="10" rx="2" fill="currentColor"/>
                    </svg>
                  </button>
                  <button
                    type="button"
                    className="chat-page__send-btn"
                    onClick={inputExpanded ? handleSend : expandInput}
                    disabled={!input.trim() && pendingImages.length === 0}
                    aria-label={t('common.submit')}
                  >
                    <svg width="12" height="12" viewBox="0 0 20 20" fill="none">
                      <path d="M10 3L10 17M10 3L5 8M10 3L15 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                    </svg>
                  </button>
                </div>
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
      {infoToast && (
        <div className="chat-page__toast chat-page__toast--info" onClick={() => setInfoToast(null)}>
          {infoToast}
        </div>
      )}
    </div>
  );
};

export default ChatPage;
