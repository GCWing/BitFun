/**
 * Monaco editor core component.
 *
 * Wraps monaco.editor.create(), integrates with MonacoModelManager,
 * exposes editor ref, proxies events, and calls ExtensionManager lifecycle hooks.
 *
 * Does not include: file IO, LSP integration (via Extension), UI components.
 */

import { useEffect, useRef, useCallback, useState, forwardRef, useImperativeHandle } from 'react';
import * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';
import { monacoInitManager } from '../services/MonacoInitManager';
import { monacoModelManager } from '../services/MonacoModelManager';
import { themeManager } from '../services/ThemeManager';
import { editorExtensionManager } from '../services/EditorExtensionManager';
import { buildEditorOptions } from '../services/EditorOptionsBuilder';
import { activeEditTargetService, createMonacoEditTarget } from '../services/ActiveEditTargetService';
import type { MonacoEditorCoreProps } from './types';
import type { EditorExtensionContext } from '../services/EditorExtensionManager';
import type { EditorOptionsOverrides } from '../services/EditorOptionsBuilder';
import type { LineRange } from '@/component-library/components/Markdown';

const log = createLogger('MonacoEditorCore');

export interface MonacoEditorCoreRef {
  getEditor(): monaco.editor.IStandaloneCodeEditor | null;
  getModel(): monaco.editor.ITextModel | null;
  getContent(): string;
  setContent(content: string): void;
  revealPosition(line: number, column?: number): void;
  focus(): void;
  executeCommand(commandId: string): void;
  updateOptions(options: monaco.editor.IEditorOptions): void;
}

