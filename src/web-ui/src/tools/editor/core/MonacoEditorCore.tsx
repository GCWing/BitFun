/**
 * Monaco editor core component.
 *
 * Wraps monaco.editor.create(), integrates with MonacoModelManager,
 * exposes editor ref, proxies events, and calls ExtensionManager lifecycle hooks.
 *
 * Does not include: file IO, UI components.
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
    const filePathRef = useRef(filePath);
    const workspacePathRef = useRef(workspacePath);
    const languageRef = useRef(language);
    const initialContentRef = useRef(initialContent);
    const presetRef = useRef(preset);
    const configRef = useRef(config);
    const readOnlyRef = useRef(readOnly);
    const themeRef = useRef(theme);
    const showLineNumbersRef = useRef(showLineNumbers);
    const showMinimapRef = useRef(showMinimap);
    const onContentChangeRef = useRef(onContentChange);
    const onCursorChangeRef = useRef(onCursorChange);
    const onSelectionChangeRef = useRef(onSelectionChange);
    const onEditorReadyRef = useRef(onEditorReady);
    const onEditorWillDisposeRef = useRef(onEditorWillDispose);
    const onSaveRef = useRef(onSave);
    
    const [isReady, setIsReady] = useState(false);

    filePathRef.current = filePath;
    workspacePathRef.current = workspacePath;
    languageRef.current = language;
    initialContentRef.current = initialContent;
    presetRef.current = preset;
    configRef.current = config;
    readOnlyRef.current = readOnly;
    themeRef.current = theme;
    showLineNumbersRef.current = showLineNumbers;
    showMinimapRef.current = showMinimap;
    onContentChangeRef.current = onContentChange;
    onCursorChangeRef.current = onCursorChange;
    onSelectionChangeRef.current = onSelectionChange;
    onEditorReadyRef.current = onEditorReady;
    onEditorWillDisposeRef.current = onEditorWillDispose;
    onSaveRef.current = onSave;
    
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
    
    const createExtensionContext = useCallback((overrides?: Partial<EditorExtensionContext>): EditorExtensionContext => {
      return {
        filePath: overrides?.filePath ?? filePathRef.current,
        language: overrides?.language ?? languageRef.current,
        workspacePath: overrides?.workspacePath ?? workspacePathRef.current,
        readOnly: overrides?.readOnly ?? readOnlyRef.current,
      };
    }, []);

    const registerEventListeners = useCallback((
      editor: monaco.editor.IStandaloneCodeEditor,
      model: monaco.editor.ITextModel
    ) => {
      const contentDisposable = model.onDidChangeContent((event) => {
        onContentChangeRef.current?.(model.getValue(), event);

        const context = createExtensionContext();
        editorExtensionManager.notifyContentChanged(editor, model, event, context);
      });
      disposablesRef.current.push(contentDisposable);

      const cursorDisposable = editor.onDidChangeCursorPosition((e) => {
        onCursorChangeRef.current?.(e.position);
      });
      disposablesRef.current.push(cursorDisposable);

      const selectionDisposable = editor.onDidChangeCursorSelection((e) => {
        onSelectionChangeRef.current?.(e.selection);
      });
      disposablesRef.current.push(selectionDisposable);
    }, [createExtensionContext]);
    
    useEffect(() => {
      if (!containerRef.current) return;
      
      isUnmountedRef.current = false;
      hasJumpedRef.current = false;
      const container = containerRef.current;
      const currentFilePath = filePath;
      const initialContext = createExtensionContext({
        filePath: currentFilePath,
        language: languageRef.current,
        workspacePath: workspacePathRef.current,
        readOnly: readOnlyRef.current,
      });
      
      const initEditor = async () => {
        try {
          await monacoInitManager.initialize();
          
          if (isUnmountedRef.current) return;
          
          themeManager.initialize();
          
          const model = monacoModelManager.getOrCreateModel(
            currentFilePath,
            languageRef.current,
            initialContentRef.current,
            workspacePathRef.current
          );
          modelRef.current = model;
          
          const overrides: EditorOptionsOverrides = {
            readOnly: readOnlyRef.current,
            lineNumbers: showLineNumbersRef.current,
            minimap: showMinimapRef.current,
            theme: themeRef.current,
          };
          
          const editorOptions = buildEditorOptions({
            config: configRef.current,
            preset: presetRef.current,
            overrides,
          });
          
          const editor = monaco.editor.create(container, {
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
          
          editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
            const content = model.getValue();
            onSaveRef.current?.(content);
          });
          
          editorIdRef.current = editorExtensionManager.notifyEditorCreated(editor, model, initialContext);
          
          setIsReady(true);
          
          onEditorReadyRef.current?.(editor, model);
          
        } catch (error) {
          log.error('Failed to initialize editor', error);
        }
      };
      
      initEditor();
      
      return () => {
        isUnmountedRef.current = true;
        
        onEditorWillDisposeRef.current?.();
        
        if (editorRef.current && modelRef.current && editorIdRef.current) {
          editorExtensionManager.notifyEditorWillDispose(
            editorIdRef.current,
            editorRef.current,
            modelRef.current,
            initialContext
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
          monacoModelManager.releaseModel(currentFilePath);
          modelRef.current = null;
        }
        
        setIsReady(false);
      };
    }, [filePath, createExtensionContext, registerEventListeners]);
    
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
      if (!isReady || !modelRef.current) {
        return;
      }

      if (modelRef.current.getLanguageId() !== language) {
        monaco.editor.setModelLanguage(modelRef.current, language);
      }
    }, [isReady, language]);
    
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
