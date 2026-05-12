/**
 * Markdown Editor Component
 * 
 * Based on M-Editor with IR (Instant Render) mode.
 * @module components/MarkdownEditor
 */

import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import { MEditor } from '../meditor';
import type { EditorInstance } from '../meditor';
import { analyzeMarkdownEditability, type MarkdownEditabilityAnalysis } from '../meditor/utils/tiptapMarkdown';
import { AlertCircle, Check, Copy } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { sendDebugProbe } from '@/shared/utils/debugProbe';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { globalEventBus } from '@/infrastructure/event-bus';
import { isSamePath } from '@/shared/utils/pathUtils';
import { CubeLoading, Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import CodeEditor from './CodeEditor';
import {
  diskVersionFromMetadata,
  diskVersionsDiffer,
  type DiskFileVersion,
} from '../utils/diskFileVersion';
import { confirmDialog } from '@/component-library/components/ConfirmDialog/confirmService';
import {
  isFileMissingFromMetadata,
  isLikelyFileNotFoundError,
} from '@/shared/utils/fsErrorUtils';
import './MarkdownEditor.scss';

import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github-dark.css';

const log = createLogger('MarkdownEditor');

const FILE_SYNC_POLL_INTERVAL_MS = 1000;

function getPollOffsetMs(filePath: string): number {
  let hash = 0;
  for (let i = 0; i < filePath.length; i++) {
    hash = ((hash << 5) - hash + filePath.charCodeAt(i)) | 0;
  }
  return Math.abs(hash) % 400;
}

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
  /** When false, disk sync polling is paused (background tab). */
  isActiveTab?: boolean;
  /** File missing on disk (tab chrome); skipped when embedded CodeEditor handles the same path */
  onFileMissingFromDiskChange?: (missing: boolean) => void;
}

