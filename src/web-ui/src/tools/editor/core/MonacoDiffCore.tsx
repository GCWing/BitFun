/**
 * Monaco Diff editor core component.
 *
 * Wraps monaco.editor.createDiffEditor(), manages original/modified Models,
 * and provides diff navigation API.
 *
 * Does not include: file IO, custom UI controls.
 */

import { useEffect, useRef, useState, forwardRef, useImperativeHandle, useCallback } from 'react';
import * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';
import { monacoInitManager } from '../services/MonacoInitManager';
import { themeManager } from '../services/ThemeManager';
import { buildDiffEditorOptions } from '../services/EditorOptionsBuilder';
import type { MonacoDiffCoreProps } from './types';
import type { EditorOptionsOverrides } from '../services/EditorOptionsBuilder';

const log = createLogger('MonacoDiffCore');

export interface MonacoDiffCoreRef {
  getDiffEditor(): monaco.editor.IStandaloneDiffEditor | null;
  getOriginalModel(): monaco.editor.ITextModel | null;
  getModifiedModel(): monaco.editor.ITextModel | null;
  getOriginalEditor(): monaco.editor.ICodeEditor | null;
  getModifiedEditor(): monaco.editor.ICodeEditor | null;
  getModifiedContent(): string;
  setModifiedContent(content: string): void;
  getChanges(): monaco.editor.ILineChange[];
  goToNextDiff(): void;
  goToPreviousDiff(): void;
  revealLine(line: number): void;
  focus(): void;
}

