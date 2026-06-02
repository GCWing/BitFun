/**
 * Markdown component
 * Used to render Markdown-formatted text
 */

import React, { useState, useMemo, useCallback, useEffect, useLayoutEffect, Component, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';
import { visit } from 'unist-util-visit';
import { useI18n } from '@/infrastructure/i18n';
import { MermaidBlock } from './MermaidBlock';
import { ReproductionStepsBlock } from './ReproductionStepsBlock';
import { AsyncPrismSyntaxHighlighter } from './AsyncPrismSyntaxHighlighter';
import { buildMarkdownPrismStyle } from './markdownPrismTheme';
import { Tooltip } from '../Tooltip';
import { globalAPI, systemAPI, workspaceAPI } from '../../../infrastructure/api';
import { getPrismLanguageFromAlias } from '@/infrastructure/language-detection';
import { useTheme } from '@/infrastructure/theme';
import { contextMenuController } from '@/shared/context-menu-system/core/ContextMenuController';
import { ContextType, type CustomContext, type MenuItem } from '@/shared/context-menu-system/types';
import { createLogger } from '@/shared/utils/logger';
import {
  isStartupRenderTraceEnabled,
  recordReactRenderProfile,
  startupTrace,
} from '@/shared/utils/startupTrace';
import path from 'path-browserify';
import 'katex/dist/katex.min.css';
import './Markdown.scss';

const log = createLogger('Markdown');
const COMPUTER_LINK_PREFIX = 'computer://';

// Module-level cache so that all simultaneously-mounting Markdown instances
// (e.g. dozens of history blocks after a workspace switch) share a single
// IPC round-trip for the workspace path. The in-flight deduplication in
// GlobalAPI already coalesces concurrent calls into one; this cache avoids
// even triggering a new IPC call while the result is still fresh.
let _cachedWorkspacePathResult: string | undefined;
let _cachedWorkspacePathAt = 0;
const WORKSPACE_PATH_CACHE_MS = 5000;

export interface MarkdownTraceContext {
  turnId?: string;
  roundId?: string;
  itemId?: string;
}

interface MarkdownRenderTraceProps {
  startedAtMs: number;
  contentLength: number;
  hasCodeBlock: boolean;
  hasTable: boolean;
  isStreaming: boolean;
  traceContext?: MarkdownTraceContext;
}

const MarkdownRenderTrace: React.FC<MarkdownRenderTraceProps> = ({
  startedAtMs,
  contentLength,
  hasCodeBlock,
  hasTable,
  isStreaming,
  traceContext,
}) => {
  useLayoutEffect(() => {
    recordReactRenderProfile(startupTrace, {
      component: 'MarkdownRenderer',
      phase: 'commit',
      actualDurationMs: performance.now() - startedAtMs,
      contentLength,
      turnId: traceContext?.turnId,
      roundId: traceContext?.roundId,
      itemId: traceContext?.itemId,
      hasCodeBlock,
      hasTable,
      isStreaming,
    });
  });

  return null;
};

async function getWorkspacePathCached(): Promise<string | undefined> {
  const now = Date.now();
  if (_cachedWorkspacePathResult !== undefined && now - _cachedWorkspacePathAt < WORKSPACE_PATH_CACHE_MS) {
    return _cachedWorkspacePathResult;
  }
  const result = await globalAPI.getCurrentWorkspacePath();
  _cachedWorkspacePathResult = result;
  _cachedWorkspacePathAt = Date.now();
  return result;
}

/** Catches render errors from react-markdown/remark-gfm (e.g. RegExp in transformGfmAutolinkLiterals) and shows plain text fallback. */
class MarkdownErrorBoundary extends Component<
  { children: ReactNode; fallbackContent: string },
  { hasError: boolean }
> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: Error) {
    log.error('Markdown render error, showing plain text fallback', { message: error.message });
  }

  componentDidUpdate(prevProps: { fallbackContent: string }) {
    if (prevProps.fallbackContent !== this.props.fallbackContent && this.state.hasError) {
      this.setState({ hasError: false });
    }
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="markdown-renderer markdown-renderer--fallback" style={{ whiteSpace: 'pre-wrap' }}>
          {this.props.fallbackContent}
        </div>
      );
    }
    return this.props.children;
  }
}
const FILE_LINK_PREFIX = 'file://';
const WORKSPACE_FOLDER_PLACEHOLDER = '{{workspaceFolder}}';
const LOCAL_IMAGE_PLACEHOLDER =
  'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
