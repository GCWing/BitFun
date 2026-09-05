import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { PrismLight as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus, vs } from 'react-syntax-highlighter/dist/esm/styles/prism';
import bash from 'react-syntax-highlighter/dist/esm/languages/prism/bash';
import c from 'react-syntax-highlighter/dist/esm/languages/prism/c';
import cpp from 'react-syntax-highlighter/dist/esm/languages/prism/cpp';
import csharp from 'react-syntax-highlighter/dist/esm/languages/prism/csharp';
import css from 'react-syntax-highlighter/dist/esm/languages/prism/css';
import diff from 'react-syntax-highlighter/dist/esm/languages/prism/diff';
import go from 'react-syntax-highlighter/dist/esm/languages/prism/go';
import java from 'react-syntax-highlighter/dist/esm/languages/prism/java';
import javascript from 'react-syntax-highlighter/dist/esm/languages/prism/javascript';
import json from 'react-syntax-highlighter/dist/esm/languages/prism/json';
import jsx from 'react-syntax-highlighter/dist/esm/languages/prism/jsx';
import kotlin from 'react-syntax-highlighter/dist/esm/languages/prism/kotlin';
import markdown from 'react-syntax-highlighter/dist/esm/languages/prism/markdown';
import markup from 'react-syntax-highlighter/dist/esm/languages/prism/markup';
import php from 'react-syntax-highlighter/dist/esm/languages/prism/php';
import python from 'react-syntax-highlighter/dist/esm/languages/prism/python';
import ruby from 'react-syntax-highlighter/dist/esm/languages/prism/ruby';
import rust from 'react-syntax-highlighter/dist/esm/languages/prism/rust';
import sql from 'react-syntax-highlighter/dist/esm/languages/prism/sql';
import swift from 'react-syntax-highlighter/dist/esm/languages/prism/swift';
import tsx from 'react-syntax-highlighter/dist/esm/languages/prism/tsx';
import typescript from 'react-syntax-highlighter/dist/esm/languages/prism/typescript';
import yaml from 'react-syntax-highlighter/dist/esm/languages/prism/yaml';
import { MobileButton, MobileIconButton, MobileLink } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import { useTheme } from '../theme';

const SYNTAX_LANGUAGES = { bash, c, cpp, csharp, css, diff, go, java, javascript, json, jsx, kotlin, markdown, markup, php, python, ruby, rust, sql, swift, tsx, typescript, yaml };
Object.entries(SYNTAX_LANGUAGES).forEach(([name, grammar]) => SyntaxHighlighter.registerLanguage(name, grammar));
SyntaxHighlighter.registerLanguage('cs', csharp);
SyntaxHighlighter.registerLanguage('html', markup);
SyntaxHighlighter.registerLanguage('js', javascript);
SyntaxHighlighter.registerLanguage('md', markdown);
SyntaxHighlighter.registerLanguage('py', python);
SyntaxHighlighter.registerLanguage('rb', ruby);
SyntaxHighlighter.registerLanguage('sh', bash);
SyntaxHighlighter.registerLanguage('ts', typescript);
SyntaxHighlighter.registerLanguage('xml', markup);
SyntaxHighlighter.registerLanguage('yml', yaml);

export function copyToClipboard(text: string): Promise<void> {
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
    <MobileIconButton
      appearance="plain"
      aria-label={copied ? 'Copied' : 'Copy code'}
      className={`copy-button${copied ? ' copy-success' : ''}`}
      icon={copied ? (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
      onClick={handleCopy}
      size="sm"
    />
  );
};
const COMPUTER_LINK_PREFIX = 'computer://';
const FILE_LINK_PREFIX = 'file://';
const WORKSPACE_FOLDER_PLACEHOLDER = '{{workspaceFolder}}';
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

  // Normalize URI-like Windows absolute paths with a leading slash before the drive letter.
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

function resolveFileReferenceHref(href: string): string | null {
  if (
    href.startsWith(COMPUTER_LINK_PREFIX) ||
    href.startsWith(FILE_LINK_PREFIX) ||
    href.startsWith('file:')
  ) {
    return normalizeFileLikeHref(href);
  }
  return isLocalFileLink(href);
}

interface ProjectedFileReference {
  path: string;
}

function projectFileReferences(content: string): ProjectedFileReference[] {
  const references: ProjectedFileReference[] = [];
  const seen = new Set<string>();
  const addReference = (href: string) => {
    const path = resolveFileReferenceHref(href);
    if (!path || seen.has(path)) return;
    seen.add(path);
    references.push({ path });
  };

  // Markdown attachment links stay readable inline; their richer cards are
  // projected into a separate block below the message, matching HarmonyOS.
  const markdownLinkPattern = /(?<!!)\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|([^\s)\n]+))(?:\s+["'][^"'\n]*["'])?\s*\)/g;
  for (const match of content.matchAll(markdownLinkPattern)) {
    addReference(match[1] || match[2] || '');
  }

  // Preserve support for assistant output that emits a bare computer/file URI.
  const bareReferencePattern = /(?:computer|file):\/\/[^\s<>()\]]+/g;
  for (const match of content.matchAll(bareReferencePattern)) {
    addReference(match[0].replace(/[.,;:!?，。；：！？]+$/, ''));
  }

  return references.slice(0, 4);
}

function formatFileSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

const FileTextIcon: React.FC<{ size?: number; className?: string }> = ({ size = 20, className }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={className}
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

  if (state.status === 'loading') {
    return (
      <span className="file-card" data-status="loading">
        <span className="file-card__icon"><FileTextIcon size={20} /></span>
        <span className="file-card__placeholder">{t('chat.fileLoading')}</span>
      </span>
    );
  }
  if (state.status === 'error') {
    return (
      <span className="file-card" data-status="error" title={state.message}>
        <span className="file-card__icon"><FileTextIcon size={20} /></span>
        <span className="file-card__placeholder">{t('chat.fileUnavailable')}</span>
      </span>
    );
  }

  const { name, size } = state as { name: string; size: number; mimeType: string; status: string };
  const isDownloading = state.status === 'downloading';
  const isDone = state.status === 'done';

  return (
    <MobileButton
      appearance="plain"
      className="file-card"
      data-status={state.status}
      onClick={handleClick}
      title={isDownloading ? t('chat.fileDownloading') : isDone ? t('chat.fileDownloaded') : t('chat.clickToDownload')}
    >
      <span className="file-card__icon"><FileTextIcon size={20} /></span>
      <span className="file-card__copy">
        <span className="file-card__name">{name}</span>
        <span className="file-card__meta">{formatFileSize(size)}</span>
      </span>
      <span className="file-card__action">
        {isDownloading ? `${Math.round((state as any).progress * 100)}%` : isDone ? '✓' : '↓'}
      </span>
    </MobileButton>
  );
};
interface MarkdownContentProps {
  content: string;
  onFileDownload?: (path: string, onProgress?: (downloaded: number, total: number) => void) => Promise<void>;
  onGetFileInfo?: (path: string) => Promise<{ name: string; size: number; mimeType: string }>;
}

export const MarkdownContent: React.FC<MarkdownContentProps> = ({ content, onFileDownload, onGetFileInfo }) => {
  const { isDark } = useTheme();
  const syntaxTheme = isDark ? vscDarkPlus : vs;
  const fileReferences = useMemo(
    () => onFileDownload && onGetFileInfo ? projectFileReferences(content) : [],
    [content, onFileDownload, onGetFileInfo],
  );

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
              fontSize: 'var(--openbitfun-type-code-sm-font-size)',
              lineHeight: 'var(--openbitfun-type-body-md-line-height)',
            }}
            codeTagProps={{
              style: {
                fontFamily: 'var(--openbitfun-type-code-md-font-family)',
              },
            }}
            lineNumberStyle={{
              color: 'var(--openbitfun-color-content-muted)',
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
      const filePath = typeof href === 'string' ? resolveFileReferenceHref(href) : null;
      if (filePath && onFileDownload) {
        return (
          <MobileButton
            appearance="plain"
            className="file-link"
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); onFileDownload(filePath); }}
            type="button"
          >
            {children}
          </MobileButton>
        );
      }

      // Fallback: render as plain text for computer:// links without handler,
      // or as a regular link for http(s) links.
      if (typeof href === 'string') {
        // Open all external links in a new tab.
        const isExternalLink = href.startsWith('http://') || href.startsWith('https://');
        if (isExternalLink) {
          return (
            <MobileLink
              href={href}
              target="_blank"
              rel="noopener noreferrer"
            >
              {children}
            </MobileLink>
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
    <>
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
      {fileReferences.length > 0 && onGetFileInfo && onFileDownload && (
        <div className="message-file-cards">
          {fileReferences.map((reference) => (
            <FileCard
              key={reference.path}
              path={reference.path}
              onGetFileInfo={onGetFileInfo}
              onDownload={onFileDownload}
            />
          ))}
        </div>
      )}
    </>
  );
};