const MarkdownEditor: React.FC<MarkdownEditorProps> = ({
  filePath,
  initialContent = '',
  workspacePath,
  fileName,
  readOnly = false,
  className = '',
  onContentChange,
  onSave,
  jumpToLine,
  jumpToColumn,
  isActiveTab = true,
  onFileMissingFromDiskChange,
}) => {
  const { t } = useI18n('tools');
  const { isLight } = useTheme();
  const [content, setContent] = useState<string>(initialContent);
  const [hasChanges, setHasChanges] = useState(false);
  const [viewMode, setViewMode] = useState<'preview' | 'markdown'>('preview');
  const [unsafeViewMode, setUnsafeViewMode] = useState<'source' | 'preview'>('source');
  const [loading, setLoading] = useState(!!filePath);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [editability, setEditability] = useState<MarkdownEditabilityAnalysis>(() => analyzeMarkdownEditability(initialContent));
  const editorRef = useRef<EditorInstance>(null);
  const isUnmountedRef = useRef(false);
  const diskVersionRef = useRef<DiskFileVersion | null>(null);
  const isCheckingDiskRef = useRef(false);
  const hasChangesRef = useRef(false);
  const lastJumpPositionRef = useRef<{ filePath: string; line: number } | null>(null);
  const onContentChangeRef = useRef(onContentChange);
  const contentRef = useRef(content);
  const lastReportedDirtyRef = useRef<boolean | null>(null);
  const unsafeViewModeRef = useRef(unsafeViewMode);
  unsafeViewModeRef.current = unsafeViewMode;
  const lastReportedMissingRef = useRef<boolean | undefined>(undefined);

  const reportFileMissingFromDisk = useCallback(
    (missing: boolean) => {
      if (!onFileMissingFromDiskChange) {
        return;
      }
      const isUnsafeSplit =
        !!filePath &&
        (editability.mode === 'unsafe' ||
          editability.containsRenderOnlyBlocks ||
          editability.containsRawHtmlInlines);
      if (isUnsafeSplit && unsafeViewModeRef.current === 'source') {
        return;
      }
      if (lastReportedMissingRef.current === missing) {
        return;
      }
      lastReportedMissingRef.current = missing;
      onFileMissingFromDiskChange(missing);
    },
    [editability.containsRawHtmlInlines, editability.containsRenderOnlyBlocks, editability.mode, filePath, onFileMissingFromDiskChange]
  );

  onContentChangeRef.current = onContentChange;
  contentRef.current = content;

  useEffect(() => {
    hasChangesRef.current = hasChanges;
  }, [hasChanges]);

  const toNormalizedMarkdown = useCallback((raw: string) => {
    const nextEditability = analyzeMarkdownEditability(raw);
    const nextContent =
      nextEditability.mode === 'unsafe' ? raw : nextEditability.canonicalMarkdown;
    return { nextEditability, nextContent };
  }, []);

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
    const editor = editorRef.current;
    return () => {
      isUnmountedRef.current = true;
      editor?.destroy();
    };
  }, []);

  useEffect(() => {
    setViewMode('preview');
    setUnsafeViewMode('source');
  }, [filePath, initialContent]);

  const fetchFileMetadata = useCallback(async () => {
    if (!filePath) {
      throw new Error('Missing file path');
    }
    const { workspaceAPI } = await import('@/infrastructure/api');
    return workspaceAPI.getFileMetadata(filePath);
  }, [filePath]);

  const loadFileContent = useCallback(async () => {
    if (!filePath || isUnmountedRef.current) return;

    setLoading(true);
    setError(null);

    try {
      const { workspaceAPI } = await import('@/infrastructure/api');

      const fileContent = await workspaceAPI.readFileContent(filePath);
      reportFileMissingFromDisk(false);

      try {
        const fileInfo = await fetchFileMetadata();
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
        log.warn('Failed to get file metadata', err);
      }

      if (!isUnmountedRef.current) {
        const { nextEditability, nextContent } = toNormalizedMarkdown(fileContent);

        setEditability(nextEditability);
        setContent(nextContent);
        setHasChanges(false);
        lastReportedDirtyRef.current = false;
        setTimeout(() => {
          editorRef.current?.setInitialContent?.(nextContent);
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
        if (errStr.includes('does not exist') || errStr.includes('No such file')) {
          reportFileMissingFromDisk(true);
        }
      }
    } finally {
      if (!isUnmountedRef.current) {
        setLoading(false);
      }
    }
  }, [fetchFileMetadata, filePath, reportFileMissingFromDisk, t, toNormalizedMarkdown]);

  // Initial file load - only run once when filePath changes
  const loadFileContentCalledRef = useRef(false);
  useEffect(() => {
    loadFileContentCalledRef.current = false;
    diskVersionRef.current = null;
    lastReportedMissingRef.current = undefined;
  }, [filePath]);
  
  useEffect(() => {
    if (filePath) {
      if (!loadFileContentCalledRef.current) {
        loadFileContentCalledRef.current = true;
        loadFileContent();
      }
    } else if (initialContent !== undefined) {
      const nextEditability = analyzeMarkdownEditability(initialContent);
      const nextContent = nextEditability.mode === 'unsafe'
        ? initialContent
        : nextEditability.canonicalMarkdown;

      setEditability(nextEditability);
      setContent(nextContent);
      setHasChanges(false);
      lastReportedDirtyRef.current = false;
      setTimeout(() => {
        editorRef.current?.setInitialContent?.(nextContent);
      }, 0);
      // NOTE: Do NOT call onContentChange here during initial load.
      // Calling it triggers parent re-render which unmounts this component,
      // causing an infinite loop.
    }
  }, [filePath, initialContent, loadFileContent]);

  const syncMarkdownFromDisk = useCallback(async (source: 'poll' | 'event') => {
    if (!filePath || isUnmountedRef.current || isCheckingDiskRef.current) {
      return;
    }

    if (
      source === 'poll' &&
      (!isActiveTab ||
        (typeof document !== 'undefined' && document.visibilityState !== 'visible'))
    ) {
      return;
    }

    isCheckingDiskRef.current = true;
    const startedAt = nowMs();
    let outcome = 'started';
    let probeError: string | null = null;
    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      const fileInfo = await fetchFileMetadata();
      if (isFileMissingFromMetadata(fileInfo)) {
        outcome = 'missing-on-disk';
        reportFileMissingFromDisk(true);
        return;
      }
      reportFileMissingFromDisk(false);
      const currentVersion = diskVersionFromMetadata(fileInfo);
      if (!currentVersion) {
        outcome = 'missing-version';
        return;
      }
      const baseline = diskVersionRef.current;
      if (!baseline) {
        diskVersionRef.current = currentVersion;
        outcome = 'initialized-baseline';
        return;
      }
      if (!diskVersionsDiffer(currentVersion, baseline)) {
        outcome = 'no-change';
        return;
      }

      const localBefore = contentRef.current;
      const raw = await workspaceAPI.readFileContent(filePath);
      if (localBefore !== contentRef.current) {
        outcome = 'editor-changed-before-read';
        return;
      }
      const { nextEditability, nextContent } = toNormalizedMarkdown(raw);
      if (nextContent === contentRef.current) {
        diskVersionRef.current = currentVersion;
        outcome = 'content-match';
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
          diskVersionRef.current = currentVersion;
          outcome = 'kept-local-changes';
          return;
        }
      }

      if (!isUnmountedRef.current) {
        setEditability(nextEditability);
        setContent(nextContent);
        contentRef.current = nextContent;
        setHasChanges(false);
        lastReportedDirtyRef.current = false;
        onContentChangeRef.current?.(nextContent, false);
        setTimeout(() => {
          editorRef.current?.setInitialContent?.(nextContent);
        }, 0);
        editorRef.current?.markSaved?.();
        reportFileMissingFromDisk(false);
      }

      const fileInfoAfter = await fetchFileMetadata();
      if (!isFileMissingFromMetadata(fileInfoAfter)) {
        const vAfter = diskVersionFromMetadata(fileInfoAfter);
        if (vAfter) {
          diskVersionRef.current = vAfter;
        }
      }
      outcome = 'reloaded-from-disk';
    } catch (e) {
      outcome = 'error';
      probeError = e instanceof Error ? e.message : String(e);
      if (isLikelyFileNotFoundError(e)) {
        reportFileMissingFromDisk(true);
      }
      log.error('Markdown disk sync check failed', e);
    } finally {
      const durationMs = elapsedMs(startedAt);
      if (probeError || outcome !== 'no-change' || durationMs >= 80) {
        sendDebugProbe(
          'MarkdownEditor.tsx:checkMarkdownDisk',
          'Markdown editor disk sync completed',
          {
            filePath,
            source,
            outcome,
            durationMs,
            error: probeError,
          }
        );
      }
      isCheckingDiskRef.current = false;
    }
  }, [fetchFileMetadata, filePath, isActiveTab, reportFileMissingFromDisk, t, toNormalizedMarkdown]);

  const checkMarkdownDisk = useCallback(async () => {
    await syncMarkdownFromDisk('poll');
  }, [syncMarkdownFromDisk]);

  const isUnsafeSplitUi =
    !!filePath &&
    (editability.mode === 'unsafe' ||
      editability.containsRenderOnlyBlocks ||
      editability.containsRawHtmlInlines);
  const pollMarkdownDisk = !isUnsafeSplitUi || unsafeViewMode !== 'source';

  useEffect(() => {
    if (!filePath || !isActiveTab || !pollMarkdownDisk) {
      return;
    }
    const tick = () => {
      void checkMarkdownDisk();
    };
    const pollOffsetMs = getPollOffsetMs(filePath);
    let intervalId: number | null = null;
    const timeoutId = window.setTimeout(() => {
      tick();
      intervalId = window.setInterval(tick, FILE_SYNC_POLL_INTERVAL_MS + pollOffsetMs);
    }, 250 + pollOffsetMs);
    return () => {
      window.clearTimeout(timeoutId);
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
    };
  }, [checkMarkdownDisk, filePath, isActiveTab, pollMarkdownDisk]);

  useEffect(() => {
    if (!filePath || !pollMarkdownDisk) {
      return;
    }

    return globalEventBus.on('editor:file-changed', (data: { filePath?: string }) => {
      if (!isSamePath(data.filePath || '', filePath)) {
        return;
      }
      void syncMarkdownFromDisk('event');
    });
  }, [filePath, pollMarkdownDisk, syncMarkdownFromDisk]);

  const saveFileContent = useCallback(async () => {
    if (!hasChanges || isUnmountedRef.current) return;

    setError(null);

    try {
      if (filePath && workspacePath) {
        const { workspaceAPI } = await import('@/infrastructure/api');

        const fileInfoPre = await fetchFileMetadata();
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
            const raw = await workspaceAPI.readFileContent(filePath);
            const { nextEditability, nextContent } = toNormalizedMarkdown(raw);
            if (!isUnmountedRef.current) {
              setEditability(nextEditability);
              setContent(nextContent);
              contentRef.current = nextContent;
              setHasChanges(false);
              lastReportedDirtyRef.current = false;
              editorRef.current?.markSaved?.();
              onContentChangeRef.current?.(nextContent, false);
              setTimeout(() => {
                editorRef.current?.setInitialContent?.(nextContent);
              }, 0);
              reportFileMissingFromDisk(false);
            }
            try {
              const fileInfoAfter = await fetchFileMetadata();
              if (!isFileMissingFromMetadata(fileInfoAfter)) {
                const v = diskVersionFromMetadata(fileInfoAfter);
                if (v) {
                  diskVersionRef.current = v;
                }
              }
            } catch (err) {
              log.warn('Failed to sync disk version after save conflict reload', err);
            }
            return;
          }
        }

        await workspaceAPI.writeFileContent(workspacePath, filePath, content);

        try {
          const fileInfo = await fetchFileMetadata();
          if (!isFileMissingFromMetadata(fileInfo)) {
            reportFileMissingFromDisk(false);
            const v = diskVersionFromMetadata(fileInfo);
            if (v) {
              diskVersionRef.current = v;
            }
          }
        } catch (err) {
          log.warn('Failed to get file metadata', err);
        }

        if (!isUnmountedRef.current) {
          editorRef.current?.markSaved?.();
          setHasChanges(false);
          lastReportedDirtyRef.current = false;
          if (onContentChangeRef.current) {
            onContentChangeRef.current(content, false);
          }
        }

        globalEventBus.emit('file-tree:refresh');
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
  }, [content, fetchFileMetadata, filePath, hasChanges, onSave, reportFileMissingFromDisk, t, toNormalizedMarkdown, workspacePath]);

  const handleContentChange = useCallback((newContent: string) => {
    contentRef.current = newContent;
    setContent(newContent);
  }, []);

  const handleDirtyChange = useCallback((isDirty: boolean) => {
    setHasChanges(isDirty);
    if (lastReportedDirtyRef.current === isDirty) {
      return;
    }

    lastReportedDirtyRef.current = isDirty;
    onContentChangeRef.current?.(contentRef.current, isDirty);
  }, []);

  const handleSave = useCallback((_value: string) => {
    saveFileContent();
  }, [saveFileContent]);

  const handleCopyMarkdown = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(contentRef.current);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch (err) {
      log.warn('Failed to copy markdown editor content', err);
    }
  }, []);

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

  const notices = useMemo(() => {
    const nextNotices: string[] = [];

    if (filePath && (
      editability.mode === 'unsafe' ||
      editability.containsRenderOnlyBlocks ||
      editability.containsRawHtmlInlines
    )) {
      nextNotices.push(t('editor.markdownEditor.notice.sourcePreviewFallback'));
    }

    return nextNotices;
  }, [editability, filePath, t]);

  const shouldUseSourcePreviewFallback = !!filePath && (
    editability.mode === 'unsafe' ||
    editability.containsRenderOnlyBlocks ||
    editability.containsRawHtmlInlines
  );

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

  if (shouldUseSourcePreviewFallback) {
    return (
      <div className={`bitfun-markdown-editor ${className}`}>
        {notices.length > 0 && (
          <div className="bitfun-markdown-editor__notice-bar">
            <AlertCircle className="bitfun-markdown-editor__notice-icon" />
            <div className="bitfun-markdown-editor__notice-copy">
              {notices.map(notice => (
                <p key={notice}>{notice}</p>
              ))}
            </div>
          </div>
        )}
        <div className="bitfun-markdown-editor__mode-toolbar">
          <div className="bitfun-markdown-editor__mode-toggle" role="tablist" aria-label={t('editor.markdownEditor.viewModeLabel')}>
            <Button
              type="button"
              size="small"
              variant={unsafeViewMode === 'source' ? 'primary' : 'secondary'}
              className="bitfun-markdown-editor__toolbar-button"
              onClick={() => setUnsafeViewMode('source')}
              aria-pressed={unsafeViewMode === 'source'}
            >
              {t('editor.markdownEditor.markdown')}
            </Button>
            <Button
              type="button"
              size="small"
              variant={unsafeViewMode === 'preview' ? 'primary' : 'secondary'}
              className="bitfun-markdown-editor__toolbar-button"
              onClick={() => setUnsafeViewMode('preview')}
              aria-pressed={unsafeViewMode === 'preview'}
            >
              {t('editor.markdownEditor.preview')}
            </Button>
          </div>
          <div className="bitfun-markdown-editor__toolbar-actions">
            <Button
              type="button"
              size="small"
              variant="secondary"
              iconOnly
              className="bitfun-markdown-editor__toolbar-button bitfun-markdown-editor__copy-button"
              onClick={() => void handleCopyMarkdown()}
              aria-label={copied
                ? t('editor.markdownEditor.copiedMarkdown', { defaultValue: 'Copied Markdown' })
                : t('editor.markdownEditor.copyMarkdown', { defaultValue: 'Copy Markdown' })}
              title={copied
                ? t('editor.markdownEditor.copiedMarkdown', { defaultValue: 'Copied Markdown' })
                : t('editor.markdownEditor.copyMarkdown', { defaultValue: 'Copy Markdown' })}
            >
              {copied ? <Check size={13} /> : <Copy size={13} />}
            </Button>
          </div>
        </div>
        <div className="bitfun-markdown-editor__unsafe-body">
          {unsafeViewMode === 'source' ? (
            <CodeEditor
              filePath={filePath}
              workspacePath={workspacePath}
              fileName={filePath.split(/[/\\]/).pop() || fileName}
              language="markdown"
              readOnly={readOnly}
              showLineNumbers={true}
              showMinimap={true}
              jumpToLine={jumpToLine}
              jumpToColumn={jumpToColumn}
              isActiveTab={isActiveTab}
              onFileMissingFromDiskChange={onFileMissingFromDiskChange}
              onContentChange={(newContent, dirty) => {
                contentRef.current = newContent;
                setContent(newContent);
                setHasChanges(dirty);
                if (lastReportedDirtyRef.current === dirty) {
                  return;
                }

                lastReportedDirtyRef.current = dirty;
                onContentChangeRef.current?.(newContent, dirty);
              }}
              onSave={(_savedContent) => {
                setHasChanges(false);
                lastReportedDirtyRef.current = false;
                onContentChangeRef.current?.(contentRef.current, false);
              }}
            />
          ) : (
            <MEditor
              ref={editorRef}
              value={content}
              onChange={handleContentChange}
              onSave={handleSave}
              onDirtyChange={handleDirtyChange}
              mode="preview"
              theme={isLight ? 'light' : 'dark'}
              height="100%"
              width="100%"
              placeholder={t('editor.markdownEditor.placeholder')}
              readonly={true}
              toolbar={false}
              filePath={filePath}
              basePath={basePath}
            />
          )}
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-markdown-editor ${className}`}>
      {notices.length > 0 && (
        <div className="bitfun-markdown-editor__notice-bar">
          <AlertCircle className="bitfun-markdown-editor__notice-icon" />
          <div className="bitfun-markdown-editor__notice-copy">
            {notices.map(notice => (
              <p key={notice}>{notice}</p>
            ))}
          </div>
        </div>
      )}
      <div className="bitfun-markdown-editor__mode-toolbar">
        <div className="bitfun-markdown-editor__mode-toggle" role="tablist" aria-label={t('editor.markdownEditor.viewModeLabel')}>
          <Button
            type="button"
            size="small"
            variant={viewMode === 'preview' ? 'primary' : 'secondary'}
            className="bitfun-markdown-editor__toolbar-button"
            onClick={() => setViewMode('preview')}
            aria-pressed={viewMode === 'preview'}
          >
            {t('editor.markdownEditor.preview')}
          </Button>
          <Button
            type="button"
            size="small"
            variant={viewMode === 'markdown' ? 'primary' : 'secondary'}
            className="bitfun-markdown-editor__toolbar-button"
            onClick={() => setViewMode('markdown')}
            aria-pressed={viewMode === 'markdown'}
          >
            {t('editor.markdownEditor.markdown')}
          </Button>
        </div>
        <div className="bitfun-markdown-editor__toolbar-actions">
          <Button
            type="button"
            size="small"
            variant="secondary"
            iconOnly
            className="bitfun-markdown-editor__toolbar-button bitfun-markdown-editor__copy-button"
            onClick={() => void handleCopyMarkdown()}
            aria-label={copied
              ? t('editor.markdownEditor.copiedMarkdown', { defaultValue: 'Copied Markdown' })
              : t('editor.markdownEditor.copyMarkdown', { defaultValue: 'Copy Markdown' })}
            title={copied
              ? t('editor.markdownEditor.copiedMarkdown', { defaultValue: 'Copied Markdown' })
              : t('editor.markdownEditor.copyMarkdown', { defaultValue: 'Copy Markdown' })}
          >
            {copied ? <Check size={13} /> : <Copy size={13} />}
          </Button>
        </div>
      </div>
      <MEditor
        ref={editorRef}
        value={content}
        onChange={handleContentChange}
        onSave={handleSave}
        onDirtyChange={handleDirtyChange}
        mode={viewMode === 'preview' ? 'ir' : 'edit'}
        theme={isLight ? 'light' : 'dark'}
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