const EDITOR_OPENABLE_EXTENSIONS = new Set([
  'js', 'jsx', 'ts', 'tsx', 'mjs', 'cjs', 'mts', 'cts',
  'py', 'pyw', 'pyi',
  'rs', 'go', 'java', 'kt', 'kts', 'scala', 'groovy',
  'c', 'cpp', 'cc', 'cxx', 'h', 'hpp', 'hxx', 'hh',
  'cs', 'rb', 'php', 'swift', 'dart', 'lua', 'r', 'jl',
  'vue', 'svelte',
  'html', 'htm', 'css', 'scss', 'less', 'sass',
  'json', 'jsonc', 'yaml', 'yml', 'toml', 'xml',
  'md', 'mdx', 'rst', 'txt', 'csv', 'tsv',
  'sh', 'bash', 'zsh', 'fish', 'ps1', 'bat', 'cmd',
  'sql', 'graphql', 'gql', 'proto',
  'ini', 'cfg', 'conf', 'env', 'lock',
  'gitignore', 'gitattributes', 'editorconfig',
  'log', 'dockerfile', 'makefile', 'mk', 'gradle',
  'properties', 'plist', 'tex', 'mermaid', 'svg',
]);
const EDITOR_OPENABLE_BASENAMES = new Set([
  'dockerfile',
  'makefile',
  'cmakelists.txt',
  '.gitignore',
  '.gitattributes',
  '.editorconfig',
  '.npmrc',
  '.nvmrc',
  '.prettierrc',
  '.prettierignore',
  '.eslintrc',
  '.eslintignore',
  '.stylelintrc',
  '.stylelintignore',
  '.babelrc',
  '.env',
  '.env.local',
  '.env.development',
  '.env.production',
  '.env.test',
  'gemfile',
  'rakefile',
  'podfile',
  'brewfile',
  'justfile',
  'procfile',
  'license',
  'readme',
  'readme.md',
  'readme.txt',
]);

const localImageDataUrlCache = new Map<string, string>();
const localImageRequestCache = new Map<string, Promise<string>>();

const sanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames || []), 'details', 'summary'],
  attributes: {
    ...defaultSchema.attributes,
    a: [...(defaultSchema.attributes?.a || []), 'href', 'title'],
    code: [...(defaultSchema.attributes?.code || []), 'className'],
    div: [...(defaultSchema.attributes?.div || []), 'align'],
    details: [...(defaultSchema.attributes?.details || []), 'open'],
    img: [...(defaultSchema.attributes?.img || []), 'src', 'alt', 'title', 'width', 'height', 'align'],
    input: [...(defaultSchema.attributes?.input || []), 'type', 'checked', 'disabled'],
    p: [...(defaultSchema.attributes?.p || []), 'align'],
    pre: [...(defaultSchema.attributes?.pre || []), 'className'],
    summary: [...(defaultSchema.attributes?.summary || [])],
  },
  protocols: {
    ...defaultSchema.protocols,
    href: [...(defaultSchema.protocols?.href || []), 'computer', 'file', 'tab', 'visualization'],
    src: [...(defaultSchema.protocols?.src || []), 'asset', 'data', 'http', 'https', 'tauri'],
  },
};

function remarkAutolinkComputerFileLinks() {
  return (tree: any) => {
    visit(tree, 'text', (node: any, index: number | undefined, parent: any) => {
      if (index === undefined || !parent || !Array.isArray(parent.children)) {
        return;
      }

      if (parent.type === 'link' || parent.type === 'linkReference') {
        return;
      }

      const value = node.value;
      if (typeof value !== 'string' || (!value.includes(COMPUTER_LINK_PREFIX) && !value.includes(FILE_LINK_PREFIX))) {
        return;
      }

      const re = /(computer:\/\/|file:\/\/)[^\s<>()]+/g;
      let match: RegExpExecArray | null;
      let lastIndex = 0;
      const nextChildren: any[] = [];

      while ((match = re.exec(value)) !== null) {
        const start = match.index;
        const end = start + match[0].length;
        const url = match[0];

        if (start > lastIndex) {
          nextChildren.push({
            type: 'text',
            value: value.slice(lastIndex, start)
          });
        }

        nextChildren.push({
          type: 'link',
          url,
          title: null,
          children: [{ type: 'text', value: url }]
        });

        lastIndex = end;
      }

      if (nextChildren.length === 0) {
        return;
      }

      if (lastIndex < value.length) {
        nextChildren.push({
          type: 'text',
          value: value.slice(lastIndex)
        });
      }

      parent.children.splice(index, 1, ...nextChildren);
      return index + nextChildren.length;
    });
  };
}

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

  // Normalize URI-style Windows drive paths to native absolute paths.
  if (/^\/[A-Za-z]:[\\/]/.test(filePath)) {
    filePath = filePath.slice(1);
  }

  try {
    return decodeURIComponent(filePath);
  } catch {
    return filePath;
  }
}

function normalizePath(filePath: string): string {
  return filePath.replace(/\\/g, '/');
}

