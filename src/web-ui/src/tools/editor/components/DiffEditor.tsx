/** Monaco diff editor wrapper (side-by-side/inline). */

import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import * as monaco from 'monaco-editor';
import { monacoInitManager } from '../services/MonacoInitManager';
import { 
  forceRegisterTheme,
  BitFunDarkTheme,
  BitFunDarkThemeMetadata 
} from '../themes';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { EditorConfig as EditorConfigType } from '@/infrastructure/config/types';
import { useMonacoLsp } from '@/tools/lsp/hooks/useMonacoLsp';
import { getMonacoLanguage } from '@/infrastructure/language-detection';
import { Tooltip, CubeLoading } from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { AlertCircle } from 'lucide-react';
import './DiffEditor.scss';

const log = createLogger('DiffEditor');

export interface DiffEditorProps {
  /** Original content */
  originalContent: string;
  /** Modified content */
  modifiedContent: string;
  /** File path */
  filePath?: string;
  /** Workspace path (reserved for future use) */
  workspacePath?: string;
  /** Repository path (for Git Diff) */
  repositoryPath?: string;
  /** Programming language */
  language?: string;
  /** Read-only (modified side) */
  readOnly?: boolean;
  /** View mode: side-by-side or inline */
  renderSideBySide?: boolean;
  /** Show minimap */
  showMinimap?: boolean;
  /** CSS class name */
  className?: string;
  /** Modified content change callback */
  onModifiedContentChange?: (content: string) => void;
  /** Diff change callback (triggered when Monaco finishes diff computation) */
  onDiffChange?: (changes: monaco.editor.ILineChange[]) => void;
  /** Accept change callback (for custom UI, not yet implemented) */
  onAcceptChange?: (lineNumber: number) => void;
  /** Reject change callback (for custom UI, not yet implemented) */
  onRejectChange?: (lineNumber: number) => void;
  /** Enable custom toolbar */
  enableCustomToolbar?: boolean;
  /** Save callback (Ctrl+S triggers) */
  onSave?: (content: string) => void;
  /** Reveal line in modified editor (1-based) */
  revealLine?: number;
  /** Enable LSP (only for modified editor) */
  enableLsp?: boolean;
  /** Show +/- indicators before lines (default true) */
  renderIndicators?: boolean;
}

