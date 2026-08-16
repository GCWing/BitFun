import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { MarkdownRenderer } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { splitMarkdownFrontmatter } from '../utils/markdownFrontmatter';
import './Preview.scss';

interface PreviewProps {
  value: string;
  basePath?: string;
  progressive?: boolean;
}

const PROGRESSIVE_CHUNK_TARGET_CHARS = 32 * 1024;
const PROGRESSIVE_CHUNK_MAX_CHARS = 64 * 1024;
const PROGRESSIVE_CHUNK_BATCH = 2;

export interface MarkdownChunkResult {
  content: string;
  nextOffset: number;
}

export function takeNextMarkdownChunk(value: string, startOffset: number): MarkdownChunkResult {
  if (startOffset >= value.length) {
    return { content: '', nextOffset: value.length };
  }

  const scanEnd = Math.min(startOffset + PROGRESSIVE_CHUNK_MAX_CHARS, value.length);
  const scanWindow = value.slice(startOffset, scanEnd);
  let cursor = 0;
  let lastBlockBoundary = 0;
  let fenceMarker: '`' | '~' | null = null;

  while (cursor < scanWindow.length) {
    const newline = scanWindow.indexOf('\n', cursor);
    const lineEnd = newline === -1 ? scanWindow.length : newline + 1;
    const line = scanWindow.slice(cursor, newline === -1 ? scanWindow.length : newline);
    const fence = line.match(/^[ \t]{0,3}(`{3,}|~{3,})/);
    if (fence) {
      const marker = fence[1][0] as '`' | '~';
      fenceMarker = fenceMarker === null ? marker : fenceMarker === marker ? null : fenceMarker;
    }

    if (fenceMarker === null && line.trim().length === 0) {
      lastBlockBoundary = lineEnd;
    }

    if (lineEnd >= PROGRESSIVE_CHUNK_TARGET_CHARS && lastBlockBoundary > 0) {
      const nextOffset = startOffset + lastBlockBoundary;
      return { content: value.slice(startOffset, nextOffset), nextOffset };
    }

    cursor = lineEnd;
  }

  return { content: scanWindow, nextOffset: scanEnd };
}

export function shouldFillPreviewViewport(scrollHeight: number, clientHeight: number): boolean {
  return clientHeight > 0 && scrollHeight <= clientHeight;
}

export const Preview: React.FC<PreviewProps> = ({ value, basePath, progressive = false }) => {
  const { t } = useI18n('tools');
  const frontmatter = useMemo(() => splitMarkdownFrontmatter(value), [value]);
  const markdownValue = frontmatter?.body ?? value;
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const loadMoreScheduledRef = useRef(false);
  const loadMoreTimerRef = useRef<number | null>(null);
  const nextOffsetRef = useRef(0);
  const hasMoreRef = useRef(progressive && markdownValue.length > 0);
  const initialChunk = useMemo(
    () => progressive
      ? takeNextMarkdownChunk(markdownValue, 0)
      : { content: markdownValue, nextOffset: markdownValue.length },
    [markdownValue, progressive],
  );
  const [renderedChunks, setRenderedChunks] = useState<string[]>([initialChunk.content]);

  useEffect(() => {
    nextOffsetRef.current = initialChunk.nextOffset;
    hasMoreRef.current = progressive && initialChunk.nextOffset < markdownValue.length;
    setRenderedChunks(initialChunk.content ? [initialChunk.content] : []);
  }, [initialChunk, markdownValue.length, progressive]);

  const loadMore = useCallback(() => {
    if (loadMoreScheduledRef.current || !hasMoreRef.current) {
      return;
    }
    loadMoreScheduledRef.current = true;
    loadMoreTimerRef.current = window.setTimeout(() => {
      loadMoreTimerRef.current = null;
      loadMoreScheduledRef.current = false;
      setRenderedChunks(current => {
        const additions: string[] = [];
        let offset = nextOffsetRef.current;
        for (let index = 0; index < PROGRESSIVE_CHUNK_BATCH && offset < markdownValue.length; index += 1) {
          const chunk = takeNextMarkdownChunk(markdownValue, offset);
          additions.push(chunk.content);
          offset = chunk.nextOffset;
        }
        nextOffsetRef.current = offset;
        hasMoreRef.current = offset < markdownValue.length;
        return additions.length > 0 ? [...current, ...additions] : current;
      });
    }, 0);
  }, [markdownValue]);

  useEffect(() => () => {
    if (loadMoreTimerRef.current !== null) {
      window.clearTimeout(loadMoreTimerRef.current);
      loadMoreTimerRef.current = null;
    }
    loadMoreScheduledRef.current = false;
  }, [loadMore]);

  useEffect(() => {
    if (!progressive || !hasMoreRef.current) {
      return;
    }

    const container = scrollContainerRef.current;
    if (!container) {
      return;
    }

    const fillViewportIfNeeded = () => {
      if (shouldFillPreviewViewport(container.scrollHeight, container.clientHeight)) {
        loadMore();
      }
    };
    const handleScroll = () => {
      if (container.scrollHeight - container.scrollTop - container.clientHeight < 800) {
        loadMore();
      }
    };
    const resizeObserver = typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(fillViewportIfNeeded);

    container.addEventListener('scroll', handleScroll, { passive: true });
    window.addEventListener('resize', fillViewportIfNeeded);
    resizeObserver?.observe(container);
    if (contentRef.current) {
      resizeObserver?.observe(contentRef.current);
    }
    fillViewportIfNeeded();

    return () => {
      container.removeEventListener('scroll', handleScroll);
      window.removeEventListener('resize', fillViewportIfNeeded);
      resizeObserver?.disconnect();
    };
  }, [loadMore, progressive, renderedChunks.length]);

  return (
    <div ref={scrollContainerRef} className="m-editor-preview" data-bf-component="editor-tool" data-bf-part="meditorPreview">
      <div ref={contentRef} className="m-editor-preview-content">
        {frontmatter && (
          <section className="m-editor-preview-frontmatter" data-bf-component="editor-tool" data-bf-part="meditorFrontmatter">
            <header className="m-editor-preview-frontmatter__header">
              <span className="m-editor-preview-frontmatter__label">
                {t('editor.meditor.frontmatter.label')}
              </span>
            </header>
            <pre className="m-editor-preview-frontmatter__source">
              <code>{frontmatter.yaml}</code>
            </pre>
          </section>
        )}
        {renderedChunks.map((chunk, index) => (
          <div className="m-editor-preview-chunk" key={index} style={{ contentVisibility: 'auto', containIntrinsicSize: '800px' }}>
            <MarkdownRenderer content={chunk} basePath={basePath} />
          </div>
        ))}
        {progressive && hasMoreRef.current && <div className="m-editor-preview-load-more" />}
      </div>
    </div>
  );
};