function normalizeDisplayPath(filePath: string): string {
  const normalized = normalizePath(filePath);

  if (/^[A-Za-z]:\//.test(normalized)) {
    return normalized.replace(/\//g, '\\');
  }

  if (/^\/[A-Za-z]:\//.test(normalized)) {
    return normalized.slice(1).replace(/\//g, '\\');
  }

  return normalized;
}

function isAbsoluteFilesystemPath(filePath: string): boolean {
  const normalized = normalizePath(filePath);
  if (/^[A-Za-z]:/.test(normalized) || /^\/[A-Za-z]:/.test(normalized)) {
    return true;
  }

  return normalized.startsWith('/') && !normalized.startsWith('//');
}

function resolveBaseRelativePath(targetPath: string, basePath?: string): string {
  if (!targetPath || !basePath || isAbsoluteFilesystemPath(targetPath)) {
    return targetPath;
  }

  const normalizedTarget = normalizePath(targetPath);
  if (normalizedTarget.startsWith('./') || normalizedTarget.startsWith('../')) {
    return path.normalize(path.join(basePath, normalizedTarget));
  }

  return path.normalize(path.join(basePath, normalizedTarget));
}

function resolveDisplayFilePath(targetPath: string, basePath?: string, workspacePath?: string): string {
  const baseResolved = resolveBaseRelativePath(targetPath, basePath);

  if (!baseResolved || isAbsoluteFilesystemPath(baseResolved) || !workspacePath) {
    return normalizeDisplayPath(baseResolved);
  }

  return normalizeDisplayPath(resolveBaseRelativePath(baseResolved, workspacePath));
}

function isLocalAssetPath(src: string): boolean {
  if (!src) {
    return false;
  }

  return !/^(https?:|data:|asset:|tauri:)/i.test(src);
}

function normalizeExternalImageSrc(src: string): string {
  const githubBlobMatch = src.match(
    /^https:\/\/github\.com\/([^/]+)\/([^/]+)\/blob\/([^/]+)\/(.+)$/i,
  );

  if (githubBlobMatch) {
    const [, owner, repo, ref, assetPath] = githubBlobMatch;
    return `https://raw.githubusercontent.com/${owner}/${repo}/${ref}/${assetPath}`;
  }

  return src;
}

function getMimeType(filePath: string): string {
  const ext = filePath.toLowerCase().split('.').pop();
  const mimeTypes: Record<string, string> = {
    avif: 'image/avif',
    bmp: 'image/bmp',
    gif: 'image/gif',
    ico: 'image/x-icon',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    png: 'image/png',
    svg: 'image/svg+xml',
    webp: 'image/webp',
  };

  return mimeTypes[ext || ''] || 'image/jpeg';
}

async function getLocalImageDataUrl(localPath: string): Promise<string> {
  const cachedDataUrl = localImageDataUrlCache.get(localPath);
  if (cachedDataUrl) {
    return cachedDataUrl;
  }

  const pendingRequest = localImageRequestCache.get(localPath);
  if (pendingRequest) {
    return pendingRequest;
  }

  const request = (async () => {
    const base64Content = await workspaceAPI.readFileContent(localPath);
    const dataUrl = `data:${getMimeType(localPath)};base64,${base64Content}`;
    localImageDataUrlCache.set(localPath, dataUrl);
    localImageRequestCache.delete(localPath);
    return dataUrl;
  })().catch((error) => {
    localImageRequestCache.delete(localPath);
    throw error;
  });

  localImageRequestCache.set(localPath, request);
  return request;
}

interface MarkdownImageProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  basePath?: string;
}

const MarkdownImage: React.FC<MarkdownImageProps> = ({ src, alt, className, basePath, ...imgProps }) => {
  const rawSrc = typeof src === 'string' ? normalizeExternalImageSrc(src) : '';
  const localPath = useMemo(() => {
    if (!rawSrc || !isLocalAssetPath(rawSrc)) {
      return null;
    }

    return resolveBaseRelativePath(rawSrc, basePath);
  }, [basePath, rawSrc]);
  const [resolvedSrc, setResolvedSrc] = useState(() => {
    if (!localPath) {
      return rawSrc;
    }

    return localImageDataUrlCache.get(localPath) || LOCAL_IMAGE_PLACEHOLDER;
  });
  const [loadState, setLoadState] = useState<'idle' | 'loading' | 'loaded' | 'error'>(() => {
    if (!localPath) {
      return 'loaded';
    }

    return localImageDataUrlCache.has(localPath) ? 'loaded' : 'idle';
  });

  useEffect(() => {
    if (!localPath) {
      setResolvedSrc(rawSrc);
      setLoadState('loaded');
      return;
    }

    const cachedDataUrl = localImageDataUrlCache.get(localPath);
    if (cachedDataUrl) {
      setResolvedSrc(cachedDataUrl);
      setLoadState('loaded');
      return;
    }

    let cancelled = false;
    setResolvedSrc(LOCAL_IMAGE_PLACEHOLDER);
    setLoadState('loading');

    void getLocalImageDataUrl(localPath)
      .then((dataUrl) => {
        if (cancelled) {
          return;
        }

        setResolvedSrc(dataUrl);
        setLoadState('loaded');
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }

        log.error('Failed to load local markdown image', { path: localPath, error });
        setResolvedSrc(rawSrc);
        setLoadState('error');
      });

    return () => {
      cancelled = true;
    };
  }, [localPath, rawSrc]);

  return (
    <img
      {...imgProps}
      alt={alt}
      className={[
        className,
        loadState === 'loading' ? 'markdown-image markdown-image--loading' : '',
        loadState === 'error' ? 'markdown-image markdown-image--error' : '',
      ].filter(Boolean).join(' ')}
      loading="lazy"
      src={resolvedSrc}
    />
  );
};