export const DiffEditor: React.FC<DiffEditorProps> = ({
  originalContent,
  modifiedContent,
  filePath,
  workspacePath,
  repositoryPath: _repositoryPath,
  language: propLanguage,
  readOnly = false,
  renderSideBySide = true,
  showMinimap = false,
  className = '',
  onModifiedContentChange,
  onDiffChange,
  onAcceptChange: _onAcceptChange,
  onRejectChange: _onRejectChange,
  enableCustomToolbar = false,
  revealLine,
  enableLsp = true,
  renderIndicators = true,
  onSave
}) => {
  const notification = useNotification();
  const { t } = useI18n('tools');

  const [diffEditor, setDiffEditor] = useState<monaco.editor.IStandaloneDiffEditor | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [changes, setChanges] = useState<monaco.editor.ILineChange[]>([]);
  const [editorConfig, setEditorConfig] = useState<Partial<EditorConfigType>>({
    font_size: 14,
    font_family: "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
    line_height: 1.5,
    tab_size: 2,
    insert_spaces: true,
    word_wrap: 'on',
    line_numbers: 'on',
    minimap: { enabled: showMinimap, side: 'right', size: 'proportional' }
  });
  const [_currentThemeId, setCurrentThemeId] = useState<string>(BitFunDarkThemeMetadata.id);
  void _currentThemeId;
  
  const containerRef = useRef<HTMLDivElement>(null);
  const originalModelRef = useRef<monaco.editor.ITextModel | null>(null);
  const modifiedModelRef = useRef<monaco.editor.ITextModel | null>(null);
  const changeListenerRef = useRef<monaco.IDisposable | null>(null);
  const contentChangeListenerRef = useRef<monaco.IDisposable | null>(null);
  const isUnmountedRef = useRef(false);
  const [modifiedEditorInstance, setModifiedEditorInstance] = useState<monaco.editor.IStandaloneCodeEditor | null>(null);
  const onSaveRef = useRef(onSave);
  
  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  const detectLanguage = useCallback((filePath?: string): string => {
    if (propLanguage) return propLanguage;
    if (!filePath) return 'plaintext';
    
    const detected = getMonacoLanguage(filePath);
    return detected !== 'plaintext' ? detected : 'plaintext';
  }, [propLanguage]);

  const detectedLanguage = useMemo(() => detectLanguage(filePath), [filePath, detectLanguage]);

  useMonacoLsp(
    modifiedEditorInstance,
    detectedLanguage,
    filePath || '',
    Boolean(enableLsp && modifiedEditorInstance && filePath),
    workspacePath
  );

  useEffect(() => {
    const loadEditorConfig = async () => {
      try {
        const config = await configManager.getConfig<EditorConfigType>('editor');
        if (config) {
          setEditorConfig(prev => ({
            ...prev,
            ...config,
            minimap: {
              ...config.minimap,
              enabled: showMinimap
            }
          }));
        }
      } catch (error) {
        log.error('Failed to load editor config', error);
      }
    };
    loadEditorConfig();
  }, [showMinimap]);

  useEffect(() => {
    if (!containerRef.current) return;

    let editor: monaco.editor.IStandaloneDiffEditor | null = null;
    let originalModel: monaco.editor.ITextModel | null = null;
    let modifiedModel: monaco.editor.ITextModel | null = null;

    const initDiffEditor = async () => {
      try {
        await monacoInitManager.initialize();

        if (!containerRef.current) {
          log.error('Container ref is null during initialization');
          return;
        }

        const timestamp = Date.now();
        const originalUri = monaco.Uri.parse(`inmemory://diff-original/${timestamp}/${filePath || 'untitled'}`);
        const modifiedUri = monaco.Uri.parse(`inmemory://diff-modified/${timestamp}/${filePath || 'untitled'}`);

        const existingOriginalModel = monaco.editor.getModel(originalUri);
        const existingModifiedModel = monaco.editor.getModel(modifiedUri);
        
        if (existingOriginalModel) {
          existingOriginalModel.dispose();
        }
        if (existingModifiedModel) {
          existingModifiedModel.dispose();
        }

        const existingOriginal = monaco.editor.getModel(originalUri);
        if (existingOriginal) {
          existingOriginal.dispose();
        }

        const existingModified = monaco.editor.getModel(modifiedUri);
        if (existingModified) {
          existingModified.dispose();
        }

        originalModel = monaco.editor.createModel(originalContent, detectedLanguage, originalUri);
        modifiedModel = monaco.editor.createModel(modifiedContent, detectedLanguage, modifiedUri);

        originalModelRef.current = originalModel;
        modifiedModelRef.current = modifiedModel;

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
        
        forceRegisterTheme(BitFunDarkThemeMetadata.id, BitFunDarkTheme);
        
        const editorOptions: monaco.editor.IStandaloneDiffEditorConstructionOptions = {
          renderSideBySide: renderSideBySide,
          renderOverviewRuler: false,
          renderIndicators: renderIndicators,
          renderMarginRevertIcon: true,
          renderGutterMenu: true,

          originalEditable: false,
          readOnly: readOnly,
          
          ignoreTrimWhitespace: false,
          renderWhitespace: 'selection',
          diffWordWrap: (editorConfig.word_wrap as any) || 'on',
          diffAlgorithm: 'advanced',
          
          hideUnchangedRegions: {
            enabled: true,
            contextLineCount: 3,
            minimumLineCount: 5,
            revealLineCount: 20,
          },
          
          theme: themeId,
          automaticLayout: true,
          fontSize: editorConfig.font_size || 14,
          fontFamily: editorConfig.font_family || "'Fira Code', 'Noto Sans SC', Consolas, 'Courier New', monospace",
          lineHeight: editorConfig.line_height 
            ? Math.round((editorConfig.font_size || 14) * editorConfig.line_height)
            : 0,
          lineNumbers: (editorConfig.line_numbers || 'on') as monaco.editor.LineNumbersType,
          minimap: { 
            enabled: showMinimap,
            side: (editorConfig.minimap?.side || 'right') as 'right' | 'left',
            size: (editorConfig.minimap?.size || 'proportional') as 'proportional' | 'fill' | 'fit'
          },
          scrollBeyondLastLine: false,
          contextmenu: false,
          
          glyphMargin: false,
          folding: false,
          lineNumbersMinChars: 4,
          lineDecorationsWidth: 10,
          padding: { top: 4, bottom: 4 },
          
          renderLineHighlight: 'none',
          overviewRulerBorder: false,
          
          enableSplitViewResizing: true,
        };

        editor = monaco.editor.createDiffEditor(containerRef.current, editorOptions);
        
        editor.setModel({
          original: originalModel,
          modified: modifiedModel
        });

        setModifiedEditorInstance(editor.getModifiedEditor());
        setDiffEditor(editor);
        
        // Force set background color immediately to avoid white flash
        requestAnimationFrame(() => {
          if (containerRef.current && !isUnmountedRef.current) {
            const elementsToFix = [
              '.monaco-diff-editor',
              '.editor.original',
              '.editor.modified',
              '.editor.original .monaco-editor',
              '.editor.modified .monaco-editor',
              '.monaco-editor-background',
              '.editor.original .monaco-editor-background',
              '.editor.modified .monaco-editor-background',
              '.margin',
              '.margin-view-overlays',
              '.gutter-background',
              '.editor.original .margin',
              '.editor.modified .margin',
              '.editor.original .margin-view-overlays',
              '.editor.modified .margin-view-overlays',
            ];
            
            elementsToFix.forEach(selector => {
              const elements = containerRef.current!.querySelectorAll(selector);
              elements.forEach((element) => {
                const htmlElement = element as HTMLElement;
                htmlElement.style.backgroundColor = 'var(--color-bg-primary)';
              });
            });
            
            if (editor) {
              editor.layout();
              
              const originalEditor = editor.getOriginalEditor();
              const modifiedEditor = editor.getModifiedEditor();
              originalEditor.layout();
              modifiedEditor.layout();
            }
          }
        });

        if (!readOnly) {
          contentChangeListenerRef.current = modifiedModel.onDidChangeContent(() => {
            if (!isUnmountedRef.current) {
              const newContent = modifiedModel!.getValue();
              onModifiedContentChange?.(newContent);
            }
          });

          const modifiedEditor = editor.getModifiedEditor();
          modifiedEditor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
            const content = modifiedModel!.getValue();
            onSaveRef.current?.(content);
          });
        }

        const diffUpdateDisposable = editor.onDidUpdateDiff(() => {
          if (isUnmountedRef.current || !editor) return;
          
          const lineChanges = editor.getLineChanges();
          
          if (lineChanges) {
            setChanges(lineChanges);
            onDiffChange?.(lineChanges);
          } else {
            setChanges([]);
            onDiffChange?.([]);
          }
        });

        changeListenerRef.current = diffUpdateDisposable;

        setLoading(false);

      } catch (err) {
        log.error('Failed to initialize DiffEditor', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(t('editor.diffEditor.openFailed'));
        setLoading(false);
        notification.error(t('editor.diffEditor.initFailedWithMessage', { message: errorMessage }));
      }
    };

    initDiffEditor();

    return () => {
      isUnmountedRef.current = true;

      if (changeListenerRef.current) {
        changeListenerRef.current.dispose();
        changeListenerRef.current = null;
      }
      
      if (contentChangeListenerRef.current) {
        contentChangeListenerRef.current.dispose();
        contentChangeListenerRef.current = null;
      }

      if (editor) {
        editor.dispose();
      }

      if (originalModel) {
        originalModel.dispose();
      }

      if (modifiedModel) {
        modifiedModel.dispose();
      }

      originalModelRef.current = null;
      modifiedModelRef.current = null;
      setModifiedEditorInstance(null);
    };
  }, [filePath, detectedLanguage, renderSideBySide, readOnly, showMinimap]);

  useEffect(() => {
    if (originalModelRef.current && originalModelRef.current.getValue() !== originalContent) {
      originalModelRef.current.setValue(originalContent);
    }
  }, [originalContent]);

  useEffect(() => {
    if (modifiedModelRef.current && modifiedModelRef.current.getValue() !== modifiedContent) {
      modifiedModelRef.current.setValue(modifiedContent);
    }
  }, [modifiedContent]);

  // Reveal a specific line in the modified editor (1-based)
  useEffect(() => {
    if (!diffEditor || !revealLine || revealLine < 1) return;
    try {
      const modifiedEditor = diffEditor.getModifiedEditor();
      modifiedEditor.revealLineInCenter(revealLine);
      modifiedEditor.setPosition({ lineNumber: revealLine, column: 1 });
    } catch (error) {
      log.warn('Failed to reveal line', error);
    }
  }, [diffEditor, revealLine]);

  useEffect(() => {
    if (!diffEditor) {
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
            
            try {
              diffEditor.updateOptions({});
            } catch (error) {
              log.warn('Failed to update diff editor options', error);
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
  }, [diffEditor]);

  const navigateToNextChange = useCallback(() => {
    if (!diffEditor) return;
    
    const modifiedEditor = diffEditor.getModifiedEditor();
    if (modifiedEditor) {
      const action = modifiedEditor.getAction('editor.action.diffReview.next');
      if (action) {
        action.run();
      }
    }
  }, [diffEditor]);

  const navigateToPrevChange = useCallback(() => {
    if (diffEditor) {
      const modifiedEditor = diffEditor.getModifiedEditor();
      if (modifiedEditor) {
        const action = modifiedEditor.getAction('editor.action.diffReview.prev');
        if (action) {
          action.run();
        }
      }
    }
  }, [diffEditor]);

  const toggleViewMode = useCallback(() => {
    if (diffEditor) {
      diffEditor.updateOptions({
        renderSideBySide: !renderSideBySide
      });
    }
  }, [diffEditor, renderSideBySide]);

  const diffStats = useMemo(() => {
    let additions = 0;
    let deletions = 0;
    
    changes.forEach(change => {
      if (change.modifiedEndLineNumber && change.modifiedStartLineNumber) {
        const modifiedLines = change.modifiedEndLineNumber - change.modifiedStartLineNumber + 1;
        if (!change.originalEndLineNumber || change.originalStartLineNumber > change.originalEndLineNumber) {
          additions += modifiedLines;
        } else {
          const originalLines = change.originalEndLineNumber - change.originalStartLineNumber + 1;
          if (modifiedLines > originalLines) {
            additions += modifiedLines - originalLines;
          }
        }
      }
      
      if (change.originalEndLineNumber && change.originalStartLineNumber) {
        const originalLines = change.originalEndLineNumber - change.originalStartLineNumber + 1;
        if (!change.modifiedEndLineNumber || change.modifiedStartLineNumber > change.modifiedEndLineNumber) {
          deletions += originalLines;
        } else {
          const modifiedLines = change.modifiedEndLineNumber - change.modifiedStartLineNumber + 1;
          if (originalLines > modifiedLines) {
            deletions += originalLines - modifiedLines;
          }
        }
      }
    });
    
    return { additions, deletions, total: changes.length };
  }, [changes]);

  useEffect(() => {
    if (!diffEditor || !enableCustomToolbar) return;
    
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if ((e.altKey && e.key === 'ArrowDown') || e.key === 'F7') {
        e.preventDefault();
        navigateToNextChange();
      }
      if ((e.altKey && e.key === 'ArrowUp') || (e.shiftKey && e.key === 'F7')) {
        e.preventDefault();
        navigateToPrevChange();
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [diffEditor, enableCustomToolbar, navigateToNextChange, navigateToPrevChange]);

  const renderToolbar = () => {
    if (!enableCustomToolbar) return null;

    return (
      <div className="diff-editor-toolbar">
        <div className="diff-editor-toolbar__info">
          <span className="diff-editor-toolbar__stats">
            {diffStats.additions > 0 && (
              <span className="diff-editor-toolbar__stat diff-editor-toolbar__stat--add">
                +{diffStats.additions}
              </span>
            )}
            {diffStats.deletions > 0 && (
              <span className="diff-editor-toolbar__stat diff-editor-toolbar__stat--del">
                -{diffStats.deletions}
              </span>
            )}
            <span className="diff-editor-toolbar__changes-count">
              {t('editor.diffEditor.changesCount', { count: diffStats.total })}
            </span>
          </span>
          {filePath && (
            <span className="diff-editor-toolbar__file-path">{filePath}</span>
          )}
        </div>
        
        <div className="diff-editor-toolbar__actions">
          <Tooltip content={t('editor.diffEditor.prevChange')} placement="top">
            <button
              className="diff-editor-toolbar__btn"
              onClick={navigateToPrevChange}
              disabled={changes.length === 0}
            >
              ↑
            </button>
          </Tooltip>
          <Tooltip content={t('editor.diffEditor.nextChange')} placement="top">
            <button
              className="diff-editor-toolbar__btn"
              onClick={navigateToNextChange}
              disabled={changes.length === 0}
            >
              ↓
            </button>
          </Tooltip>
          <Tooltip
            content={renderSideBySide ? t('editor.diffEditor.switchToInline') : t('editor.diffEditor.switchToSideBySide')}
            placement="top"
          >
            <button
              className="diff-editor-toolbar__btn"
              onClick={toggleViewMode}
            >
              {renderSideBySide ? '⊟' : '⊞'}
            </button>
          </Tooltip>
        </div>
      </div>
    );
  };

  return (
    <div className={`diff-editor-container ${className}`}>
      {renderToolbar()}
      
      <div className="diff-editor-wrapper">
        <div 
          ref={containerRef} 
          className="diff-editor-content"
          style={{ 
            width: '100%', 
            height: '100%',
            opacity: loading ? 0.3 : 1,
            transition: 'opacity 0.2s'
          }} 
        />
      </div>

      {loading && (
        <div className="diff-editor-loading-overlay">
          <CubeLoading size="medium" text={t('editor.diffEditor.loading')} />
        </div>
      )}

      {error && (
        <div className="diff-editor-error">
          <AlertCircle size={32} className="diff-editor-error__icon" />
          <p className="diff-editor-error__message">{error}</p>
          {filePath && (
            <p className="diff-editor-error__path">{filePath}</p>
          )}
        </div>
      )}
    </div>
  );
};

export default DiffEditor;

