/**
 * Code Editor Component
 * 
 * Monaco Editor-based editable code editor with file editing and saving support.
 * @module components/CodeEditor
 */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { AlertCircle } from 'lucide-react';
import * as monaco from 'monaco-editor';
import { monacoInitManager } from '../services/MonacoInitManager';
import { monacoModelManager } from '../services/MonacoModelManager';
import { activeEditTargetService, createMonacoEditTarget } from '../services/ActiveEditTargetService';
import { 
  forceRegisterTheme,
  BitFunDarkTheme,
  BitFunDarkThemeMetadata 
} from '../themes';
import { useMonacoLsp } from '@/tools/lsp/hooks/useMonacoLsp';
import { lspExtensionRegistry } from '@/tools/lsp/services/LspExtensionRegistry';
import { globalEventBus } from '@/infrastructure/event-bus';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { EditorConfig as EditorConfigType } from '@/infrastructure/config/types';
import { CubeLoading } from '@/component-library';
import { getMonacoLanguage } from '@/infrastructure/language-detection';
import { createLogger } from '@/shared/utils/logger';
import { isSamePath } from '@/shared/utils/pathUtils';
import {
  diskContentMatchesEditorForExternalSync,
  diskVersionFromMetadata,
  diskVersionsDiffer,
  editorSyncContentSha256Hex,
  type DiskFileVersion,
} from '../utils/diskFileVersion';
import { confirmDialog } from '@/component-library/components/ConfirmDialog/confirmService';
import {
  isFileMissingFromMetadata,
  isLikelyFileNotFoundError,
} from '@/shared/utils/fsErrorUtils';
import { useI18n } from '@/infrastructure/i18n';
import { EditorBreadcrumb } from './EditorBreadcrumb';
import { EditorStatusBar } from './EditorStatusBar';

const log = createLogger('CodeEditor');
import {
  GoToLinePopover,
  IndentPopover,
  EncodingPopover,
  LanguagePopover,
} from './StatusBarPopovers';
import type { AnchorRect } from './StatusBarPopovers';
import './CodeEditor.scss';

export interface CodeEditorProps {
  /** File path */
  filePath: string;
  /** Workspace path */
  workspacePath?: string;
  /** File name */
  fileName?: string;
  /** Programming language for syntax highlighting */
  language?: string;
  /** Read-only mode */
  readOnly?: boolean;
  /** Show line numbers */
  showLineNumbers?: boolean;
  /** Show minimap */
  showMinimap?: boolean;
  /** Editor theme */
  theme?: 'vs-dark' | 'vs-light' | 'hc-black';
  /** CSS class name */
  className?: string;
  /** Content change callback */
  onContentChange?: (content: string, hasChanges: boolean) => void;
  /** Save callback */
  onSave?: (content: string) => void;
  /** Enable LSP support */
  enableLsp?: boolean;
  /** Jump to line number (deprecated, use jumpToRange) */
  jumpToLine?: number;
  /** Jump to column (deprecated, use jumpToRange) */
  jumpToColumn?: number;
  /** Jump to line range (preferred, supports single or multi-line selection) */
  jumpToRange?: import('@/component-library/components/Markdown').LineRange;
  /** When false, disk sync polling is paused (e.g. background editor tab). */
  isActiveTab?: boolean;
  /** File path is not an existing file on disk (drives tab "deleted" label). */
  onFileMissingFromDiskChange?: (missing: boolean) => void;
}

const LARGE_FILE_SIZE_THRESHOLD_BYTES = 1 * 1024 * 1024; // 1MB
const LARGE_FILE_MAX_LINE_LENGTH = 20000;
const LARGE_FILE_RENDER_LINE_LIMIT = 10000;
const LARGE_FILE_MAX_TOKENIZATION_LINE_LENGTH = 2000;
const LARGE_FILE_EXPANSION_LABELS = ['show more', '显示更多', '展开更多'];

/** Poll disk metadata for open file; only while tab is active (see isActiveTab). */
const FILE_SYNC_POLL_INTERVAL_MS = 1000;

function hasVeryLongLine(content: string, maxLineLength: number): boolean {
  let currentLineLength = 0;
  for (let i = 0; i < content.length; i++) {
    const code = content.charCodeAt(i);
    if (code === 10 || code === 13) {
      currentLineLength = 0;
      continue;
    }
    currentLineLength++;
    if (currentLineLength >= maxLineLength) {
      return true;
    }
  }
  return false;
}

function isMacOSDesktop(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  const isTauri = '__TAURI__' in window;
  return isTauri && typeof navigator.platform === 'string' && navigator.platform.toUpperCase().includes('MAC');
}