function isEditorOpenableFilePath(filePath: string): boolean {
  const normalizedPath = filePath.trim().replace(/[?#].*$/, '');
  const fileName = normalizedPath.split(/[\\/]/).pop()?.toLowerCase() || '';

  if (!fileName) {
    return false;
  }

  if (EDITOR_OPENABLE_BASENAMES.has(fileName)) {
    return true;
  }

  const dotIdx = fileName.lastIndexOf('.');
  if (dotIdx <= 0) {
    return false;
  }

  return EDITOR_OPENABLE_EXTENSIONS.has(fileName.slice(dotIdx + 1));
}

/** Human-readable label for Prism language ids (code block toolbar). */
function formatCodeLanguageLabel(lang: string): string {
  if (!lang) return 'Text';
  const key = lang.toLowerCase();
  const aliases: Record<string, string> = {
    js: 'JavaScript',
    jsx: 'JavaScript',
    mjs: 'JavaScript',
    cjs: 'JavaScript',
    ts: 'TypeScript',
    tsx: 'TSX',
    py: 'Python',
    rs: 'Rust',
    go: 'Go',
    rb: 'Ruby',
    sh: 'Shell',
    bash: 'Bash',
    zsh: 'Zsh',
    fish: 'Fish',
    md: 'Markdown',
    yml: 'YAML',
    yaml: 'YAML',
    json: 'JSON',
    html: 'HTML',
    css: 'CSS',
    scss: 'SCSS',
    sass: 'Sass',
    less: 'Less',
    cpp: 'C++',
    cxx: 'C++',
    hpp: 'C++',
    hxx: 'C++',
    cc: 'C++',
    c: 'C',
    cs: 'C#',
    fs: 'F#',
    swift: 'Swift',
    kt: 'Kotlin',
    java: 'Java',
    sql: 'SQL',
    graphql: 'GraphQL',
    dockerfile: 'Dockerfile',
    makefile: 'Makefile',
    toml: 'TOML',
    xml: 'XML',
    rust: 'Rust',
    typescript: 'TypeScript',
    javascript: 'JavaScript',
  };
  if (aliases[key]) return aliases[key];
  const raw = lang.replace(/[_-]/g, ' ');
  return raw.charAt(0).toUpperCase() + raw.slice(1).toLowerCase();
}

export interface FlowCodeBlockFallbackProps {
  code: string;
  language: string;
  bodyStyle: React.CSSProperties;
  codeTagStyle: React.CSSProperties;
  gutterColor: string;
}

/**
 * Lightweight, stable line-numbered code renderer used while the surrounding
 * markdown is still streaming. Its layout deliberately matches the
 * `react-syntax-highlighter` `showLineNumbers` output: a fixed-width inline
 * line-number column followed by the line content, separated visually by the
 * same padding. This keeps the code block from visibly jumping when streaming
 * completes and the heavy Prism highlighter takes over.
 */
const CodeBlockFallback: React.FC<FlowCodeBlockFallbackProps> = ({
  code,
  language,
  bodyStyle,
  codeTagStyle,
  gutterColor,
}) => {
  const lineCount = code.length === 0 ? 1 : code.split('\n').length;
  let gutterText = '';
  for (let i = 1; i <= lineCount; i++) {
    gutterText += i === lineCount ? `${i}` : `${i}\n`;
  }

  return (
    <pre
      className={`language-${language} code-block-fallback code-block-fallback--linenumbers`}
      style={bodyStyle}
    >
      <code style={{ ...codeTagStyle, display: 'flex' }}>
        <span
          aria-hidden="true"
          style={{
            flex: 'none',
            display: 'block',
            minWidth: '3em',
            paddingRight: '1em',
            textAlign: 'right',
            color: gutterColor,
            userSelect: 'none',
            whiteSpace: 'pre',
          }}
        >
          {gutterText}
        </span>
        <span
          style={{
            flex: 1,
            minWidth: 0,
            display: 'block',
            whiteSpace: 'pre',
          }}
        >
          {code}
        </span>
      </code>
    </pre>
  );
};

const CopyButton: React.FC<{ code: string }> = ({ code }) => {
  const { t } = useI18n('components');
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      log.warn('Failed to copy code', { error });
    }
  };

  return (
    <button 
      className={`copy-button${copied ? ' copy-success' : ''}`}
      onClick={handleCopy}
      title={copied ? t('markdown.copySuccess') : t('markdown.copyCode')}
    >
      {copied ? (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12"></polyline>
        </svg>
      ) : (
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
        </svg>
      )}
    </button>
  );
};

export interface LineRange {
  start: number;
  end?: number;
}

export interface MarkdownProps {
  content: string;
  basePath?: string;
  className?: string;
  isStreaming?: boolean;
  expandDetailsByDefault?: boolean;
  onOpenVisualization?: (visualization: any) => void;
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any) => void;
  onHttpLinkClick?: (url: string, event: React.MouseEvent<HTMLAnchorElement>) => boolean | void;
  onReproductionProceed?: () => void;
  traceContext?: MarkdownTraceContext;
}