export const MonacoEditorCore = forwardRef<MonacoEditorCoreRef, MonacoEditorCoreProps>(
  (props, ref) => {
    const {
      filePath,
      workspacePath,
      language = 'plaintext',
      initialContent = '',
      preset = 'standard',
      config,
      readOnly = false,
      theme,
      enableLsp = true,
      showLineNumbers = true,
      showMinimap = true,
      onContentChange,
      onCursorChange,
      onSelectionChange,
      onEditorReady,
      onEditorWillDispose,
      onSave,
      className = '',
      style,
      jumpToLine,
      jumpToColumn,
      jumpToRange,
    } = props;
    
    const containerRef = useRef<HTMLDivElement>(null);
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const modelRef = useRef<monaco.editor.ITextModel | null>(null);
    const editorIdRef = useRef<string>('');
    const isUnmountedRef = useRef(false);
    const disposablesRef = useRef<monaco.IDisposable[]>([]);
    const macosEditorBindingCleanupRef = useRef<(() => void) | null>(null);
    const hasJumpedRef = useRef(false);
    
    const [isReady, setIsReady] = useState(false);
    
    useImperativeHandle(ref, () => ({
      getEditor: () => editorRef.current,
      getModel: () => modelRef.current,
      getContent: () => modelRef.current?.getValue() || '',
      setContent: (content: string) => {
        if (modelRef.current) {
          modelRef.current.setValue(content);
        }
      },
      revealPosition: (line: number, column: number = 1) => {
        if (editorRef.current) {
          editorRef.current.revealLineInCenter(line);
          editorRef.current.setPosition({ lineNumber: line, column });
        }
      },
      focus: () => {
        editorRef.current?.focus();
      },
      executeCommand: (commandId: string) => {
        editorRef.current?.trigger('api', commandId, null);
      },
      updateOptions: (options: monaco.editor.IEditorOptions) => {
        editorRef.current?.updateOptions(options);
      },
    }), []);
    
    const createExtensionContext = useCallback((): EditorExtensionContext => {
      return {
        filePath,
        language,
        workspacePath,
        readOnly,
        enableLsp,
      };
    }, [filePath, language, workspacePath, readOnly, enableLsp]);
    
    useEffect(() => {
      if (!containerRef.current) return;
      
      isUnmountedRef.current = false;
      hasJumpedRef.current = false;
      
      const initEditor = async () => {
        try {
          await monacoInitManager.initialize();
          
          if (isUnmountedRef.current || !containerRef.current) return;
          
          themeManager.initialize();
          
          const model = monacoModelManager.getOrCreateModel(
            filePath,
            language,
            initialContent,
            workspacePath
          );
          modelRef.current = model;
          
          const overrides: EditorOptionsOverrides = {
            readOnly,
            lineNumbers: showLineNumbers,
            minimap: showMinimap,
            theme,
          };
          
          const editorOptions = buildEditorOptions({
            config,
            preset,
            overrides,
          });
          
          const editor = monaco.editor.create(containerRef.current, {
            ...editorOptions,
            model,
          });
          editorRef.current = editor;
          const editTarget = createMonacoEditTarget(editor);
          const unbindEditTarget = activeEditTargetService.bindTarget(editTarget);
          const focusDisposable = editor.onDidFocusEditorText(() => {
            activeEditTargetService.setActiveTarget(editTarget.id);
          });
          const blurDisposable = editor.onDidBlurEditorText(() => {
            window.setTimeout(() => {
              if (editor.hasTextFocus()) {
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
          
          registerEventListeners(editor, model);
          
          if (onSave) {
            editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
              const content = model.getValue();
              onSave(content);
            });
          }
          
          const context = createExtensionContext();
          editorIdRef.current = editorExtensionManager.notifyEditorCreated(editor, model, context);
          
          setIsReady(true);
          
          if (onEditorReady) {
            onEditorReady(editor, model);
          }
          
        } catch (error) {
          log.error('Failed to initialize editor', error);
        }
      };
      
      initEditor();
      
      return () => {
        isUnmountedRef.current = true;
        
        if (onEditorWillDispose) {
          onEditorWillDispose();
        }
        
        if (editorRef.current && modelRef.current && editorIdRef.current) {
          const context = createExtensionContext();
          editorExtensionManager.notifyEditorWillDispose(
            editorIdRef.current,
            editorRef.current,
            modelRef.current,
            context
          );
        }
        
        disposablesRef.current.forEach(d => d.dispose());
        disposablesRef.current = [];

        if (macosEditorBindingCleanupRef.current) {
          macosEditorBindingCleanupRef.current();
          macosEditorBindingCleanupRef.current = null;
        }
        
        if (editorRef.current) {
          editorRef.current.dispose();
          editorRef.current = null;
        }
        
        if (modelRef.current) {
          monacoModelManager.releaseModel(filePath);
          modelRef.current = null;
        }
        
        setIsReady(false);
      };
    }, [filePath]);
    
    const registerEventListeners = useCallback((
      editor: monaco.editor.IStandaloneCodeEditor,
      model: monaco.editor.ITextModel
    ) => {
      const contentDisposable = model.onDidChangeContent((event) => {
        if (onContentChange) {
          onContentChange(model.getValue(), event);
        }
        
        const context = createExtensionContext();
        editorExtensionManager.notifyContentChanged(editor, model, event, context);
      });
      disposablesRef.current.push(contentDisposable);
      
      const cursorDisposable = editor.onDidChangeCursorPosition((e) => {
        if (onCursorChange) {
          onCursorChange(e.position);
        }
      });
      disposablesRef.current.push(cursorDisposable);
      
      const selectionDisposable = editor.onDidChangeCursorSelection((e) => {
        if (onSelectionChange) {
          onSelectionChange(e.selection);
        }
      });
      disposablesRef.current.push(selectionDisposable);
    }, [onContentChange, onCursorChange, onSelectionChange, createExtensionContext]);
    
    useEffect(() => {
      if (!isReady || !editorRef.current || hasJumpedRef.current) return;
      
      // Prefer jumpToRange, fallback to jumpToLine for backward compatibility
      const finalRange: LineRange | undefined = jumpToRange || (jumpToLine && jumpToLine > 0 ? { start: jumpToLine, end: jumpToColumn ? jumpToLine : undefined } : undefined);
      
      if (finalRange) {
        const line = finalRange.start;
        const endLine = finalRange.end;
        const column = 1;
        
        editorRef.current.setPosition({ lineNumber: line, column });
        
        if (endLine && endLine > line && modelRef.current) {
          const endLineMaxColumn = modelRef.current.getLineMaxColumn(endLine);
          editorRef.current.setSelection({
            startLineNumber: line,
            startColumn: 1,
            endLineNumber: endLine,
            endColumn: endLineMaxColumn
          });
          editorRef.current.revealRangeInCenter({
            startLineNumber: line,
            startColumn: 1,
            endLineNumber: endLine,
            endColumn: endLineMaxColumn
          });
        } else {
          editorRef.current.revealLineInCenter(line);
        }
        
        editorRef.current.focus();
        
        hasJumpedRef.current = true;
      }
    }, [isReady, jumpToRange, jumpToLine, jumpToColumn]);
    
    useEffect(() => {
      if (!editorRef.current) return;
      
      const overrides: EditorOptionsOverrides = {
        readOnly,
        lineNumbers: showLineNumbers,
        minimap: showMinimap,
        theme,
      };
      
      const editorOptions = buildEditorOptions({
        config,
        preset,
        overrides,
      });
      
      editorRef.current.updateOptions(editorOptions);
    }, [config, preset, readOnly, showLineNumbers, showMinimap, theme]);
    
    useEffect(() => {
      const unsubscribe = themeManager.onThemeChange((event) => {
        if (editorRef.current) {
          monaco.editor.setTheme(event.currentThemeId);
        }
      });
      
      return unsubscribe;
    }, []);
    
    return (
      <div
        ref={containerRef}
        className={`monaco-editor-core ${className}`}
        style={{
          width: '100%',
          height: '100%',
          overflow: 'hidden',
          ...style,
        }}
      />
    );
  }
);

MonacoEditorCore.displayName = 'MonacoEditorCore';

export default MonacoEditorCore;
