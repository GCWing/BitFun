/**
 * Mermaid syntax highlighter built on CodeMirror.
 */

import React, { useRef, useEffect, useCallback } from 'react';
import { EditorView, keymap, highlightSpecialChars, drawSelection, lineNumbers, highlightActiveLineGutter } from '@codemirror/view';
import { EditorState, Extension, StateEffect } from '@codemirror/state';
import { history, defaultKeymap, historyKeymap } from '@codemirror/commands';
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
import { bracketMatching, indentOnInput, syntaxHighlighting, HighlightStyle } from '@codemirror/language';
import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { mermaid, mermaidHighlightStyle } from '../utils/codemirrorMermaid';
import './MermaidSyntaxHighlighter.css';

export interface MermaidSyntaxHighlighterProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  readOnly?: boolean;
  showLineNumbers?: boolean;
  onCursorPositionChange?: (line: number, column: number) => void;
}

export const MermaidSyntaxHighlighter: React.FC<MermaidSyntaxHighlighterProps> = ({
  value,
  onChange,
  className = '',
  readOnly = false,
  showLineNumbers = true,
  onCursorPositionChange
}) => {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  // Theme overrides.
  const customTheme = EditorView.theme({
    '&': {
      color: 'var(--text-primary, #ffffff)',
      backgroundColor: 'var(--color-bg-primary)',
      fontSize: '14px',
      fontFamily: "'Consolas', 'Monaco', 'Courier New', monospace",
      height: '100%',
      border: 'none !important',
      outline: 'none !important'
    },
    '.cm-content': {
      padding: '16px',
      minHeight: '200px',
      caretColor: '#ffffff !important',
      border: 'none !important',
      outline: 'none !important'
    },
    '.cm-content.cm-focused': {
      caretColor: '#ffffff !important'
    },
    '.cm-focused': {
      outline: 'none !important',
      border: 'none !important'
    },
    '.cm-editor': {
      height: '100%',
      border: 'none !important',
      outline: 'none !important'
    },
    '.cm-scroller': {
      fontFamily: "'Consolas', 'Monaco', 'Courier New', monospace",
      lineHeight: '1.5',
      border: 'none !important',
      outline: 'none !important'
    },
    '.cm-gutters': {
      backgroundColor: 'var(--color-bg-secondary, #18181a)',
      color: 'var(--color-text-muted, #707070)',
      border: 'none',
      borderRight: '1px solid var(--border-subtle, rgba(255, 255, 255, 0.08))'
    },
    '.cm-lineNumbers .cm-gutterElement': {
      minWidth: '30px',
      fontSize: '12px'
    },
    '.cm-activeLine': {
      backgroundColor: 'rgba(255, 255, 255, 0.05)'
    },
    '.cm-activeLineGutter': {
      backgroundColor: 'var(--color-bg-secondary, #18181a)',
      color: 'var(--color-accent-500, #60a5fa)'
    },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
      backgroundColor: 'rgba(59, 130, 246, 0.3) !important'
    },
    '.cm-line ::selection': {
      backgroundColor: 'rgba(59, 130, 246, 0.3) !important'
    },
    '.cm-cursor, .cm-cursor-primary': {
      borderLeft: '2px solid #ffffff !important',
      borderRight: 'none !important',
      borderTop: 'none !important', 
      borderBottom: 'none !important',
      marginLeft: '-1px !important',
      width: '0 !important',
      height: '1.2em !important',
      display: 'block !important',
      opacity: '1 !important',
      visibility: 'visible !important',
      position: 'relative !important',
      zIndex: '100 !important',
      background: 'transparent !important'
    },
    '.cm-placeholder': {
      color: 'var(--text-muted, #666666)'
    }
  }, { dark: true });


  // Cursor position update listener.
  const cursorPositionExtension = EditorView.updateListener.of((update) => {
    if (update.selectionSet) {
      const state = update.state;
      const cursor = state.selection.main.head;
      const line = state.doc.lineAt(cursor);
      const lineNumber = line.number;
      const column = cursor - line.from + 1;
      
      onCursorPositionChange?.(lineNumber, column);
    }
  });

  // Content change listener.
  const onChangeExtension = EditorView.updateListener.of((update) => {
    if (update.docChanged) {
      const newValue = update.state.doc.toString();
      onChange(newValue);
    }
  });

  // Build editor extensions.
  const createExtensions = useCallback((): Extension[] => {
    const extensions: Extension[] = [
      highlightSpecialChars(),
      history(),
      drawSelection(),
      syntaxHighlighting(HighlightStyle.define(mermaidHighlightStyle)),
      mermaid(),
      bracketMatching(),
      closeBrackets(),
      indentOnInput(),
      highlightSelectionMatches(),
      keymap.of([
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        {
          key: 'Ctrl-/',
          mac: 'Cmd-/',
          run: () => {
            // Toggle Mermaid line comments (%%).
            if (viewRef.current) {
              const view = viewRef.current;
              const state = view.state;
              const selection = state.selection.main;
              const lineStart = state.doc.lineAt(selection.from);
              const lineEnd = state.doc.lineAt(selection.to);
              
              const changes = [];
              for (let lineNum = lineStart.number; lineNum <= lineEnd.number; lineNum++) {
                const line = state.doc.line(lineNum);
                const lineText = line.text;
                const trimmed = lineText.trim();
                
                if (trimmed.startsWith('%%')) {
                  // Remove comment.
                  const newText = lineText.replace(/^(\s*)%%\s?/, '$1');
                  changes.push({ from: line.from, to: line.to, insert: newText });
                } else if (trimmed) {
                  // Add comment.
                  const match = lineText.match(/^(\s*)/);
                  const indent = match ? match[1] : '';
                  const newText = indent + '%% ' + lineText.substring(indent.length);
                  changes.push({ from: line.from, to: line.to, insert: newText });
                }
              }
              
              if (changes.length > 0) {
                view.dispatch({ changes });
              }
            }
            return true;
          }
        }
      ]),
      customTheme,
      cursorPositionExtension,
      onChangeExtension
    ];

    // Optional extensions.
    if (showLineNumbers) {
      extensions.push(lineNumbers(), highlightActiveLineGutter());
    }

    if (readOnly) {
      extensions.push(EditorState.readOnly.of(true));
    }

    return extensions;
  }, [showLineNumbers, readOnly, onCursorPositionChange, onChange]);

  // Initialize editor.
  useEffect(() => {
    if (!editorRef.current) return;

    const startState = EditorState.create({
      doc: value,
      extensions: createExtensions()
    });

    const view = new EditorView({
      state: startState,
      parent: editorRef.current
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  // Sync external value to the editor.
  useEffect(() => {
    if (viewRef.current && viewRef.current.state.doc.toString() !== value) {
      const view = viewRef.current;
      const transaction = view.state.update({
        changes: {
          from: 0,
          to: view.state.doc.length,
          insert: value
        }
      });
      view.dispatch(transaction);
    }
  }, [value]);

  // Reconfigure when options change.
  useEffect(() => {
    if (viewRef.current) {
      const view = viewRef.current;
      view.dispatch({
        effects: StateEffect.reconfigure.of(createExtensions())
      });
    }
  }, [createExtensions]);

  return (
    <div className={`codemirror-syntax-highlighter ${className}`}>
      <div
        ref={editorRef}
        className="codemirror-container"
      />
    </div>
  );
};