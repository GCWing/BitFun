/**
 * CodeEditor component
 * Code editor based on MonacoEditorCore with syntax highlighting, themes, and fullscreen toggle
 */

import React, { useRef, useState, useEffect, useCallback, useMemo } from 'react';
import * as monaco from 'monaco-editor';
import { useI18n } from '@/infrastructure/i18n';
import { MonacoEditorCore, type MonacoEditorCoreRef } from '@/tools/editor/core';
import type { EditorConfigPartial } from '@/tools/editor/config';
import './CodeEditor.scss';

export interface CodeEditorProps {
  /** Code content */
  value?: string;
  /** Programming language */
  language?: string;
  /** Theme */
  theme?: 'vs-dark' | 'vs-light' | 'hc-black' | 'bitfun-dark' | 'bitfun-light';
  /** Read-only */
  readOnly?: boolean;
  /** Show line numbers */
  lineNumbers?: 'on' | 'off' | 'relative' | 'interval';
  /** Show minimap */
  minimap?: boolean;
  /** Height */
  height?: string | number;
  /** Width */
  width?: string | number;
  /** Word wrap */
  wordWrap?: 'on' | 'off' | 'wordWrapColumn' | 'bounded';
  /** Font size */
  fontSize?: number;
  /** Tab size */
  tabSize?: number;
  /** Content change callback */
  onChange?: (value: string | undefined) => void;
  /** Editor mount callback */
  onMount?: (editor: monaco.editor.IStandaloneCodeEditor, monaco: typeof import('monaco-editor')) => void;
  /** Custom class name */
  className?: string;
  /** Placeholder text */
  placeholder?: string;
  /** Show fullscreen button */
  showFullscreenButton?: boolean;
}

export const CodeEditor: React.FC<CodeEditorProps> = ({
  value = '',
  language = 'typescript',
  theme = 'bitfun-dark',
  readOnly = false,
  lineNumbers = 'on',
  minimap = true,
  height = '500px',
  width = '100%',
  wordWrap = 'off',
  fontSize = 16,
  tabSize = 2,
  onChange,
  onMount,
  className = '',
  placeholder = '// Enter code here...',
  showFullscreenButton = true,
}) => {
  const { t } = useI18n('components');
  const editorCoreRef = useRef<MonacoEditorCoreRef>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  
  const filePath = useMemo(() => {
    const timestamp = Date.now();
    const random = Math.random().toString(36).substring(2, 8);
    return `inmemory://component-library/${timestamp}/${random}/code.${getExtensionFromLanguage(language)}`;
  }, [language]);
  
  const config: EditorConfigPartial = useMemo(() => ({
    fontSize,
    tabSize,
    wordWrap,
    lineNumbers: lineNumbers as 'on' | 'off' | 'relative' | 'interval',
    minimap: {
      enabled: minimap,
      side: 'right',
      size: 'proportional',
    },
    smoothScrolling: true,
    scrollBeyondLastLine: false,
    bracketPairColorization: true,
    semanticHighlighting: true,
    guides: {
      indentation: true,
      bracketPairs: true,
      bracketPairsHorizontal: 'active',
      highlightActiveBracketPair: true,
      highlightActiveIndentation: true,
    },
  }), [fontSize, tabSize, wordWrap, lineNumbers, minimap]);
  
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isFullscreen) {
        setIsFullscreen(false);
      }
    };

    if (isFullscreen) {
      document.addEventListener('keydown', handleEscape);
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.removeEventListener('keydown', handleEscape);
      document.body.style.overflow = '';
    };
  }, [isFullscreen]);
  
  const handleEditorReady = useCallback((
    editor: monaco.editor.IStandaloneCodeEditor,
    _model: monaco.editor.ITextModel
  ) => {
    if (!value && placeholder) {
      editor.setValue(placeholder);
      editor.setSelection({ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 1 });
    }
    
    if (onMount) {
      onMount(editor, monaco);
    }
  }, [value, placeholder, onMount]);
  
  const handleContentChange = useCallback((content: string) => {
    if (onChange) {
      onChange(content);
    }
  }, [onChange]);
  
  const toggleFullscreen = () => {
    setIsFullscreen(!isFullscreen);
  };
  
  const classNames = [
    'code-editor',
    isFullscreen && 'code-editor--fullscreen',
    className
  ]
    .filter(Boolean)
    .join(' ');
  
  const computedHeight = isFullscreen ? '100vh' : (typeof height === 'number' ? `${height}px` : height);
  const computedWidth = typeof width === 'number' ? `${width}px` : width;

  return (
    <div className={classNames}>
      <div 
        className="code-editor__wrapper"
        style={{ height: computedHeight, width: computedWidth }}
      >
        {showFullscreenButton && (
          <button
            className="code-editor__fullscreen-btn"
            onClick={toggleFullscreen}
            title={isFullscreen ? t('codeEditor.exitFullscreenHint') : t('codeEditor.enterFullscreen')}
            aria-label={isFullscreen ? t('codeEditor.exitFullscreen') : t('codeEditor.enterFullscreen')}
          >
            {isFullscreen ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3" />
              </svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
              </svg>
            )}
          </button>
        )}
        <MonacoEditorCore
          ref={editorCoreRef}
          filePath={filePath}
          language={language}
          initialContent={value}
          preset={readOnly ? 'readonly' : 'standard'}
          config={config}
          readOnly={readOnly}
          theme={theme}
          showLineNumbers={lineNumbers !== 'off'}
          showMinimap={minimap}
          enableLsp={false}
          onContentChange={handleContentChange}
          onEditorReady={handleEditorReady}
        />
      </div>
    </div>
  );
};

function getExtensionFromLanguage(language: string): string {
  const languageExtensionMap: Record<string, string> = {
    typescript: 'ts',
    javascript: 'js',
    typescriptreact: 'tsx',
    javascriptreact: 'jsx',
    python: 'py',
    java: 'java',
    csharp: 'cs',
    cpp: 'cpp',
    c: 'c',
    go: 'go',
    rust: 'rs',
    ruby: 'rb',
    php: 'php',
    swift: 'swift',
    kotlin: 'kt',
    scala: 'scala',
    html: 'html',
    css: 'css',
    scss: 'scss',
    less: 'less',
    json: 'json',
    yaml: 'yaml',
    xml: 'xml',
    markdown: 'md',
    sql: 'sql',
    shell: 'sh',
    bash: 'sh',
    powershell: 'ps1',
    plaintext: 'txt',
  };
  
  return languageExtensionMap[language] || 'txt';
}

export default CodeEditor;