export const MonacoDiffCore = forwardRef<MonacoDiffCoreRef, MonacoDiffCoreProps>(
  (props, ref) => {
    const {
      originalContent,
      modifiedContent,
      filePath,
      workspacePath: _workspacePath,
      language = 'plaintext',
      preset = 'diff',
      config,
      readOnly = false,
      theme,
      renderSideBySide = true,
      renderOverviewRuler = false,
      renderIndicators = true,
      originalEditable = false,
      ignoreTrimWhitespace = false,
      showMinimap = false,
      onModifiedContentChange,
      onDiffChange,
      onEditorReady,
      onEditorWillDispose,
      className = '',
      style,
      revealLine,
    } = props;
    
    const containerRef = useRef<HTMLDivElement>(null);
    const diffEditorRef = useRef<monaco.editor.IStandaloneDiffEditor | null>(null);
    const originalModelRef = useRef<monaco.editor.ITextModel | null>(null);
    const modifiedModelRef = useRef<monaco.editor.ITextModel | null>(null);
    const changesRef = useRef<monaco.editor.ILineChange[]>([]);
    const currentDiffIndexRef = useRef(0);
    const isUnmountedRef = useRef(false);
    const disposablesRef = useRef<monaco.IDisposable[]>([]);
    const hasRevealedRef = useRef(false);
    const filePathRef = useRef(filePath);
    const originalContentRef = useRef(originalContent);
    const modifiedContentRef = useRef(modifiedContent);
    const languageRef = useRef(language);
    const presetRef = useRef(preset);
    const configRef = useRef(config);
    const readOnlyRef = useRef(readOnly);
    const themeRef = useRef(theme);
    const renderSideBySideRef = useRef(renderSideBySide);
    const renderOverviewRulerRef = useRef(renderOverviewRuler);
    const renderIndicatorsRef = useRef(renderIndicators);
    const originalEditableRef = useRef(originalEditable);
    const ignoreTrimWhitespaceRef = useRef(ignoreTrimWhitespace);
    const showMinimapRef = useRef(showMinimap);
    const onModifiedContentChangeRef = useRef(onModifiedContentChange);
    const onDiffChangeRef = useRef(onDiffChange);
    const onEditorReadyRef = useRef(onEditorReady);
    const onEditorWillDisposeRef = useRef(onEditorWillDispose);
    
    const [isReady, setIsReady] = useState(false);

    filePathRef.current = filePath;
    originalContentRef.current = originalContent;
    modifiedContentRef.current = modifiedContent;
    languageRef.current = language;
    presetRef.current = preset;
    configRef.current = config;
    readOnlyRef.current = readOnly;
    themeRef.current = theme;
    renderSideBySideRef.current = renderSideBySide;
    renderOverviewRulerRef.current = renderOverviewRuler;
    renderIndicatorsRef.current = renderIndicators;
    originalEditableRef.current = originalEditable;
    ignoreTrimWhitespaceRef.current = ignoreTrimWhitespace;
    showMinimapRef.current = showMinimap;
    onModifiedContentChangeRef.current = onModifiedContentChange;
    onDiffChangeRef.current = onDiffChange;
    onEditorReadyRef.current = onEditorReady;
    onEditorWillDisposeRef.current = onEditorWillDispose;
    
    useImperativeHandle(ref, () => ({
      getDiffEditor: () => diffEditorRef.current,
      getOriginalModel: () => originalModelRef.current,
      getModifiedModel: () => modifiedModelRef.current,
      getOriginalEditor: () => diffEditorRef.current?.getOriginalEditor() || null,
      getModifiedEditor: () => diffEditorRef.current?.getModifiedEditor() || null,
      getModifiedContent: () => modifiedModelRef.current?.getValue() || '',
      setModifiedContent: (content: string) => {
        if (modifiedModelRef.current) {
          modifiedModelRef.current.setValue(content);
        }
      },
      getChanges: () => changesRef.current,
      goToNextDiff: () => {
        if (!diffEditorRef.current || changesRef.current.length === 0) return;
        
        currentDiffIndexRef.current = (currentDiffIndexRef.current + 1) % changesRef.current.length;
        const change = changesRef.current[currentDiffIndexRef.current];
        if (change) {
          const line = change.modifiedStartLineNumber || 1;
          diffEditorRef.current.getModifiedEditor()?.revealLineInCenter(line);
        }
      },
      goToPreviousDiff: () => {
        if (!diffEditorRef.current || changesRef.current.length === 0) return;
        
        currentDiffIndexRef.current = currentDiffIndexRef.current <= 0 
          ? changesRef.current.length - 1 
          : currentDiffIndexRef.current - 1;
        const change = changesRef.current[currentDiffIndexRef.current];
        if (change) {
          const line = change.modifiedStartLineNumber || 1;
          diffEditorRef.current.getModifiedEditor()?.revealLineInCenter(line);
        }
      },
      revealLine: (line: number) => {
        diffEditorRef.current?.getModifiedEditor()?.revealLineInCenter(line);
      },
      focus: () => {
        diffEditorRef.current?.getModifiedEditor()?.focus();
      },
    }), []);
    
    const generateUri = useCallback((type: 'original' | 'modified'): monaco.Uri => {
      const timestamp = Date.now();
      const random = Math.random().toString(36).substring(2, 8);
      const basePath = filePathRef.current || 'untitled';
      return monaco.Uri.parse(`inmemory://diff/${type}/${timestamp}/${random}/${basePath}`);
    }, []);

    const registerEventListeners = useCallback((
      diffEditor: monaco.editor.IStandaloneDiffEditor,
      modifiedModel: monaco.editor.ITextModel
    ) => {
      const contentDisposable = modifiedModel.onDidChangeContent(() => {
        onModifiedContentChangeRef.current?.(modifiedModel.getValue());
      });
      disposablesRef.current.push(contentDisposable);

      const diffDisposable = diffEditor.onDidUpdateDiff(() => {
        const changes = diffEditor.getLineChanges() || [];
        changesRef.current = changes;

        onDiffChangeRef.current?.(changes);
      });
      disposablesRef.current.push(diffDisposable);
    }, []);

    useEffect(() => {
      if (!containerRef.current) return;
      
      isUnmountedRef.current = false;
      hasRevealedRef.current = false;
      const container = containerRef.current;
      
      const initEditor = async () => {
        try {
          await monacoInitManager.initialize();
          
          if (isUnmountedRef.current) return;
          
          themeManager.initialize();
          
          const originalModel = monaco.editor.createModel(
            originalContentRef.current,
            languageRef.current,
            generateUri('original')
          );
          const modifiedModel = monaco.editor.createModel(
            modifiedContentRef.current,
            languageRef.current,
            generateUri('modified')
          );
          originalModelRef.current = originalModel;
          modifiedModelRef.current = modifiedModel;
          
          const overrides: EditorOptionsOverrides = {
            readOnly: readOnlyRef.current,
            minimap: showMinimapRef.current,
            theme: themeRef.current,
          };
          
          const diffOptions = buildDiffEditorOptions({
            config: configRef.current,
            preset: presetRef.current,
            overrides,
          });
          
          const diffEditor = monaco.editor.createDiffEditor(container, {
            ...diffOptions,
            renderSideBySide: renderSideBySideRef.current,
            renderOverviewRuler: renderOverviewRulerRef.current,
            renderIndicators: renderIndicatorsRef.current,
            originalEditable: originalEditableRef.current,
            ignoreTrimWhitespace: ignoreTrimWhitespaceRef.current,
          });
          diffEditorRef.current = diffEditor;
          
          diffEditor.setModel({
            original: originalModel,
            modified: modifiedModel,
          });
          
          registerEventListeners(diffEditor, modifiedModel);
          
          setIsReady(true);
          
          onEditorReadyRef.current?.(diffEditor, originalModel, modifiedModel);
          
        } catch (error) {
          log.error('Failed to initialize diff editor', error);
        }
      };
      
      initEditor();
      
      return () => {
        isUnmountedRef.current = true;
        
        onEditorWillDisposeRef.current?.();
        
        disposablesRef.current.forEach(d => d.dispose());
        disposablesRef.current = [];
        
        if (diffEditorRef.current) {
          diffEditorRef.current.dispose();
          diffEditorRef.current = null;
        }
        
        if (originalModelRef.current) {
          originalModelRef.current.dispose();
          originalModelRef.current = null;
        }
        if (modifiedModelRef.current) {
          modifiedModelRef.current.dispose();
          modifiedModelRef.current = null;
        }
        
        setIsReady(false);
      };
    }, [filePath, generateUri, registerEventListeners]);
    
    useEffect(() => {
      if (!isReady) return;
      
      if (originalModelRef.current && originalModelRef.current.getValue() !== originalContent) {
        originalModelRef.current.setValue(originalContent);
      }
      if (modifiedModelRef.current && modifiedModelRef.current.getValue() !== modifiedContent) {
        modifiedModelRef.current.setValue(modifiedContent);
      }
    }, [isReady, originalContent, modifiedContent]);
    
    useEffect(() => {
      if (!isReady) return;
      
      if (originalModelRef.current) {
        monaco.editor.setModelLanguage(originalModelRef.current, language);
      }
      if (modifiedModelRef.current) {
        monaco.editor.setModelLanguage(modifiedModelRef.current, language);
      }
    }, [isReady, language]);
    
    useEffect(() => {
      if (!isReady || !diffEditorRef.current || hasRevealedRef.current) return;
      
      if (revealLine && revealLine > 0) {
        diffEditorRef.current.getModifiedEditor()?.revealLineInCenter(revealLine);
        hasRevealedRef.current = true;
      }
    }, [isReady, revealLine]);
    
    useEffect(() => {
      const unsubscribe = themeManager.onThemeChange((event) => {
        monaco.editor.setTheme(event.currentThemeId);
      });
      
      return unsubscribe;
    }, []);
    
    return (
      <div
        ref={containerRef}
        className={`monaco-diff-core ${className}`}
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

MonacoDiffCore.displayName = 'MonacoDiffCore';

export default MonacoDiffCore;
