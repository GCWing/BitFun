/**
 * Markdown Editor Component
 * 
 * Based on M-Editor with IR (Instant Render) mode.
 * @module components/MarkdownEditor
 */

import React, { useEffect, useState, useCallback, useRef } from 'react';
import { MEditor } from '../meditor';
import type { EditorInstance } from '../meditor';
import { AlertCircle } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { CubeLoading, Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './MarkdownEditor.scss';

const log = createLogger('MarkdownEditor');
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

export interface MarkdownEditorProps {
  /** File path - loads from file if provided, otherwise uses initialContent */
  filePath?: string;
  /** Initial content - used when no filePath */
  initialContent?: string;
  /** Workspace path */
  workspacePath?: string;
  /** File name */
  fileName?: string;
  /** Read-only mode */
  readOnly?: boolean;
  /** CSS class name */
  className?: string;
  /** Content change callback */
  onContentChange?: (content: string, hasChanges: boolean) => void;
  /** Save callback */
  onSave?: (content: string) => void;
  /** Jump to line number (auto-jump after file opens) */
  jumpToLine?: number;
  /** Jump to column (auto-jump after file opens) */
  jumpToColumn?: number;
}

const MarkdownEditor: React.FC<MarkdownEditorProps> = ({
  filePath,
  initialContent = '',
  workspacePath,
  readOnly = false,
  className = '',
  onContentChange,
  onSave,
  jumpToLine,
  jumpToColumn,
}) => {
  const { t } = useI18n('tools');
  const [content, setContent] = useState<string>(initialContent);
  const [hasChanges, setHasChanges] = useState(false);
  const [loading, setLoading] = useState(!!filePath);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<EditorInstance>(null);
  const isUnmountedRef = useRef(false);
  const lastModifiedTimeRef = useRef<number>(0);
  const lastJumpPositionRef = useRef<{ filePath: string; line: number } | null>(null);
  const onContentChangeRef = useRef(onContentChange);
  onContentChangeRef.current = onContentChange;

  const basePath = React.useMemo(() => {
    if (!filePath) return undefined;
    const normalizedPath = filePath.replace(/\\/g, '/');
    const lastSlashIndex = normalizedPath.lastIndexOf('/');
    if (lastSlashIndex >= 0) {
      return normalizedPath.substring(0, lastSlashIndex);
    }
    return undefined;
  }, [filePath]);

  useEffect(() => {
    isUnmountedRef.current = false;
    return () => {
      isUnmountedRef.current = true;
      editorRef.current?.destroy();
    };
  }, []);

  const loadFileContent = useCallback(async () => {
    if (!filePath || isUnmountedRef.current) return;

    setLoading(true);
    setError(null);

    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      const { invoke } = await import('@tauri-apps/api/core');
      
      const fileContent = await workspaceAPI.readFileContent(filePath);

      try {
        const fileInfo: any = await invoke('get_file_metadata', { 
          request: { path: filePath } 
        });
        lastModifiedTimeRef.current = fileInfo.modified;
      } catch (err) {
        log.warn('Failed to get file metadata', err);
      }
        
      if (!isUnmountedRef.current) {
        setContent(fileContent);
        setHasChanges(false);
        setTimeout(() => {
          editorRef.current?.setInitialContent?.(fileContent);
        }, 0);
        // NOTE: Do NOT call onContentChange here during initial load.
        // Calling it triggers parent re-render which unmounts this component,
        // causing an infinite loop.
      }
    } catch (err) {
      if (!isUnmountedRef.current) {
        const errStr = String(err);
        log.error('Failed to load file', err);
        let displayError = t('editor.common.loadFailed');
        if (errStr.includes('does not exist') || errStr.includes('No such file')) {
          displayError = t('editor.common.fileNotFound');
        } else if (errStr.includes('Permission denied') || errStr.includes('permission')) {
          displayError = t('editor.common.permissionDenied');
        }
        setError(displayError);
      }
    } finally {
      if (!isUnmountedRef.current) {
        setLoading(false);
      }
    }
  }, [filePath, t]);

  // Initial file load - only run once when filePath changes
  const loadFileContentCalledRef = useRef(false);
  useEffect(() => {
    // Reset the flag when filePath changes
    loadFileContentCalledRef.current = false;
  }, [filePath]);
  
  useEffect(() => {
    if (filePath) {
      if (!loadFileContentCalledRef.current) {
        loadFileContentCalledRef.current = true;
        loadFileContent();
      }
    } else if (initialContent !== undefined) {
      setContent(initialContent);
      setHasChanges(false);
      setTimeout(() => {
        editorRef.current?.setInitialContent?.(initialContent);
      }, 0);
      // NOTE: Do NOT call onContentChange here during initial load.
      // Calling it triggers parent re-render which unmounts this component,
      // causing an infinite loop.
    }
  }, [filePath, initialContent, loadFileContent]);

  const saveFileContent = useCallback(async () => {
    if (!hasChanges || isUnmountedRef.current) return;

    setError(null);

    try {
      if (filePath && workspacePath) {
        const { workspaceAPI } = await import('@/infrastructure/api');
        const { invoke } = await import('@tauri-apps/api/core');

        await workspaceAPI.writeFileContent(workspacePath, filePath, content);
        
        try {
          const fileInfo: any = await invoke('get_file_metadata', { 
            request: { path: filePath } 
          });
          lastModifiedTimeRef.current = fileInfo.modified;
        } catch (err) {
          log.warn('Failed to get file metadata', err);
        }

        if (!isUnmountedRef.current) {
          editorRef.current?.markSaved?.();
          setHasChanges(false);
          if (onContentChangeRef.current) {
            onContentChangeRef.current(content, false);
          }
        }
      }

      if (onSave) {
        onSave(content);
      }
    } catch (err) {
      if (!isUnmountedRef.current) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        log.error('Failed to save file', err);
        setError(t('editor.common.saveFailedWithMessage', { message: errorMessage }));
      }
    }
  }, [content, filePath, workspacePath, hasChanges, onSave, t]);

  const handleContentChange = useCallback((newContent: string) => {
    setContent(newContent);
  }, []);

  const handleDirtyChange = useCallback((isDirty: boolean) => {
    setHasChanges(isDirty);
  }, []);

  const handleSave = useCallback((_value: string) => {
    saveFileContent();
  }, [saveFileContent]);

  useEffect(() => {
    if (!jumpToLine) {
      return;
    }

    const lastJump = lastJumpPositionRef.current;
    if (lastJump && 
        lastJump.filePath === filePath && 
        lastJump.line === jumpToLine) {
      return;
    }

    if (loading) {
      return;
    }

    if (!editorRef.current) {
      return;
    }

    const timer = setTimeout(() => {
      if (editorRef.current?.scrollToLine) {
        editorRef.current.scrollToLine(jumpToLine, true);
        
        lastJumpPositionRef.current = {
          filePath: filePath || '',
          line: jumpToLine
        };
      }
    }, 100);

    return () => clearTimeout(timer);
  }, [jumpToLine, jumpToColumn, filePath, loading, content]);

  if (loading) {
    return (
      <div className={`bitfun-markdown-editor-loading ${className}`}>
        <CubeLoading size="medium" text={t('editor.markdownEditor.loadingFile')} />
      </div>
    );
  }

  if (error) {
    return (
      <div className={`bitfun-markdown-editor-error ${className}`}>
        <div className="error-content">
          <AlertCircle className="error-icon" />
          <p>{error}</p>
          {filePath && (
            <Button variant="secondary" size="small" onClick={loadFileContent}>
              {t('editor.common.retry')}
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-markdown-editor ${className}`}>
      <MEditor
        ref={editorRef}
        value={content}
        onChange={handleContentChange}
        onSave={handleSave}
        onDirtyChange={handleDirtyChange}
        mode="ir"
        theme="dark"
        height="100%"
        width="100%"
        placeholder={t('editor.markdownEditor.placeholder')}
        readonly={readOnly}
        toolbar={false}
        filePath={filePath}
        basePath={basePath}
      />
    </div>
  );
};

export default MarkdownEditor;