export const Markdown = React.memo<MarkdownProps>(({ 
  content, 
  basePath,
  className = '',
  isStreaming = false,
  expandDetailsByDefault = false,
  onOpenVisualization,
  onFileViewRequest,
  onTabOpen,
  onHttpLinkClick,
  onReproductionProceed,
  traceContext,
}) => {
  const { isLight } = useTheme();
  const { t } = useI18n('components');
  const [currentWorkspacePath, setCurrentWorkspacePath] = useState('');
  
  const syntaxTheme = useMemo(() => buildMarkdownPrismStyle(isLight), [isLight]);
  
  const contentStr = typeof content === 'string' ? content : String(content || '');
  const renderTraceEnabled = isStartupRenderTraceEnabled();
  const renderTraceStartedAtMs = renderTraceEnabled ? performance.now() : null;

  useEffect(() => {
    let cancelled = false;

    void getWorkspacePathCached()
      .then((workspacePath) => {
        if (!cancelled && workspacePath) {
          setCurrentWorkspacePath(workspacePath);
        }
      })
      .catch((error) => {
        log.warn('Failed to resolve workspace path for markdown links', { error });
      });

    return () => {
      cancelled = true;
    };
  }, []);
  
  // Fault-tolerant extraction of <reproduction_steps> content
  const { markdownContent, reproductionSteps } = useMemo(() => {
    const regex = /<reproduction_steps>([\s\S]*?)<\/reproduction[\s_]*steps\s*>?/g;
    const match = regex.exec(contentStr);

    let body = contentStr;
    let steps: string | null = null;
    if (match) {
      steps = match[1].trim();
      body = contentStr.replace(regex, '').trim();
    }

    // While streaming, the model may emit an opening ```lang fence long before
    // the closing ```. react-markdown then flips between parsing the tail as a
    // paragraph (raw text) and as a fenced code block as more tokens arrive,
    // which unmounts/remounts the code block and shifts its position every
    // tick. Append a synthetic closing fence so the AST stays a stable code
    // block from the moment the opening fence appears.
    if (isStreaming) {
      const fenceMatches = body.match(/^[ \t]{0,3}(`{3,}|~{3,})/gm);
      if (fenceMatches && fenceMatches.length % 2 === 1) {
        const lastFence = fenceMatches[fenceMatches.length - 1].trim();
        const needsLeadingNewline = !body.endsWith('\n');
        body = `${body}${needsLeadingNewline ? '\n' : ''}${lastFence}`;
      }
    }

    return { markdownContent: body, reproductionSteps: steps };
  }, [contentStr, isStreaming]);

  const linkMap = useMemo(() => {
    const map = new Map<string, string>();
    const linkMatches = contentStr.match(/\[([^\]]+)\]\(([^)]+)\)/g) || [];
    
    linkMatches.forEach(match => {
      const linkMatch = match.match(/\[([^\]]+)\]\(([^)]+)\)/);
      if (linkMatch) {
        const [, text, href] = linkMatch;
        map.set(text, href);
      }
    });
    return map;
  }, [contentStr]);

  const markdownFeatureProfile = useMemo(() => ({
    contentLength: markdownContent.length,
    hasCodeBlock: /^[ \t]{0,3}(`{3,}|~{3,})/m.test(markdownContent),
    hasTable: /^[ \t]*\|.+\|[ \t]*$/m.test(markdownContent),
  }), [markdownContent]);

  // Parse line ranges like #L42 / 1-20
  const parseLineRange = useCallback((hash: string): LineRange | undefined => {
    const cleanHash = hash.replace(/^#/, '');

    const lineMatchWithL = cleanHash.match(/^L(\d+)(?:-L?(\d+))?$/i);
    if (lineMatchWithL) {
      const start = parseInt(lineMatchWithL[1], 10);
      const end = lineMatchWithL[2] ? parseInt(lineMatchWithL[2], 10) : undefined;
      return { start, end };
    }

    const lineMatchWithoutL = cleanHash.match(/^(\d+)(?:-(\d+))?$/);
    if (lineMatchWithoutL) {
      const start = parseInt(lineMatchWithoutL[1], 10);
      const end = lineMatchWithoutL[2] ? parseInt(lineMatchWithoutL[2], 10) : undefined;
      return { start, end };
    }

    return undefined;
  }, []);

  const handleFileViewRequest = useCallback((filePath: string, fileName: string, lineRange?: LineRange) => {
    onFileViewRequest?.(filePath, fileName, lineRange);
  }, [onFileViewRequest]);

  const handleOpenVisualization = useCallback((visualization: any) => {
    onOpenVisualization?.(visualization);
  }, [onOpenVisualization]);

  const handleTabOpen = useCallback((tabInfo: any) => {
    onTabOpen?.(tabInfo);
  }, [onTabOpen]);

  const handleRevealInExplorer = useCallback(async (filePath: string) => {
    let targetPath = resolveDisplayFilePath(filePath, basePath, currentWorkspacePath);
    try {
      if (!isAbsoluteFilesystemPath(targetPath)) {
        const workspacePath = await globalAPI.getCurrentWorkspacePath();
        targetPath = resolveDisplayFilePath(filePath, basePath, workspacePath || currentWorkspacePath);
      }

      await workspaceAPI.revealInExplorer(targetPath);
    } catch (error) {
      log.error('Failed to reveal file in explorer', { filePath: targetPath, error });
    }
  }, [basePath, currentWorkspacePath]);

  const showLinkContextMenu = useCallback((
    event: React.MouseEvent<HTMLElement>,
    items: MenuItem[],
    customType: string,
    data: Record<string, unknown>
  ) => {
    event.preventDefault();
    event.stopPropagation();
    event.nativeEvent.stopImmediatePropagation?.();

    const position = { x: event.clientX, y: event.clientY };
    const context: CustomContext = {
      type: ContextType.CUSTOM,
      customType,
      data,
      event: event.nativeEvent,
      targetElement: event.currentTarget,
      position,
      timestamp: Date.now(),
    };

    void contextMenuController.show(position, items, context);
  }, []);

  const canOpenInBuiltInBrowser = useCallback((targetElement: HTMLElement | null): boolean => {
    if (typeof window === 'undefined' || !targetElement) {
      return false;
    }

    return Boolean(
      targetElement.closest('.bitfun-session-scene') &&
      targetElement.closest('.modern-flowchat-container, .flow-chat-container')
    );
  }, []);

  const handleCopyLink = useCallback(async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
    } catch (error) {
      log.warn('Failed to copy markdown link', { url, error });
    }
  }, []);

  const handleOpenExternalLink = useCallback(async (url: string) => {
    try {
      await systemAPI.openExternal(url);
    } catch (error) {
      log.error('Failed to open external URL', { url, error });
    }
  }, []);

  const handleOpenBuiltInBrowserLink = useCallback((url: string) => {
    if (typeof window === 'undefined') {
      return;
    }

    window.dispatchEvent(new CustomEvent('agent-create-tab', {
      detail: {
        type: 'browser',
        title: t('markdown.openInBuiltInBrowser'),
        data: { url },
        metadata: {
          duplicateCheckKey: `browser-panel:${url}`,
        },
        checkDuplicate: true,
        duplicateCheckKey: `browser-panel:${url}`,
        replaceExisting: false,
      },
    }));
  }, [t]);

  const handleLocalFileContextMenu = useCallback((
    event: React.MouseEvent<HTMLElement>,
    filePath: string,
    displayPath: string
  ) => {
    const items: MenuItem[] = [
      {
        id: 'markdown-open-in-explorer',
        label: t('markdown.openInExplorer'),
        icon: 'FolderOpen',
        onClick: () => handleRevealInExplorer(displayPath || filePath),
      },
      {
        id: 'markdown-copy-file-path',
        label: t('markdown.copyFilePath'),
        icon: 'Copy',
        onClick: () => void handleCopyLink(displayPath || filePath),
      },
    ];

    showLinkContextMenu(event, items, 'markdown-local-file-link', {
      filePath,
      displayPath,
    });
  }, [handleRevealInExplorer, handleCopyLink, showLinkContextMenu, t]);

  const handleWebLinkContextMenu = useCallback((event: React.MouseEvent<HTMLElement>, url: string) => {
    const targetElement = event.currentTarget;
    const items: MenuItem[] = [
      {
        id: 'markdown-open-in-browser',
        label: t('markdown.openInBrowser'),
        icon: 'ExternalLink',
        onClick: () => void handleOpenExternalLink(url),
      },
      {
        id: 'markdown-copy-link',
        label: t('markdown.copyLink'),
        icon: 'Copy',
        onClick: () => void handleCopyLink(url),
      },
    ];

    if (canOpenInBuiltInBrowser(targetElement)) {
      items.splice(1, 0, {
        id: 'markdown-open-in-built-in-browser',
        label: t('markdown.openInBuiltInBrowser'),
        icon: 'PanelRightOpen',
        onClick: () => handleOpenBuiltInBrowserLink(url),
      });
    }

    showLinkContextMenu(event, items, 'markdown-web-link', { url });
  }, [
    canOpenInBuiltInBrowser,
    handleCopyLink,
    handleOpenBuiltInBrowserLink,
    handleOpenExternalLink,
    showLinkContextMenu,
    t,
  ]);
  
  const components = useMemo(() => ({
    code({ node: _node, className, children, ...props }: any) {
      const match = /language-(\w+)/.exec(className || '');
      const language = match ? match[1] : '';
      const code = String(children).replace(/\n$/, '');
      
      const hasMultipleLines = code.includes('\n');
      const isCodeBlock = className?.startsWith('language-') || hasMultipleLines;
      
      if (!isCodeBlock) {
        return (
          <code className="inline-code" {...props}>
            {children}
          </code>
        );
      }
      
      if (language.toLowerCase().startsWith('mermaid')) {
        return (
          <MermaidBlock
            code={code}
            isStreaming={isStreaming}
          />
        );
      }
      
      const normalizedLang = getPrismLanguageFromAlias(language);
      const codeBodyStyle: React.CSSProperties = {
        margin: 0,
        borderRadius: '0 0 8px 8px',
        fontSize: '0.875rem',
        lineHeight: '1.55',
      };
      const codeTagStyle: React.CSSProperties = {
        fontFamily: 'var(--markdown-font-mono, "Fira Code", "JetBrains Mono", Consolas, "Courier New", monospace)',
      };

      return (
        <div className={`code-block-wrapper${hasMultipleLines ? '' : ' code-block-wrapper--single-line'}`}>
          <div className="code-block-toolbar">
            <span className="code-block-lang">{formatCodeLanguageLabel(normalizedLang)}</span>
            <CopyButton code={code} />
          </div>
          <div className="code-block-body">
          {isStreaming ? (
            // While the text is still streaming, skip the heavy Prism
            // tokenization on every tick (it re-highlights the entire
            // code each frame, which is the main source of code-block
            // jitter in the chat). Render a lightweight, line-numbered
            // <pre> that matches Prism's `showLineNumbers` layout so the
            // gutter width and line indentation stay visually stable
            // across the eventual fallback -> Prism swap when streaming
            // completes.
            <CodeBlockFallback
              code={code}
              language={normalizedLang}
              bodyStyle={codeBodyStyle}
              codeTagStyle={codeTagStyle}
              gutterColor={isLight ? '#999' : '#666'}
            />
          ) : (
            <AsyncPrismSyntaxHighlighter
              language={normalizedLang}
              style={syntaxTheme}
              showLineNumbers={true}
              customStyle={codeBodyStyle}
              codeTagProps={{ style: codeTagStyle }}
              lineNumberStyle={{
                color: isLight ? '#999' : '#666',
                paddingRight: '1em',
                textAlign: 'right',
                userSelect: 'none',
                minWidth: '3em'
              }}
              fallback={CodeBlockFallback}
              fallbackProps={{
                code,
                language: normalizedLang,
                bodyStyle: codeBodyStyle,
                codeTagStyle,
                gutterColor: isLight ? '#999' : '#666',
              }}
              traceContext={traceContext}
            >
              {code}
            </AsyncPrismSyntaxHighlighter>
          )}
          </div>
        </div>
      );
    },
    
    a({ node, href, children, ...props }: any) {
      const linkText = typeof children === 'string' ? children : String(children);
      const originalHref = linkMap.get(linkText);
      const hrefValue = originalHref || href || node?.properties?.href;
      const isHashLink = typeof hrefValue === 'string' && hrefValue.startsWith('#');
      const isComputerLink = typeof hrefValue === 'string' && hrefValue.startsWith(COMPUTER_LINK_PREFIX);
      const isVisualizationLink = typeof hrefValue === 'string' && hrefValue.startsWith('visualization:');
      const isTabLink = typeof hrefValue === 'string' && hrefValue.startsWith('tab:');
      const isHttpLink = typeof hrefValue === 'string' &&
        (hrefValue.startsWith('http://') || hrefValue.startsWith('https://'));
      const isMailtoLink = typeof hrefValue === 'string' && hrefValue.startsWith('mailto:');

      if (typeof hrefValue === 'string' && !isVisualizationLink && !isTabLink && !isHttpLink && !isMailtoLink && !isHashLink) {
        let filePath = normalizeFileLikeHref(hrefValue);

        let lineRange: LineRange | undefined;

        const hashIndex = filePath.indexOf('#');
        if (hashIndex !== -1) {
          const hash = filePath.substring(hashIndex);
          filePath = filePath.substring(0, hashIndex);
          lineRange = parseLineRange(hash);
        } else {
          // Note: exclude Windows drive letters (e.g. C:)
          const colonMatch = filePath.match(/^(.+?):(\d+)(?:-(\d+))?$/);
          if (colonMatch) {
            const [, pathBeforeColon, startLine, endLine] = colonMatch;
            const isWindowsDrive = /^[A-Za-z]:$/.test(pathBeforeColon);

            if (!isWindowsDrive) {
              filePath = pathBeforeColon;
              lineRange = {
                start: parseInt(startLine, 10),
                end: endLine ? parseInt(endLine, 10) : undefined
              };
            }
          }
        }

        filePath = resolveBaseRelativePath(filePath, basePath);
        const displayFilePath = resolveDisplayFilePath(filePath, undefined, currentWorkspacePath);

        const fileName = filePath.split(/[\\/]/).pop() || filePath;

        const isFolder = filePath.endsWith('/');
        const shouldRevealInExplorer = isComputerLink || !isEditorOpenableFilePath(filePath);
        if (!isFolder) {
          const fileLinkButton = (
            <button
              className="file-link"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                if (shouldRevealInExplorer) {
                  void handleRevealInExplorer(displayFilePath || filePath);
                  return;
                }
                handleFileViewRequest(filePath, fileName, lineRange);
              }}
              onContextMenu={(e) => handleLocalFileContextMenu(e, filePath, displayFilePath)}
              type="button"
              style={{
                cursor: 'pointer',
                textDecoration: 'underline',
                background: 'none',
                border: 'none',
                font: 'inherit'
              }}
            >
              {children}
            </button>
          );

          return (
            <Tooltip
              content={<span className="markdown-link-path-tooltip">{displayFilePath || filePath}</span>}
              placement="top"
              delay={300}
            >
              {fileLinkButton}
            </Tooltip>
          );
        }
      }
      
      if (isVisualizationLink && typeof hrefValue === 'string') {
        const vizData = hrefValue.replace('visualization:', '');
        
        return (
          <button
            className="visualization-link"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              try {
                const visualization = JSON.parse(decodeURIComponent(vizData));
                handleOpenVisualization(visualization);
              } catch (error) {
                log.error('Failed to parse visualization data', { error });
              }
            }}
            type="button"
          >
            {children}
          </button>
        );
      }
      
      if (isTabLink && typeof hrefValue === 'string') {
        const tabData = hrefValue.replace('tab:', '');
        
        return (
          <button
            className="tab-link"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              try {
                const tabInfo = JSON.parse(decodeURIComponent(tabData));
                handleTabOpen(tabInfo);
              } catch (error) {
                log.error('Failed to parse tab data', { error });
              }
            }}
            type="button"
            style={{ 
              cursor: 'pointer',
              textDecoration: 'underline',
              background: 'none',
              border: 'none',
              font: 'inherit'
            }}
          >
            {children}
          </button>
        );
      }
      
      if (isHttpLink && typeof hrefValue === 'string') {
        return (
          <a 
            href={hrefValue} 
            {...props}
            onClick={async (e) => {
              e.preventDefault();
              e.stopPropagation();
              if (onHttpLinkClick?.(hrefValue, e)) {
                return;
              }
              try {
                await systemAPI.openExternal(hrefValue);
              } catch (error) {
                log.error('Failed to open external URL', { url: hrefValue, error });
              }
            }}
            onContextMenu={(e) => handleWebLinkContextMenu(e, hrefValue)}
            style={{ cursor: 'pointer', textDecoration: 'underline' }}
          >
            {children}
          </a>
        );
      }

      if (isMailtoLink && typeof hrefValue === 'string') {
        return (
          <a href={hrefValue} {...props}>
            {children}
          </a>
        );
      }
      
      return (
        <a 
          href={typeof hrefValue === 'string' ? hrefValue : undefined} 
          {...props}
          onClick={(e) => {
            e.preventDefault();
          }}
          style={{ cursor: 'pointer' }}
        >
          {children}
        </a>
      );
    },
    
    table({ children }: any) {
      return (
        <div className="table-wrapper">
          <table>{children}</table>
        </div>
      );
    },

    details({ children, open, ...props }: any) {
      return (
        <details {...props} open={open ?? expandDetailsByDefault}>
          {children}
        </details>
      );
    },

    img({ node: _node, ...props }: any) {
      return <MarkdownImage {...props} basePath={basePath} />;
    },
    
    blockquote({ children }: any) {
      return <blockquote className="custom-blockquote">{children}</blockquote>;
    },
    
    ul({ children, ...props }: any) {
      return <ul {...props}>{children}</ul>;
    },
    
    ol({ children, ...props }: any) {
      return <ol {...props}>{children}</ol>;
    },
    
    li({ children, ...props }: any) {
      return <li {...props}>{children}</li>;
    },
    
    p({ children, align, style, ...props }: any) {
      return (
        <p
          {...props}
          style={align ? { ...style, textAlign: align } : style}
        >
          {children}
        </p>
      );
    }
  }), [
    basePath,
    expandDetailsByDefault,
    isStreaming,
    linkMap,
    handleFileViewRequest,
    handleRevealInExplorer,
    handleLocalFileContextMenu,
    handleWebLinkContextMenu,
    handleOpenVisualization,
    handleTabOpen,
    onHttpLinkClick,
    parseLineRange,
    syntaxTheme,
    isLight,
    currentWorkspacePath,
    traceContext,
  ]);
  
  const wrapperClassName = `markdown-renderer ${className}`.trim();

  return (
    <div className={wrapperClassName}>
      {renderTraceEnabled && renderTraceStartedAtMs !== null && (
        <MarkdownRenderTrace
          startedAtMs={renderTraceStartedAtMs}
          contentLength={markdownFeatureProfile.contentLength}
          hasCodeBlock={markdownFeatureProfile.hasCodeBlock}
          hasTable={markdownFeatureProfile.hasTable}
          isStreaming={isStreaming}
          traceContext={traceContext}
        />
      )}
      <MarkdownErrorBoundary fallbackContent={markdownContent}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm, remarkMath, remarkAutolinkComputerFileLinks]}
          rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema], rehypeKatex]}
          components={components}
        >
          {markdownContent}
        </ReactMarkdown>
      </MarkdownErrorBoundary>
      
      {reproductionSteps && !isStreaming && (
        <ReproductionStepsBlock 
          steps={reproductionSteps}
          onProceed={onReproductionProceed}
        />
      )}
    </div>
  );
});
