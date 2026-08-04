/**
 * HTML Preview Panel
 *
 * Renders local .html/.htm file content in a sandboxed iframe,
 * providing an in-app preview without exporting to a system browser.
 *
 * @module HtmlPreviewPanel
 */

import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { RefreshCw, Code } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { createCodeEditorTab } from '@/shared/utils/tabUtils';
import { createLogger } from '@/shared/utils/logger';
import './HtmlPreviewPanel.scss';

const log = createLogger('HtmlPreviewPanel');

export interface HtmlPreviewPanelProps {
  /** HTML file path */
  filePath: string;
  /** File name */
  fileName?: string;
  /** Workspace path (for relative path resolution) */
  workspacePath?: string;
  /** Whether this tab is currently active */
  isActiveTab?: boolean;
}

const HtmlPreviewPanel: React.FC<HtmlPreviewPanelProps> = ({
  filePath,
  fileName,
  workspacePath,
  isActiveTab = true,
}) => {
  const [htmlContent, setHtmlContent] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [reloadKey, setReloadKey] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const loadContent = useCallback(async () => {
    if (!filePath) {
      setError('No file path provided');
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const content = await workspaceAPI.readFileContent(filePath);
      setHtmlContent(content ?? '');
    } catch (err) {
      log.error('Failed to read HTML file:', err);
      setError(err instanceof Error ? err.message : 'Failed to load HTML file');
    } finally {
      setLoading(false);
    }
  }, [filePath]);

  useEffect(() => {
    if (isActiveTab) {
      loadContent();
    }
  }, [isActiveTab, loadContent, reloadKey]);

  const handleReload = useCallback(() => {
    setReloadKey(k => k + 1);
  }, []);

  const handleEditSource = useCallback(() => {
    if (filePath) {
      const name = fileName || filePath.split(/[/\\]/).pop() || 'untitled.html';
      createCodeEditorTab(filePath, name);
    }
  }, [filePath, fileName]);

  const srcDoc = useMemo(() => htmlContent, [htmlContent]);

  if (loading) {
    return (
      <div className="html-preview-panel html-preview-panel--loading" data-bf-component="html-preview" data-bf-state="loading">
        <div className="html-preview-panel__spinner" />
        <span className="html-preview-panel__loading-text">Loading preview…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="html-preview-panel html-preview-panel--error" data-bf-component="html-preview" data-bf-state="error">
        <div className="html-preview-panel__error-icon">⚠</div>
        <p className="html-preview-panel__error-text">{error}</p>
        <button className="html-preview-panel__retry" onClick={handleReload}>
          <RefreshCw size={14} />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="html-preview-panel" data-bf-component="html-preview" data-bf-state="ready">
      <div className="html-preview-panel__toolbar">
        <Tooltip content="Reload preview">
          <button
            className="html-preview-panel__btn"
            onClick={handleReload}
            aria-label="Reload preview"
          >
            <RefreshCw size={14} />
          </button>
        </Tooltip>
        <Tooltip content="Edit source code">
          <button
            className="html-preview-panel__btn"
            onClick={handleEditSource}
            aria-label="Edit source code"
          >
            <Code size={14} />
          </button>
        </Tooltip>
      </div>
      <iframe
        ref={iframeRef}
        className="html-preview-panel__iframe"
        sandbox="allow-scripts"
        srcDoc={srcDoc}
        title={fileName || 'HTML Preview'}
      />
    </div>
  );
};

export default HtmlPreviewPanel;
