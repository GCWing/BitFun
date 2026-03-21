import React, { useEffect, useRef } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { useMarkdown } from '../hooks/useMarkdown';
import { MermaidService } from '@/tools/mermaid-editor/services/MermaidService';
import { loadLocalImages } from '../utils/loadLocalImages';
import { useI18n } from '@/infrastructure/i18n';
import type { RenderOptions } from '../types';

const log = createLogger('InlineMarkdownPreview');

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

interface InlineMarkdownPreviewProps {
  value: string;
  options?: RenderOptions;
  basePath?: string;
}

export const InlineMarkdownPreview: React.FC<InlineMarkdownPreviewProps> = ({
  value,
  options,
  basePath,
}) => {
  const { t } = useI18n('tools');
  const mergedOptions = React.useMemo(() => ({
    ...options,
    basePath,
  }), [options, basePath]);
  const { html, isLoading } = useMarkdown(value, mergedOptions);
  const previewRef = useRef<HTMLDivElement>(null);
  const mermaidService = useRef(MermaidService.getInstance());
  const lastHtmlRef = useRef<string>('');

  useEffect(() => {
    if (previewRef.current && html && lastHtmlRef.current !== html) {
      previewRef.current.innerHTML = html;
      lastHtmlRef.current = html;
    }
  }, [html]);

  useEffect(() => {
    if (!previewRef.current || !html) {
      return;
    }

    const mermaidContainers = previewRef.current.querySelectorAll('.mermaid-container:not(.mermaid-rendered)');
    if (mermaidContainers.length === 0) {
      return;
    }

    let isCancelled = false;

    const renderMermaidDiagrams = async () => {
      for (const container of Array.from(mermaidContainers)) {
        if (isCancelled) {
          break;
        }

        const mermaidCode = container.getAttribute('data-mermaid-code');
        if (!mermaidCode) {
          continue;
        }

        try {
          const svg = await mermaidService.current.renderDiagram(mermaidCode);
          if (!isCancelled && previewRef.current?.contains(container)) {
            container.innerHTML = svg;
            container.classList.add('mermaid-rendered');
          }
        } catch (error) {
          log.error('Mermaid render failed', error);
          if (!isCancelled && previewRef.current?.contains(container)) {
            const rawMsg = error instanceof Error ? error.message : t('editor.meditor.unknownError');
            const detail = rawMsg.replace(/^Render failed:\s*/i, '');
            const title = escapeHtml(t('editor.meditor.mermaidRenderFailed'));
            container.innerHTML = `<div class="mermaid-error"><div class="mermaid-error-title">${title}</div><hr class="mermaid-error-divider"/><div class="mermaid-error-detail">${escapeHtml(detail)}</div></div>`;
          }
        }
      }
    };

    const loadImages = async () => {
      if (previewRef.current && !isCancelled) {
        await loadLocalImages(previewRef.current);
      }
    };

    void renderMermaidDiagrams();
    void loadImages();

    return () => {
      isCancelled = true;
    };
  }, [html, t]);

  return (
    <div className="m-editor-inline-ai-rendered">
      {isLoading && (
        <div className="m-editor-inline-ai-rendered__loading">
          {t('editor.meditor.loading')}
        </div>
      )}
      <div
        ref={previewRef}
        className="m-editor-inline-ai-rendered__content markdown-body"
      />
    </div>
  );
};