const CodeEditor: React.FC<CodeEditorProps> = ({
  filePath: rawFilePath,
  workspacePath,
  fileName,
  language = 'plaintext',
  readOnly = false,
  showLineNumbers = true,
  showMinimap = true,
  className = '',
  onContentChange,
  onSave,
  enableLsp = true,
  jumpToLine,
  jumpToColumn,
  jumpToRange,
  isActiveTab = true,
  onFileMissingFromDiskChange,
}) => {
  // Decode URL-encoded paths (e.g. d%3A/path -> d:/path)
  const filePath = useMemo(() => {
    try {
      if (rawFilePath.includes('%')) {
        return decodeURIComponent(rawFilePath);
      }
    } catch (err) {
      log.warn('Failed to decode path', { rawFilePath, error: err });
    }
    return rawFilePath;
  }, [rawFilePath]);

  const { t } = useI18n('tools');
  
  const detectLanguageFromFileName = useCallback((fileName: string): string => {
    const detected = getMonacoLanguage(fileName);
    return detected !== 'plaintext' ? detected : (language || 'plaintext');
  }, [language]);

  const [content, setContent] = useState('');
  const [, setHasChanges] = useState(false);
  const [loading, setLoading] = useState(true);
  const [showLoadingOverlay, setShowLoadingOverlay] = useState(false);
  const loadingOverlayDelayRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const LOADING_OVERLAY_DELAY_MS = 80;
  useEffect(() => {
    if (loading) {
      const t = setTimeout(() => {
        loadingOverlayDelayRef.current = null;
        setShowLoadingOverlay(true);
      }, LOADING_OVERLAY_DELAY_MS);
      loadingOverlayDelayRef.current = t;
      return () => {
        if (loadingOverlayDelayRef.current) {
          clearTimeout(loadingOverlayDelayRef.current);
          loadingOverlayDelayRef.current = null;
        }
      };
    } else {
      if (loadingOverlayDelayRef.current) {
        clearTimeout(loadingOverlayDelayRef.current);
        loadingOverlayDelayRef.current = null;
      }
      setShowLoadingOverlay(false);
    }
  }, [loading]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [detectedLanguage, setDetectedLanguage] = useState(() => {
    return fileName ? detectLanguageFromFileName(fileName) : language;
  });
  const [lspReady, setLspReady] = useState(false);
  const [monacoReady, setMonacoReady] = useState(false);
  const [editorInstance, setEditorInstance] = useState<monaco.editor.IStandaloneCodeEditor | null>(null);
  const [editorConfig, setEditorConfig] = useState<Partial<EditorConfigType>>({
    font_size: 14,
    font_family: "'Fira Code', Consolas, 'Courier New', monospace",
    font_weight: 'normal',
    line_height: 1.5,
    tab_size: 2,
    insert_spaces: true,
    word_wrap: 'off',
    line_numbers: 'on',
    minimap: { enabled: showMinimap, side: 'right', size: 'proportional' }
  });
  const [_currentThemeId, setCurrentThemeId] = useState<string>(BitFunDarkThemeMetadata.id);
  const [cursorPosition, setCursorPosition] = useState({ line: 1, column: 1 });
  const [selection, setSelection] = useState({ chars: 0, lines: 0 });
  const [statusBarPopover, setStatusBarPopover] = useState<null | 'position' | 'indent' | 'encoding' | 'language'>(null);
  const [statusBarAnchorRect, setStatusBarAnchorRect] = useState<AnchorRect | null>(null);
  const [encoding, setEncoding] = useState<string>('UTF-8');
  const [largeFileMode, setLargeFileMode] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<monaco.editor.ITextModel | null>(null);
  const isUnmountedRef = useRef(false);
  const isCheckingFileRef = useRef(false);
  /** Last disk state known to match loaded/saved editor content (mtime + size; local + remote). */
  const diskVersionRef = useRef<DiskFileVersion | null>(null);
  const lastReportedMissingRef = useRef<boolean | undefined>(undefined);

  const reportFileMissingFromDisk = useCallback(
    (missing: boolean) => {
      if (!onFileMissingFromDiskChange) {
        return;
      }
      if (lastReportedMissingRef.current === missing) {
        return;
      }
      lastReportedMissingRef.current = missing;
      onFileMissingFromDiskChange(missing);
    },
    [onFileMissingFromDiskChange]
  );
  const contentChangeListenerRef = useRef<monaco.IDisposable | null>(null);
  const ctrlDecorationsRef = useRef<string[]>([]);
  const lastHoverWordRef = useRef<string | null>(null);
  const originalContentRef = useRef<string>('');
  const isLoadingContentRef = useRef(false);
  const savedVersionIdRef = useRef<number>(0);
  const hasChangesRef = useRef<boolean>(false);
  const lastJumpPositionRef = useRef<{ filePath: string; line: number; column: number; endLine?: number } | null>(null);
  const filePathRef = useRef<string>(filePath);
  const saveFileContentRef = useRef<() => Promise<void>>();
  const latestEditorConfigRef = useRef<Partial<EditorConfigType> | null>(null);
  const delayedFontApplyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const userLanguageOverrideRef = useRef(false);
  const userIndentRef = useRef<{ tab_size: number; insert_spaces: boolean } | null>(null);
  const largeFileModeRef = useRef(false);
  const largeFileExpansionBlockedLogRef = useRef(false);
  const pendingModelContentRef = useRef<string | null>(null);
  const macosEditorBindingCleanupRef = useRef<(() => void) | null>(null);

  const detectLargeFileMode = useCallback((nextContent: string, fileSizeBytes?: number): boolean => {
    const size = typeof fileSizeBytes === 'number' && fileSizeBytes >= 0
      ? fileSizeBytes
      : new Blob([nextContent]).size;
    if (size >= LARGE_FILE_SIZE_THRESHOLD_BYTES) {
      return true;
    }
    return hasVeryLongLine(nextContent, LARGE_FILE_MAX_LINE_LENGTH);
  }, []);

  const updateLargeFileMode = useCallback((nextContent: string, fileSizeBytes?: number) => {
    const nextMode = detectLargeFileMode(nextContent, fileSizeBytes);
    if (largeFileModeRef.current !== nextMode) {
      largeFileModeRef.current = nextMode;
      setLargeFileMode(nextMode);
      log.info('Editor performance mode changed', {
        filePath,
        largeFileMode: nextMode,
        fileSizeBytes: typeof fileSizeBytes === 'number' ? fileSizeBytes : undefined
      });
    }
  }, [detectLargeFileMode, filePath]);

  const applyExternalContentToModel = useCallback((nextContent: string) => {
    const model = modelRef.current;
    if (!model) {
      pendingModelContentRef.current = nextContent;
      return;
    }

    pendingModelContentRef.current = null;
    if (model.getValue() === nextContent) {
      return;
    }

    const previousLoadingState = isLoadingContentRef.current;
    isLoadingContentRef.current = true;
    model.setValue(nextContent);

    queueMicrotask(() => {
      if (!isUnmountedRef.current) {
        isLoadingContentRef.current = previousLoadingState;
      }
    });
  }, []);

  const applyDiskSnapshotToEditor = useCallback(
    (
      fileContent: string,
      version: DiskFileVersion | null,
      options?: { restoreCursor?: monaco.IPosition | null }
    ) => {
      updateLargeFileMode(fileContent);
      if (isUnmountedRef.current) {
        return;
      }
      isLoadingContentRef.current = true;
      setContent(fileContent);
      originalContentRef.current = fileContent;
      setHasChanges(false);
      hasChangesRef.current = false;
      if (version) {
        diskVersionRef.current = version;
      }
      applyExternalContentToModel(fileContent);
      const pos = options?.restoreCursor;
      if (pos && editorRef.current) {
        editorRef.current.setPosition(pos);
      }
      onContentChange?.(fileContent, false);
      reportFileMissingFromDisk(false);
      queueMicrotask(() => {
        isLoadingContentRef.current = false;
        if (modelRef.current && !isUnmountedRef.current && filePath) {
          savedVersionIdRef.current = modelRef.current.getAlternativeVersionId();
          monacoModelManager.markAsSaved(filePath);
        }
      });
    },
    [applyExternalContentToModel, filePath, onContentChange, reportFileMissingFromDisk, updateLargeFileMode]
  );

  const shouldBlockLargeFileExpansionClick = useCallback((target: EventTarget | null): boolean => {
    if (!(target instanceof HTMLElement)) {
      return false;
    }

    if (!target.closest('.monaco-editor')) {
      return false;
    }

    const clickable = target.closest('a,button,[role="button"],.monaco-button') as HTMLElement | null;
    const text = (clickable?.textContent ?? target.textContent ?? '')
      .toLowerCase()
      .replace(/\s+/g, ' ')
      .trim();

    if (!text) {
      return false;
    }

    return LARGE_FILE_EXPANSION_LABELS.some((label) => text.includes(label));
  }, []);

  useEffect(() => {
    filePathRef.current = filePath;
    pendingModelContentRef.current = null;
    lastJumpPositionRef.current = null;
  }, [filePath]);

  useEffect(() => {
    if (!statusBarPopover) return;
    const onMouseDown = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest('.status-bar-popover') || target.closest('.editor-status-bar')) return;
      setStatusBarPopover(null);
      setStatusBarAnchorRect(null);
    };
    document.addEventListener('mousedown', onMouseDown, true);
    return () => document.removeEventListener('mousedown', onMouseDown, true);
  }, [statusBarPopover]);

  // Sync font/config to editor when editorConfig changes (fixes late getConfig when opening from file tree)
  useEffect(() => {
    if (!monacoReady || !editorRef.current) return;
    const fs = editorConfig.font_size ?? 14;
    editorRef.current.updateOptions({
      fontSize: fs,
      fontFamily: editorConfig.font_family || "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
      fontWeight: editorConfig.font_weight || 'normal',
      lineHeight: editorConfig.line_height ? Math.round(fs * editorConfig.line_height) : 0,
    });
  }, [monacoReady, editorConfig.font_size, editorConfig.font_family, editorConfig.font_weight, editorConfig.line_height]);

  useEffect(() => {
    const applyConfig = (config: Partial<EditorConfigType>) => {
      const withUserIndent = userIndentRef.current
        ? { ...config, ...userIndentRef.current }
        : config;
      const appliedFontSize = config.font_size ?? 14;
      const newConfig: Partial<EditorConfigType> = {
        ...withUserIndent,
        minimap: {
          enabled: showMinimap,
          side: withUserIndent.minimap?.side || 'right',
          size: withUserIndent.minimap?.size || 'proportional'
        }
      };
      
      setEditorConfig(newConfig);
      latestEditorConfigRef.current = withUserIndent;

      const tabSize = withUserIndent.tab_size ?? config.tab_size ?? 2;
      const insertSpaces = withUserIndent.insert_spaces !== undefined ? withUserIndent.insert_spaces : (config.insert_spaces !== undefined ? config.insert_spaces : true);
      if (editorRef.current) {
        editorRef.current.updateOptions({
          fontSize: appliedFontSize,
          fontFamily: config.font_family || "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
          fontWeight: config.font_weight || 'normal',
          lineHeight: config.line_height 
            ? Math.round(appliedFontSize * config.line_height)
            : 0,
          tabSize,
          insertSpaces,
          wordWrap: (config.word_wrap as any) || 'off',
          lineNumbers: config.line_numbers as any || 'on',
          minimap: { 
            enabled: showMinimap && !largeFileMode,
            side: (config.minimap?.side as any) || 'right',
            size: (config.minimap?.size as any) || 'proportional'
          },
          cursorStyle: config.cursor_style as any || 'line',
          cursorBlinking: config.cursor_blinking as any || 'blink',
          smoothScrolling: largeFileMode ? false : (config.smooth_scrolling ?? true),
          renderWhitespace: config.render_whitespace as any || 'none',
          renderLineHighlight: config.render_line_highlight as any || 'line',
          bracketPairColorization: { enabled: largeFileMode ? false : (config.bracket_pair_colorization ?? true) },
          formatOnPaste: config.format_on_paste ?? false,
          trimAutoWhitespace: config.trim_auto_whitespace ?? true,
          inlayHints: { enabled: largeFileMode ? 'off' : 'on' },
          quickSuggestions: largeFileMode
            ? { other: false, comments: false, strings: false }
            : { other: true, comments: false, strings: false },
          'semanticHighlighting.enabled': !largeFileMode,
          renderValidationDecorations: largeFileMode ? 'off' : 'on',
          largeFileOptimizations: true,
          maxTokenizationLineLength: largeFileMode ? LARGE_FILE_MAX_TOKENIZATION_LINE_LENGTH : LARGE_FILE_MAX_LINE_LENGTH,
          occurrencesHighlight: largeFileMode ? 'off' : 'singleFile',
          selectionHighlight: !largeFileMode,
          matchBrackets: largeFileMode ? 'never' : 'always',
          disableMonospaceOptimizations: !largeFileMode,
          stopRenderingLineAfter: largeFileMode ? LARGE_FILE_RENDER_LINE_LIMIT : -1,
        });
      }
    };

    const loadEditorConfig = async () => {
      try {
        const config = await configManager.getConfig<EditorConfigType>('editor');
        if (config) {
          applyConfig(config);
        }
      } catch (error) {
        log.error('Failed to load editor config', error);
      }
    };
    
    loadEditorConfig();
    
    const handleConfigChange = (newConfig: unknown) => {
      if (newConfig && typeof newConfig === 'object') {
        applyConfig(newConfig as Partial<EditorConfigType>);
      }
    };
    
    globalEventBus.on('editor:config:changed', handleConfigChange);
    
    return () => {
      globalEventBus.off('editor:config:changed', handleConfigChange);
    };
  }, [showMinimap, largeFileMode]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !largeFileMode) {
      return;
    }

    const blockLargeFileExpansion = (event: MouseEvent) => {
      if (!shouldBlockLargeFileExpansionClick(event.target)) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();

      if (!largeFileExpansionBlockedLogRef.current) {
        largeFileExpansionBlockedLogRef.current = true;
        log.info('Blocked long-line expansion in large file mode', { filePath });
      }
    };

    container.addEventListener('mousedown', blockLargeFileExpansion, true);
    container.addEventListener('click', blockLargeFileExpansion, true);
    return () => {
      container.removeEventListener('mousedown', blockLargeFileExpansion, true);
      container.removeEventListener('click', blockLargeFileExpansion, true);
    };
  }, [filePath, largeFileMode, shouldBlockLargeFileExpansionClick]);

  useMonacoLsp(
    editorInstance,
    detectedLanguage,
    filePath,
    enableLsp && lspReady && monacoReady && !largeFileMode,
    workspacePath
  );

  useEffect(() => {
    if (!containerRef.current) {
      return;
    }

    let editor: monaco.editor.IStandaloneCodeEditor | null = null;
    let model: monaco.editor.ITextModel | null = null;

    const initEditor = async () => {
      try {
        if (!containerRef.current) {
          log.error('Container ref is null');
          return;
        }
        
        let createFontSize = 14;
        let createFontFamily = editorConfig.font_family || "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace";
        let createFontWeight = editorConfig.font_weight || 'normal';
        let createLineHeight = 0;
        const applyFontConfig = (c: Partial<EditorConfigType>) => {
          createFontSize = c.font_size ?? 14;
          createFontFamily = c.font_family || createFontFamily;
          createFontWeight = c.font_weight || createFontWeight;
          createLineHeight = c.line_height ? Math.round(createFontSize * c.line_height) : 0;
        };
        try {
          const preloadConfig = await configManager.getConfig<EditorConfigType>('editor');
          if (preloadConfig) applyFontConfig(preloadConfig);
          else if (latestEditorConfigRef.current) applyFontConfig(latestEditorConfigRef.current);
        } catch (_) {}
        
        await monacoInitManager.initialize();

        model = monacoModelManager.getOrCreateModel(
          filePath,
          detectedLanguage,
          content || '',
          workspacePath
        );
        
        modelRef.current = model;
        const modelContent = model.getValue();
        const initialLargeFileMode = detectLargeFileMode(modelContent);
        largeFileModeRef.current = initialLargeFileMode;
        setLargeFileMode(initialLargeFileMode);
        
        const modelMetadata = monacoModelManager.getModelMetadata(filePath);
        if (modelMetadata) {
          const isDirty = modelMetadata.isDirty;
          
          setHasChanges(isDirty);
          hasChangesRef.current = isDirty;
          savedVersionIdRef.current = modelMetadata.savedVersionId;
          originalContentRef.current = modelMetadata.originalContent;
          
          if (isDirty && onContentChange) {
            onContentChange(modelContent, true);
          }
        } else {
          savedVersionIdRef.current = model.getAlternativeVersionId();
        }
        
        if (modelContent && modelContent !== content) {
          setContent(modelContent);
          if (!modelMetadata) {
            originalContentRef.current = modelContent;
          }
        }

        forceRegisterTheme(BitFunDarkThemeMetadata.id, BitFunDarkTheme);

        let themeId = BitFunDarkThemeMetadata.id;
        try {
          const { themeService } = await import('@/infrastructure/theme');
          const currentTheme = themeService.getCurrentTheme();
          if (currentTheme) {
            themeId = currentTheme.monaco ? currentTheme.id : (currentTheme.type === 'dark' ? BitFunDarkThemeMetadata.id : 'vs');
            setCurrentThemeId(themeId);
          }
        } catch (error) {
          log.warn('Failed to get current theme, using default', error);
        }
        
        const editorOptions: monaco.editor.IStandaloneEditorConstructionOptions = {
          model: model,
          theme: themeId,
          automaticLayout: true,
          readOnly: readOnly,
          lineNumbers: showLineNumbers ? 'on' : (editorConfig.line_numbers as any) || 'on',
          minimap: { 
            enabled: showMinimap && !initialLargeFileMode,
            side: (editorConfig.minimap?.side as any) || 'right',
            size: (editorConfig.minimap?.size as any) || 'proportional'
          },
          fontSize: createFontSize,
          fontFamily: createFontFamily,
          fontWeight: createFontWeight,
          lineHeight: createLineHeight || (editorConfig.line_height ? Math.round(createFontSize * editorConfig.line_height) : 0),
          scrollBeyondLastLine: false,
          wordWrap: (editorConfig.word_wrap as any) || 'off',
          tabSize: editorConfig.tab_size || 2,
          insertSpaces: editorConfig.insert_spaces !== undefined ? editorConfig.insert_spaces : true,
          contextmenu: false,
          links: true,
          gotoLocation: {
            multipleDefinitions: 'goto',
            multipleTypeDefinitions: 'goto',
            multipleDeclarations: 'goto',
            multipleImplementations: 'goto',
            multipleReferences: 'goto'
          },
          multiCursorModifier: 'alt',
          definitionLinkOpensInPeek: false,
          inlayHints: {
            enabled: initialLargeFileMode ? 'off' : 'on',
            fontSize: 12,
            fontFamily: "'Fira Code', Consolas, 'Courier New', monospace",
            padding: false
          },

          hover: (() => {
            // Only enable hover for languages with actual hover providers:
            // - Languages supported by our LSP plugins
            // - Languages with useful built-in Monaco hover (TS/JS)
            const monacoHoverLanguages = ['typescript','javascript','typescriptreact','javascriptreact'];
            const hasHoverSupport = lspExtensionRegistry.isFileSupported(filePath)
              || monacoHoverLanguages.includes(detectedLanguage);
            return {
              enabled: hasHoverSupport,
              delay: 100,
              sticky: true,
              above: false
            };
          })(),

          quickSuggestions: {
            other: !initialLargeFileMode,
            comments: false,
            strings: false
          },
          suggest: {
            showKeywords: true,
            showSnippets: true
          },
          
          'semanticHighlighting.enabled': !initialLargeFileMode,
          guides: {
            indentation: true,
            bracketPairs: true,
            bracketPairsHorizontal: 'active',
            highlightActiveBracketPair: true,
            highlightActiveIndentation: true
          },

          renderLineHighlight: 'line',
          renderControlCharacters: false,
          renderValidationDecorations: initialLargeFileMode ? 'off' : 'on',
          largeFileOptimizations: true,
          maxTokenizationLineLength: initialLargeFileMode ? LARGE_FILE_MAX_TOKENIZATION_LINE_LENGTH : LARGE_FILE_MAX_LINE_LENGTH,
          occurrencesHighlight: initialLargeFileMode ? 'off' : 'singleFile',
          selectionHighlight: !initialLargeFileMode,
          matchBrackets: initialLargeFileMode ? 'never' : 'always',
          smoothScrolling: !initialLargeFileMode,
          roundedSelection: false,
          disableMonospaceOptimizations: !initialLargeFileMode,
          fontLigatures: false,
          stopRenderingLineAfter: initialLargeFileMode ? LARGE_FILE_RENDER_LINE_LIMIT : -1,
          scrollbar: {
            vertical: 'auto',
            horizontal: 'auto',
            useShadows: false,
            verticalScrollbarSize: 10,
            horizontalScrollbarSize: 10
          }
        };

        editor = monaco.editor.create(containerRef.current, editorOptions);
        editorRef.current = editor;
        setEditorInstance(editor);
        const editTarget = createMonacoEditTarget(editor);
        const unbindEditTarget = activeEditTargetService.bindTarget(editTarget);
        const focusDisposable = editor.onDidFocusEditorText(() => {
          activeEditTargetService.setActiveTarget(editTarget.id);
        });
        const blurDisposable = editor.onDidBlurEditorText(() => {
          window.setTimeout(() => {
            if (editor?.hasTextFocus()) {
              return;
            }

            activeEditTargetService.clearActiveTarget(editTarget.id);
          }, 0);
        });
        macosEditorBindingCleanupRef.current = () => {
          focusDisposable.dispose();
          blurDisposable.dispose();
          unbindEditTarget();
        };
        // #endregion
        
        (containerRef.current as any).__monacoEditor = editor;
        
        if (model) {
          const { lspDocumentService } = await import('@/tools/lsp/services/LspDocumentService');
          lspDocumentService.associateEditor(model.uri.toString(), editor);
        }
        
        const hasContent = model && model.getValue().length > 0;
        setMonacoReady(true);
        if (hasContent) {
          setLspReady(true);
        }
        const applyOptionsFromConfig = (c: Partial<EditorConfigType>) => {
          const fs = c.font_size ?? 14;
          editor!.updateOptions({
            fontSize: fs,
            fontFamily: c.font_family || "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
            fontWeight: c.font_weight || 'normal',
            lineHeight: c.line_height ? Math.round(fs * c.line_height) : 0,
          });
        };
        try {
          const latestConfig = await configManager.getConfig<EditorConfigType>('editor');
          if (latestConfig) applyOptionsFromConfig(latestConfig);
          else if (latestEditorConfigRef.current) applyOptionsFromConfig(latestEditorConfigRef.current);
        } catch (_) {}
        // Delayed font apply: config may not be ready when opening from file tree
        if (delayedFontApplyTimerRef.current) clearTimeout(delayedFontApplyTimerRef.current);
        delayedFontApplyTimerRef.current = setTimeout(() => {
          delayedFontApplyTimerRef.current = null;
          if (isUnmountedRef.current || !editorRef.current) return;
          (async () => {
            try {
              const cfg = await configManager.getConfig<EditorConfigType>('editor') || latestEditorConfigRef.current;
              if (cfg && editorRef.current) {
                applyOptionsFromConfig(cfg);
              }
            } catch (_) {}
          })();
        }, 150);

        // Intercept cross-file jumps from Peek References
        const originalModel = model;
        editor.onDidChangeModel((e) => {
          if (e.newModelUrl && e.oldModelUrl && e.newModelUrl.toString() !== e.oldModelUrl.toString()) {
            const newUri = e.newModelUrl.toString();
            let targetLine = 1;
            let targetColumn = 1;
            
            if (editor) {
              const cursorPosition = editor.getPosition();
              if (cursorPosition) {
                targetLine = cursorPosition.lineNumber;
                targetColumn = cursorPosition.column;
              }
            }
            
            if (originalModel && !originalModel.isDisposed() && editor) {
              editor.setModel(originalModel);
            }
            
            (async () => {
              try {
                const { normalizePath } = await import('@/shared/utils/pathUtils');
                const normalizedPath = normalizePath(newUri);
                
                const { fileTabManager } = await import('@/shared/services/FileTabManager');
                fileTabManager.openFileAndJump(
                  normalizedPath,
                  targetLine,
                  targetColumn,
                  { workspacePath }
                );
              } catch (error) {
                log.error('Cross-file jump failed', error);
              }
            })();
          }
        });

        contentChangeListenerRef.current = model.onDidChangeContent(() => {
          if (isLoadingContentRef.current) {
            return;
          }
          
          const newContent = model!.getValue();
          setContent(newContent);
          
          const currentVersionId = model!.getAlternativeVersionId();
          const changed = currentVersionId !== savedVersionIdRef.current;
          
          setHasChanges(changed);
          hasChangesRef.current = changed;
          
          onContentChange?.(newContent, changed);
        });

        editor.onDidChangeCursorPosition((e) => {
          setCursorPosition({
            line: e.position.lineNumber,
            column: e.position.column
          });
        });

        editor.onDidChangeCursorSelection((e) => {
          const sel = e.selection;
          if (sel.isEmpty()) {
            setSelection({ chars: 0, lines: 0 });
          } else {
            const selectedText = model!.getValueInRange(sel);
            const lines = sel.endLineNumber - sel.startLineNumber + 1;
            setSelection({
              chars: selectedText.length,
              lines: lines > 1 ? lines : 0
            });
          }
        });

        const updateCursorPosition = (e: monaco.editor.IEditorMouseEvent) => {
          if (e.target.position && containerRef.current?.parentElement?.parentElement) {
            // containerRef -> .code-editor-tool__content -> .code-editor-tool (has data-monaco-editor)
            const container = containerRef.current.parentElement.parentElement;
            const newLine = String(e.target.position.lineNumber);
            const newColumn = String(e.target.position.column);
            
            if (container.getAttribute('data-cursor-line') !== newLine || 
                container.getAttribute('data-cursor-column') !== newColumn) {
              container.setAttribute('data-cursor-line', newLine);
              container.setAttribute('data-cursor-column', newColumn);
            }
          }
        };

        editor.onMouseDown((e) => {
          updateCursorPosition(e);

          if ((e.event.ctrlKey || e.event.metaKey) && e.event.leftButton && e.target.position) {
            e.event.preventDefault();
            e.event.stopPropagation();
            editor!.setPosition(e.target.position);
            
            globalEventBus.emit('editor:goto-definition', {
              filePath: filePath,
              line: e.target.position.lineNumber,
              column: e.target.position.column
            });
          }
        });

        editor.onMouseMove((e) => {
          updateCursorPosition(e);

          if (!(e.event.ctrlKey || e.event.metaKey)) {
            if (ctrlDecorationsRef.current.length > 0) {
              try {
                ctrlDecorationsRef.current = editor!.deltaDecorations(ctrlDecorationsRef.current, []);
              } catch (err) {
                ctrlDecorationsRef.current = [];
              }
              lastHoverWordRef.current = null;
            }
            return;
          }
          
          if (e.target.position) {
            const word = model!.getWordAtPosition(e.target.position);
            if (word && word.word !== lastHoverWordRef.current) {
              lastHoverWordRef.current = word.word;
              const range = new monaco.Range(
                e.target.position.lineNumber,
                word.startColumn,
                e.target.position.lineNumber,
                word.endColumn
              );
              ctrlDecorationsRef.current = editor!.deltaDecorations(ctrlDecorationsRef.current, [{
                range,
                options: {
                  inlineClassName: 'ctrl-click-underline'
                }
              }]);
            }
          }
        });

        setMonacoReady(true);
        
        import('@/shared/services/EditorJumpService').then(({ editorJumpService }) => {
          editorJumpService.registerEditor(filePath, editor);
        }).catch(err => {
          log.error('Failed to register EditorJumpService', err);
        });
        
        import('@/tools/editor/services/EditorReadyManager').then(({ editorReadyManager }) => {
          editorReadyManager.markEditorReady(filePath, editor);
        }).catch(err => {
          log.error('Failed to load EditorReadyManager', err);
        });
        
        if (!loading && content) {
          setLspReady(true);
        }

      } catch (error) {
        log.error('Failed to initialize editor', error);
        setError(t('editor.codeEditor.initFailedWithMessage', { message: String(error) }));
      }
    };

    initEditor();

    return () => {
      isUnmountedRef.current = true;
      if (macosEditorBindingCleanupRef.current) {
        macosEditorBindingCleanupRef.current();
        macosEditorBindingCleanupRef.current = null;
      }
      if (delayedFontApplyTimerRef.current) {
        clearTimeout(delayedFontApplyTimerRef.current);
        delayedFontApplyTimerRef.current = null;
      }
      if (contentChangeListenerRef.current) {
        contentChangeListenerRef.current.dispose();
        contentChangeListenerRef.current = null;
      }
      
      if (editorRef.current) {
        if (modelRef.current) {
          import('@/tools/lsp/services/LspDocumentService').then(({ lspDocumentService }) => {
            lspDocumentService.disassociateEditor(modelRef.current!.uri.toString());
          });
        }
        
        editorRef.current.dispose();
        editorRef.current = null;
        setEditorInstance(null);
      }
      
      if (containerRef.current) {
        delete (containerRef.current as any).__monacoEditor;
      }

      monacoModelManager.releaseModel(filePath);

      import('@/shared/services/EditorJumpService').then(({ editorJumpService }) => {
        editorJumpService.unregisterEditor(filePath);
      }).catch(err => {
        log.error('Failed to unregister EditorJumpService', err);
      });
      
      import('@/tools/editor/services/EditorReadyManager').then(({ editorReadyManager }) => {
        editorReadyManager.cleanup(filePath);
      }).catch(err => {
        log.error('Failed to cleanup EditorReadyManager', err);
      });
    };
  }, [filePath, detectedLanguage, detectLargeFileMode]);

  useEffect(() => {
    if (monacoReady && pendingModelContentRef.current !== null) {
      applyExternalContentToModel(pendingModelContentRef.current);
    }
  }, [monacoReady, applyExternalContentToModel]);

  useEffect(() => {
    if (content && !lspReady) {
      setLspReady(true);
    }
  }, [content, lspReady]);

  useEffect(() => {
    if (modelRef.current && monacoReady) {
      const currentLanguage = modelRef.current.getLanguageId();
      if (detectedLanguage !== currentLanguage) {
        monaco.editor.setModelLanguage(modelRef.current, detectedLanguage);
      }
    }
  }, [detectedLanguage, monacoReady]);

  useEffect(() => {
    if (editorRef.current) {
      editorRef.current.updateOptions({ readOnly });
    }
  }, [readOnly]);

  const performJump = useCallback((editor: any, model: any, line: number, column: number, endLine?: number) => {
    const lineCount = model.getLineCount();
    const targetLine = Math.min(line, Math.max(1, lineCount));
    const targetEndLine = endLine ? Math.min(endLine, Math.max(1, lineCount)) : undefined;
    const maxColumnForLine = model.getLineMaxColumn(targetLine);
    const targetColumn = Math.min(Math.max(1, column), maxColumnForLine);

    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        try {
          editor.setPosition({
            lineNumber: targetLine,
            column: targetColumn
          });

          if (targetEndLine && targetEndLine > targetLine) {
            const endLineMaxColumn = model.getLineMaxColumn(targetEndLine);
            editor.setSelection({
              startLineNumber: targetLine,
              startColumn: 1,
              endLineNumber: targetEndLine,
              endColumn: endLineMaxColumn
            });
            editor.revealRangeInCenter({
              startLineNumber: targetLine,
              startColumn: 1,
              endLineNumber: targetEndLine,
              endColumn: endLineMaxColumn
            });
          } else {
            editor.revealLineInCenter(targetLine);
            editor.setSelection({
              startLineNumber: targetLine,
              startColumn: targetColumn,
              endLineNumber: targetLine,
              endColumn: targetColumn
            });
          }

          editor.focus();
        } catch (error) {
          log.error('Jump execution failed', error);
        }
      });
    });
  }, []);

  // Handle initial jump (after content load). If the model has fewer lines than requested,
  // wait for content to sync into the model; otherwise we clamp to line 1, set lastJump,
  // and dedupe blocks a correct jump after the real text arrives.
  useEffect(() => {
    const editor = editorRef.current;
    const model = modelRef.current;

    const finalRange =
      jumpToRange ||
      (jumpToLine ? { start: jumpToLine, end: jumpToColumn ? jumpToLine : undefined } : undefined);

    if (!finalRange) {
      return;
    }

    const targetColumn = 1;
    const lastJump = lastJumpPositionRef.current;
    if (
      lastJump &&
      lastJump.filePath === filePath &&
      lastJump.line === finalRange.start &&
      lastJump.endLine === finalRange.end
    ) {
      return;
    }

    if (!editor || !model || !monacoReady) {
      return;
    }

    if (loading) {
      return;
    }

    const maxLineNeeded = Math.max(finalRange.start, finalRange.end ?? finalRange.start);
    const lineCount = model.getLineCount();

    const applyJumpForCurrentModel = () => {
      const ed = editorRef.current;
      const md = modelRef.current;
      if (!ed || !md) {
        return;
      }
      lastJumpPositionRef.current = {
        filePath,
        line: finalRange.start,
        column: targetColumn,
        endLine: finalRange.end,
      };
      performJump(ed, md, finalRange.start, targetColumn, finalRange.end);
    };

    if (lineCount >= maxLineNeeded) {
      applyJumpForCurrentModel();
      return;
    }

    let finished = false;
    let timeoutId: number | null = null;
    let contentDisposable: { dispose: () => void } | null = null;

    const finishOnce = () => {
      if (finished) {
        return;
      }
      finished = true;
      contentDisposable?.dispose();
      contentDisposable = null;
      if (timeoutId != null) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }
      applyJumpForCurrentModel();
    };

    contentDisposable = model.onDidChangeContent(() => {
      const md = modelRef.current;
      if (md && md.getLineCount() >= maxLineNeeded) {
        finishOnce();
      }
    });

    timeoutId = window.setTimeout(finishOnce, 600);

    return () => {
      finished = true;
      contentDisposable?.dispose();
      contentDisposable = null;
      if (timeoutId != null) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }
    };
  }, [jumpToRange, jumpToLine, jumpToColumn, monacoReady, loading, content, filePath, performJump]);

  // Status bar popover: open and confirm
  const openStatusBarPopover = useCallback((type: 'position' | 'indent' | 'encoding' | 'language', e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setStatusBarAnchorRect({
      top: rect.top,
      left: rect.left,
      right: rect.right,
      bottom: rect.bottom,
      width: rect.width,
      height: rect.height,
    });
    setStatusBarPopover(type);
  }, []);

  const closeStatusBarPopover = useCallback(() => {
    setStatusBarPopover(null);
    setStatusBarAnchorRect(null);
  }, []);

  const handleGoToLineConfirm = useCallback((line: number, column: number) => {
    const editor = editorRef.current;
    const model = modelRef.current;
    if (editor && model) performJump(editor, model, line, column);
  }, [performJump]);

  const handleIndentConfirm = useCallback((tabSize: number, insertSpaces: boolean) => {
    const merged = { tab_size: tabSize, insert_spaces: insertSpaces };
    userIndentRef.current = merged;
    setEditorConfig((prev) => ({ ...prev, ...merged }));
    const editor = editorRef.current;
    if (editor) {
      editor.updateOptions({ tabSize, insertSpaces });
    }
    // Async persistence, don't block UI update, don't trigger applyConfig override
    configManager.getConfig<EditorConfigType>('editor').then((config) => {
      const fullMerged = { ...(config || {}), ...merged };
      return configManager.setConfig('editor', fullMerged);
    }).catch((err) => {
      log.warn('Failed to persist indent config', err);
    });
  }, []);

  const handleEncodingConfirm = useCallback(async (newEncoding: string) => {
    setEncoding(newEncoding);
    if (!filePath) return;
    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      const content = await workspaceAPI.readFileContent(filePath, newEncoding);
      updateLargeFileMode(content);
      setContent(content);
      originalContentRef.current = content;
      setHasChanges(false);
      hasChangesRef.current = false;
      applyExternalContentToModel(content);
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const fileInfo: any = await invoke('get_file_metadata', { request: { path: filePath } });
        if (isFileMissingFromMetadata(fileInfo)) {
          reportFileMissingFromDisk(true);
        } else {
          reportFileMissingFromDisk(false);
          const v = diskVersionFromMetadata(fileInfo);
          if (v) {
            diskVersionRef.current = v;
          }
        }
      } catch (err) {
        if (isLikelyFileNotFoundError(err)) {
          reportFileMissingFromDisk(true);
        }
        log.warn('Failed to sync disk version after encoding change', err);
      }
      queueMicrotask(() => {
        if (modelRef.current && !isUnmountedRef.current) {
          savedVersionIdRef.current = modelRef.current.getAlternativeVersionId();
          monacoModelManager.markAsSaved(filePath);
        }
      });
    } catch (err) {
      if (isLikelyFileNotFoundError(err)) {
        reportFileMissingFromDisk(true);
      }
      log.warn('Failed to reload file with new encoding', err);
    }
  }, [applyExternalContentToModel, filePath, reportFileMissingFromDisk, updateLargeFileMode]);

  const handleLanguageConfirm = useCallback((languageId: string) => {
    userLanguageOverrideRef.current = true;
    setDetectedLanguage(languageId);
    if (modelRef.current && monacoReady) {
      monaco.editor.setModelLanguage(modelRef.current, languageId);
    }
  }, [monacoReady]);

  // Load file content
  const loadFileContent = useCallback(async () => {
    if (!filePath) {
      setLoading(false);
      return;
    }

    // If Model already has content, skip file loading to avoid overwriting unsaved changes (e.g. switching back to open tab)
    if (modelRef.current && modelRef.current.getValue()) {
      setLoading(false);
      void (async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          const fileInfo: any = await invoke('get_file_metadata', {
            request: { path: filePath }
          });
          if (isFileMissingFromMetadata(fileInfo)) {
            reportFileMissingFromDisk(true);
            return;
          }
          reportFileMissingFromDisk(false);
          const v = diskVersionFromMetadata(fileInfo);
          if (v) {
            diskVersionRef.current = v;
          }
        } catch (err) {
          if (isLikelyFileNotFoundError(err)) {
            reportFileMissingFromDisk(true);
          }
          log.warn('Failed to sync file metadata when skipping load', err);
        }
      })();
      return;
    }

    setLoading(true);
    setError(null);
    isLoadingContentRef.current = true;

    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      const { invoke } = await import('@tauri-apps/api/core');

      const fileContent = await workspaceAPI.readFileContent(filePath);
      reportFileMissingFromDisk(false);
      let fileSizeBytes: number | undefined;
      try {
        const fileInfoAfter: any = await invoke('get_file_metadata', {
          request: { path: filePath }
        });
        if (isFileMissingFromMetadata(fileInfoAfter)) {
          reportFileMissingFromDisk(true);
        } else {
          reportFileMissingFromDisk(false);
          const v = diskVersionFromMetadata(fileInfoAfter);
          if (v) {
            diskVersionRef.current = v;
          }
        }
        if (typeof fileInfoAfter?.size === 'number') {
          fileSizeBytes = fileInfoAfter.size;
        }
      } catch (err) {
        if (isLikelyFileNotFoundError(err)) {
          reportFileMissingFromDisk(true);
        }
        log.warn('Failed to get file metadata', err);
      }

      updateLargeFileMode(fileContent, fileSizeBytes);
      
      setContent(fileContent);
      originalContentRef.current = fileContent;
      setHasChanges(false);
      hasChangesRef.current = false;
      applyExternalContentToModel(fileContent);
      
      // NOTE: Do NOT call onContentChange here during initial load.
      // Calling it triggers parent re-render which unmounts this component,
      // causing an infinite loop. onContentChange should only be called
      // when user actually edits the content.
      
      // Sync versionId after Model update
      queueMicrotask(() => {
        if (modelRef.current && !isUnmountedRef.current) {
          savedVersionIdRef.current = modelRef.current.getAlternativeVersionId();
          monacoModelManager.markAsSaved(filePath);
        }
      });

    } catch (err) {
      // Simplify error message, show only core reason
      const errStr = String(err);
      let displayError = t('editor.common.loadFailed');
      if (errStr.includes('does not exist') || errStr.includes('No such file')) {
        displayError = t('editor.common.fileNotFound');
      } else if (errStr.includes('Permission denied') || errStr.includes('permission')) {
        displayError = t('editor.common.permissionDenied');
      } else if (errStr.includes('network') || errStr.includes('timeout')) {
        displayError = t('editor.common.networkError');
      }
      setError(displayError);
      log.error('Failed to load file', err);
      if (errStr.includes('does not exist') || errStr.includes('No such file')) {
        reportFileMissingFromDisk(true);
      }
    } finally {
      setLoading(false);
      queueMicrotask(() => {
        isLoadingContentRef.current = false;
      });
    }
  }, [applyExternalContentToModel, filePath, detectedLanguage, reportFileMissingFromDisk, t, updateLargeFileMode]);

  // Save file content
  const saveFileContent = useCallback(async () => {
    if (!filePath) return;
    
    // Read latest hasChanges state from ref to avoid closure issues
    const currentHasChanges = hasChangesRef.current;
    const currentContent = modelRef.current?.getValue() || '';
    
    // Use ref value instead of state
    if (!currentHasChanges) {
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      const { invoke } = await import('@tauri-apps/api/core');

      const fileInfoPre: any = await invoke('get_file_metadata', {
        request: { path: filePath }
      });
      if (isFileMissingFromMetadata(fileInfoPre)) {
        reportFileMissingFromDisk(true);
      } else {
        reportFileMissingFromDisk(false);
      }
      const diskNow = diskVersionFromMetadata(fileInfoPre);
      const baseline = diskVersionRef.current;

      if (diskNow && baseline && diskVersionsDiffer(diskNow, baseline)) {
        const overwrite = await confirmDialog({
          title: t('editor.codeEditor.saveConflictTitle'),
          message: t('editor.codeEditor.saveConflictDetail'),
          type: 'warning',
          confirmText: t('editor.codeEditor.overwriteSave'),
          cancelText: t('editor.codeEditor.reloadFromDisk'),
          confirmDanger: true,
        });
        if (!overwrite) {
          const diskContent = await workspaceAPI.readFileContent(filePath);
          const fileInfoAfter: any = await invoke('get_file_metadata', {
            request: { path: filePath }
          });
          const vAfter = diskVersionFromMetadata(fileInfoAfter);
          applyDiskSnapshotToEditor(diskContent, vAfter);
          return;
        }
      }

      await workspaceAPI.writeFileContent(workspacePath || '', filePath, currentContent);

      monacoModelManager.markAsSaved(filePath);

      originalContentRef.current = currentContent;
      setHasChanges(false);
      hasChangesRef.current = false;

      if (modelRef.current) {
        savedVersionIdRef.current = modelRef.current.getAlternativeVersionId();
      }

      onSave?.(currentContent);

      try {
        const fileInfo: any = await invoke('get_file_metadata', {
          request: { path: filePath }
        });
        if (!isFileMissingFromMetadata(fileInfo)) {
          reportFileMissingFromDisk(false);
          const v = diskVersionFromMetadata(fileInfo);
          if (v) {
            diskVersionRef.current = v;
          }
        }
      } catch (err) {
        log.warn('Failed to update file disk version after save', err);
      }

      globalEventBus.emit('file-tree:refresh');
    } catch (err) {
      const errorMsg = t('editor.common.saveFailedWithMessage', { message: String(err) });
      setError(errorMsg);
      log.error('Failed to save file', err);
    } finally {
      setSaving(false);
    }
  }, [filePath, workspacePath, onSave, reportFileMissingFromDisk, t, applyDiskSnapshotToEditor]);
  
  useEffect(() => {
    saveFileContentRef.current = saveFileContent;
  }, [saveFileContent]);

  // Container-level keyboard event handler, solves global conflict issues with multiple editor instances
  const handleContainerKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    const hasFocus = editorRef.current?.hasTextFocus() ?? false;
    if (!hasFocus) {
      return;
    }

    const isModKey = event.ctrlKey || event.metaKey;
    const lowerKey = event.key.toLowerCase();

    if (isModKey && lowerKey === 's') {
      event.preventDefault();
      event.stopPropagation();
      saveFileContentRef.current?.();
      return;
    }

    if (isModKey && lowerKey === 'z') {
      if (isMacOSDesktop()) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      if (event.shiftKey) {
        activeEditTargetService.executeAction('redo');
      } else {
        activeEditTargetService.executeAction('undo');
      }
      return;
    }

    if (!event.metaKey && event.ctrlKey && lowerKey === 'y') {
      event.preventDefault();
      event.stopPropagation();
      activeEditTargetService.executeAction('redo');
    }
  }, []);

  const checkFileModification = useCallback(async () => {
    if (!filePath || !isActiveTab || isCheckingFileRef.current) {
      return;
    }

    isCheckingFileRef.current = true;

    try {
      if (typeof document !== 'undefined' && document.visibilityState !== 'visible') {
        return;
      }

      const { invoke } = await import('@tauri-apps/api/core');
      const fileInfo: any = await invoke('get_file_metadata', {
        request: { path: filePath }
      });
      if (isFileMissingFromMetadata(fileInfo)) {
        reportFileMissingFromDisk(true);
        return;
      }
      reportFileMissingFromDisk(false);
      const currentVersion = diskVersionFromMetadata(fileInfo);
      if (!currentVersion) {
        return;
      }

      const baseline = diskVersionRef.current;
      if (!baseline) {
        diskVersionRef.current = currentVersion;
        return;
      }

      if (!diskVersionsDiffer(currentVersion, baseline)) {
        return;
      }

      const bufferBeforeRead = modelRef.current?.getValue();
      try {
        const hashRes: any = await invoke('get_file_editor_sync_hash', {
          request: { path: filePath },
        });
        const diskHash =
          typeof hashRes?.hash === 'string' ? hashRes.hash.toLowerCase() : '';
        const editorMid = modelRef.current?.getValue();
        if (
          bufferBeforeRead !== undefined &&
          editorMid !== undefined &&
          bufferBeforeRead !== editorMid
        ) {
          return;
        }
        if (diskHash && editorMid !== undefined) {
          const editorHash = await editorSyncContentSha256Hex(editorMid);
          if (editorHash === diskHash) {
            diskVersionRef.current = currentVersion;
            return;
          }
        }
      } catch (hashErr) {
        log.warn('get_file_editor_sync_hash failed, falling back to full read', {
          filePath,
          error: hashErr,
        });
      }

      const { workspaceAPI } = await import('@/infrastructure/api');
      const editorBuffer = modelRef.current?.getValue();
      if (
        bufferBeforeRead !== undefined &&
        editorBuffer !== undefined &&
        bufferBeforeRead !== editorBuffer
      ) {
        return;
      }
      if (editorBuffer === undefined) {
        return;
      }

      const fileContent = await workspaceAPI.readFileContent(filePath);
      if (diskContentMatchesEditorForExternalSync(fileContent, editorBuffer)) {
        diskVersionRef.current = currentVersion;
        return;
      }

      log.info('File modified externally', { filePath });

      if (hasChangesRef.current) {
        const shouldReload = await confirmDialog({
          title: t('editor.codeEditor.externalModifiedTitle'),
          message: t('editor.codeEditor.externalModifiedDetail'),
          type: 'warning',
          confirmText: t('editor.codeEditor.discardAndReload'),
          cancelText: t('editor.codeEditor.keepLocalEdits'),
          confirmDanger: true,
        });
        if (!shouldReload) {
          diskVersionRef.current = currentVersion;
          return;
        }
      }

      applyDiskSnapshotToEditor(fileContent, currentVersion);
    } catch (err) {
      if (isLikelyFileNotFoundError(err)) {
        reportFileMissingFromDisk(true);
      }
      log.error('Failed to check file modification', err);
    } finally {
      isCheckingFileRef.current = false;
    }
  }, [applyDiskSnapshotToEditor, filePath, isActiveTab, reportFileMissingFromDisk, t]);

  // Initial file load - only run once when filePath changes
  const loadFileContentCalledRef = useRef(false);
  useEffect(() => {
    loadFileContentCalledRef.current = false;
    diskVersionRef.current = null;
    lastReportedMissingRef.current = undefined;
  }, [filePath]);
  
  useEffect(() => {
    if (!loadFileContentCalledRef.current) {
      loadFileContentCalledRef.current = true;
      loadFileContent();
    }
  }, [loadFileContent]);

  useEffect(() => {
    if (!filePath || !isActiveTab) {
      return;
    }

    const tick = () => {
      void checkFileModification();
    };

    const intervalId = window.setInterval(tick, FILE_SYNC_POLL_INTERVAL_MS);
    document.addEventListener('visibilitychange', tick);

    return () => {
      window.clearInterval(intervalId);
      document.removeEventListener('visibilitychange', tick);
    };
  }, [checkFileModification, filePath, isActiveTab]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !monacoReady) {
      return;
    }

    const unsubscribers: Array<() => void> = [];

    const unsubGotoDef = globalEventBus.on('editor:goto-definition', async (data: any) => {
      const isMatch = isSamePath(data.filePath || '', filePath || '');
      
      if (isMatch) {
        try {
          const position = editor.getPosition();
          if (!position) {
            return;
          }
          
          const model = editor.getModel();
          if (!model) {
            return;
          }
          
          const { GlobalAdapterRegistry } = await import('@/tools/lsp/services/MonacoLspAdapter');
          const modelUri = model.uri.toString();
          const adapter = GlobalAdapterRegistry.get(modelUri);
          
          if (!adapter) {
            editor.trigger('keyboard', 'editor.action.revealDefinition', null);
            return;
          }
          
          const definition = await adapter.provideDefinition(model, position);
          
          if (!definition) {
            return;
          }
          
          const definitionUri = definition.uri.toString();
          const currentUri = model.uri.toString();
          
          // Determine if it's a cross-file jump
          if (definitionUri !== currentUri) {
            // Cross-file jump: use unified file tab manager
            const targetLine = definition.range.startLineNumber;
            const targetColumn = definition.range.startColumn;
            
            // Use unified file tab manager to open file
            const { fileTabManager } = await import('@/shared/services/FileTabManager');
            fileTabManager.openFileAndJump(
              definitionUri,
              targetLine,
              targetColumn,
              { workspacePath }
            );
          } else {
            // Same-file jump: use Monaco default behavior
            editor.setPosition({
              lineNumber: definition.range.startLineNumber,
              column: definition.range.startColumn
            });
            editor.revealPositionInCenter({
              lineNumber: definition.range.startLineNumber,
              column: definition.range.startColumn
            });
            editor.focus();
          }
          
        } catch (error) {
          log.error('Goto definition failed', error);
        }
      }
    });
    unsubscribers.push(unsubGotoDef);

    const unsubGotoTypeDef = globalEventBus.on('editor:goto-type-definition', (data: any) => {
      if (data.filePath === filePath) {
        try {
          editor.trigger('context-menu', 'editor.action.goToTypeDefinition', null);
        } catch (error) {
          log.error('Failed to trigger goToTypeDefinition', error);
          const action = editor.getAction('editor.action.goToTypeDefinition');
          if (action) {
            action.run();
          }
        }
      }
    });
    unsubscribers.push(unsubGotoTypeDef);

    const unsubFindRefs = globalEventBus.on('editor:find-references', async (data: any) => {
      if (data.filePath === filePath) {
        try {
          editor.trigger('context-menu', 'editor.action.referenceSearch.trigger', null);
        } catch (error) {
          log.error('Find references failed', error);
        }
      }
    });
    unsubscribers.push(unsubFindRefs);

    const unsubRename = globalEventBus.on('editor:rename-symbol', (data: any) => {
      if (data.filePath === filePath) {
        try {
          editor.trigger('context-menu', 'editor.action.rename', null);
        } catch (error) {
          log.error('Failed to trigger rename', error);
          const action = editor.getAction('editor.action.rename');
          if (action) {
            action.run();
          }
        }
      }
    });
    unsubscribers.push(unsubRename);

    const unsubFormat = globalEventBus.on('editor:format-document', (data: any) => {
      if (data.filePath === filePath) {
        try {
          editor.trigger('context-menu', 'editor.action.formatDocument', null);
        } catch (error) {
          log.error('Failed to trigger formatDocument', error);
          const action = editor.getAction('editor.action.formatDocument');
          if (action) {
            action.run();
          }
        }
      }
    });
    unsubscribers.push(unsubFormat);

    const unsubCodeAction = globalEventBus.on('editor:code-action', (data: any) => {
      if (data.filePath === filePath) {
        try {
          editor.trigger('context-menu', 'editor.action.quickFix', null);
        } catch (error) {
          log.error('Failed to trigger quickFix', error);
          const action = editor.getAction('editor.action.quickFix');
          if (action) {
            action.run();
          }
        }
      }
    });
    unsubscribers.push(unsubCodeAction);

    const unsubDocSymbols = globalEventBus.on('editor:document-symbols', (data: any) => {
      if (data.filePath === filePath) {
        const action = editor.getAction('editor.action.quickOutline');
        if (action) {
          action.run();
        }
      }
    });
    unsubscribers.push(unsubDocSymbols);

    const unsubDocHighlight = globalEventBus.on('editor:document-highlight', (data: any) => {
      if (data.filePath === filePath) {
        const position = editor.getPosition();
        if (position) {
          editor.setPosition(position);
          editor.focus();
        }
      }
    });
    unsubscribers.push(unsubDocHighlight);

    const unsubFileChanged = globalEventBus.on('editor:file-changed', async (data: { filePath: string }) => {
      if (!isSamePath(data.filePath || '', filePath || '')) {
        return;
      }

      try {
        const { workspaceAPI } = await import('@/infrastructure/api');
        const { invoke } = await import('@tauri-apps/api/core');
        const bufferBeforeRead = modelRef.current?.getValue();
        try {
          const hashRes: any = await invoke('get_file_editor_sync_hash', {
            request: { path: filePath },
          });
          const diskHash =
            typeof hashRes?.hash === 'string' ? hashRes.hash.toLowerCase() : '';
          const editorMid = modelRef.current?.getValue();
          if (
            bufferBeforeRead !== undefined &&
            editorMid !== undefined &&
            bufferBeforeRead !== editorMid
          ) {
            return;
          }
          if (diskHash && editorMid !== undefined) {
            const editorHash = await editorSyncContentSha256Hex(editorMid);
            if (editorHash === diskHash) {
              try {
                const fileInfo: any = await invoke('get_file_metadata', {
                  request: { path: filePath },
                });
                const v = diskVersionFromMetadata(fileInfo);
                if (v) {
                  diskVersionRef.current = v;
                }
              } catch (err) {
                log.warn('Failed to sync disk version after noop file-changed', err);
              }
              return;
            }
          }
        } catch (hashErr) {
          log.warn('get_file_editor_sync_hash failed in file-changed handler', {
            filePath,
            error: hashErr,
          });
        }

        const diskContent = await workspaceAPI.readFileContent(filePath);
        const editorBuffer = modelRef.current?.getValue();
        if (
          bufferBeforeRead !== undefined &&
          editorBuffer !== undefined &&
          bufferBeforeRead !== editorBuffer
        ) {
          return;
        }
        if (
          editorBuffer !== undefined &&
          diskContentMatchesEditorForExternalSync(diskContent, editorBuffer)
        ) {
          try {
            const fileInfo: any = await invoke('get_file_metadata', {
              request: { path: filePath },
            });
            const v = diskVersionFromMetadata(fileInfo);
            if (v) {
              diskVersionRef.current = v;
            }
          } catch (err) {
            log.warn('Failed to sync disk version after noop file-changed', err);
          }
          return;
        }

        if (hasChangesRef.current) {
          const shouldReload = await confirmDialog({
            title: t('editor.codeEditor.externalModifiedTitle'),
            message: t('editor.codeEditor.externalModifiedDetail'),
            type: 'warning',
            confirmText: t('editor.codeEditor.discardAndReload'),
            cancelText: t('editor.codeEditor.keepLocalEdits'),
            confirmDanger: true,
          });
          if (!shouldReload) {
            try {
              const fileInfo: any = await invoke('get_file_metadata', {
                request: { path: filePath }
              });
              const v = diskVersionFromMetadata(fileInfo);
              if (v) {
                diskVersionRef.current = v;
              }
            } catch (err) {
              log.warn('Failed to sync disk version after declining external reload', err);
            }
            return;
          }
        }

        const fileInfo: any = await invoke('get_file_metadata', {
          request: { path: filePath }
        });
        const ver = diskVersionFromMetadata(fileInfo);
        const currentPosition = editor?.getPosition() ?? null;
        applyDiskSnapshotToEditor(diskContent, ver, { restoreCursor: currentPosition });
      } catch (error) {
        log.error('Failed to reload file', error);
      }
    });
    unsubscribers.push(unsubFileChanged);

    const unsubSaveFile = globalEventBus.on('editor:save-file', (data: { filePath: string }) => {
      if (isSamePath(data.filePath || '', filePath || '')) {
        saveFileContentRef.current?.();
      }
    });
    unsubscribers.push(unsubSaveFile);

    return () => {
      unsubscribers.forEach(unsub => unsub());
    };
  }, [applyDiskSnapshotToEditor, applyExternalContentToModel, monacoReady, filePath, t]);

  useEffect(() => {
    userLanguageOverrideRef.current = false;
  }, [filePath]);

  useEffect(() => {
    if (userLanguageOverrideRef.current || !fileName) return;
    const newLanguage = detectLanguageFromFileName(fileName);
    if (newLanguage !== detectedLanguage) {
      setDetectedLanguage(newLanguage);
      setLspReady(false);
    }
  }, [fileName, detectedLanguage, detectLanguageFromFileName]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !monacoReady) {
      return;
    }

    let unsubscribeThemeService: (() => void) | null = null;
    
    (async () => {
      try {
        const { themeService } = await import('@/infrastructure/theme');
        
        unsubscribeThemeService = themeService.on('theme:after-change', (event) => {
          if (event.theme) {
            const newThemeId = event.theme.monaco ? event.theme.id : (event.theme.type === 'dark' ? BitFunDarkThemeMetadata.id : 'vs');
            
            setCurrentThemeId(newThemeId);
            
            // setTheme is global; updateOptions nudges this editor to re-render.
            try {
              editor.updateOptions({});
            } catch (error) {
              log.warn('Failed to update editor options', error);
            }
          }
        });
      } catch (error) {
        log.warn('Failed to register theme listener', error);
      }
    })();

    return () => {
      if (unsubscribeThemeService) {
        unsubscribeThemeService();
      }
    };
  }, [monacoReady]);

  return (
    <div 
      className={`code-editor-tool ${className} ${loading && showLoadingOverlay ? 'is-loading' : ''} ${error ? 'is-error' : ''} ${largeFileMode ? 'is-large-file-mode' : ''}`}
      data-monaco-editor="true"
      data-editor-id={`editor-${filePath.replace(/[^a-zA-Z0-9]/g, '-')}`}
      data-file-path={filePath}
      data-readonly={readOnly ? 'true' : 'false'}
      onKeyDownCapture={handleContainerKeyDown}
    >
      <EditorBreadcrumb 
        filePath={filePath}
        workspacePath={workspacePath}
      />
      
      <div className="code-editor-tool__content">
        <div 
          ref={containerRef} 
          style={{ 
            width: '100%', 
            height: '100%',
            overflow: 'hidden',
            opacity: loading && showLoadingOverlay ? 0.3 : 1,
            transition: 'opacity 0.2s'
          }} 
        />
      </div>

      {loading && showLoadingOverlay && (
        <div className="code-editor-tool__loading-overlay">
          <CubeLoading size="medium" text={t('editor.codeEditor.loadingFile')} />
        </div>
      )}

      {error && (
        <div className="code-editor-tool__error-overlay">
          <AlertCircle className="code-editor-tool__error-icon" />
          <p className="code-editor-tool__error-message">{error}</p>
          <button
            onClick={loadFileContent}
            className="code-editor-tool__error-retry-btn"
            type="button"
          >
            {t('editor.common.retry')}
          </button>
        </div>
      )}

      {saving && (
        <div className="code-editor-tool__saving-indicator">
          {t('editor.codeEditor.saving')}
        </div>
      )}

      <EditorStatusBar
        line={cursorPosition.line}
        column={cursorPosition.column}
        selectedChars={selection.chars}
        selectedLines={selection.lines}
        language={detectedLanguage}
        encoding={encoding}
        tabSize={editorConfig.tab_size || 2}
        insertSpaces={editorConfig.insert_spaces !== false}
        isReadOnly={readOnly}
        lspStatus={
          enableLsp && lspExtensionRegistry.isFileSupported(filePath)
            ? (lspReady ? 'connected' : 'connecting')
            : undefined
        }
        onPositionClick={(e) => openStatusBarPopover('position', e)}
        onIndentClick={(e) => openStatusBarPopover('indent', e)}
        onEncodingClick={(e) => openStatusBarPopover('encoding', e)}
        onLanguageClick={(e) => openStatusBarPopover('language', e)}
      />

      {statusBarPopover === 'position' && statusBarAnchorRect && (
        <GoToLinePopover
          anchorRect={statusBarAnchorRect}
          currentLine={cursorPosition.line}
          currentColumn={cursorPosition.column}
          onConfirm={handleGoToLineConfirm}
          onClose={closeStatusBarPopover}
        />
      )}
      {statusBarPopover === 'indent' && statusBarAnchorRect && (
        <IndentPopover
          anchorRect={statusBarAnchorRect}
          currentTabSize={editorConfig.tab_size || 2}
          currentInsertSpaces={editorConfig.insert_spaces !== false}
          onConfirm={handleIndentConfirm}
          onClose={closeStatusBarPopover}
        />
      )}
      {statusBarPopover === 'encoding' && statusBarAnchorRect && (
        <EncodingPopover
          anchorRect={statusBarAnchorRect}
          currentEncoding={encoding}
          onConfirm={handleEncodingConfirm}
          onClose={closeStatusBarPopover}
        />
      )}
      {statusBarPopover === 'language' && statusBarAnchorRect && (
        <LanguagePopover
          anchorRect={statusBarAnchorRect}
          currentLanguageId={detectedLanguage}
          languages={monaco.languages.getLanguages()}
          onConfirm={handleLanguageConfirm}
          onClose={closeStatusBarPopover}
        />
      )}
    </div>
  );
};

export default CodeEditor;
