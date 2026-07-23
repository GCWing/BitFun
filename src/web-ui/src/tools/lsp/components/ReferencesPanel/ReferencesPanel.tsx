/**
 * References panel shown as a floating overlay.
 */

import React, { useCallback, useMemo } from 'react';
import { createRoot, Root } from 'react-dom/client';
import { FileText, X, ChevronRight } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { IconButton } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './ReferencesPanel.scss';

const log = createLogger('ReferencesPanel');

export interface ReferenceLocation {
  uri: string;
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
  /** Optional line preview text. */
  text?: string;
}

interface GroupedReferences {
  filePath: string;
  fileName: string;
  references: Array<{
    location: ReferenceLocation;
    lineNumber: number;
    preview: string;
  }>;
}

export interface ReferencesPanelProps {
  /** Reference locations to render. */
  references: ReferenceLocation[];
  /** Symbol name used for the query (optional). */
  symbolName?: string;
  /** Panel anchor position (viewport coords). */
  position: { x: number; y: number };
  /** Close callback. */
  onClose: () => void;
  /** Click callback for a single reference entry. */
  onReferenceClick: (reference: ReferenceLocation) => void;
  /** Max panel height in px. */
  maxHeight?: number;
}

export const ReferencesPanel: React.FC<ReferencesPanelProps> = ({
  references,
  symbolName,
  position,
  onClose,
  onReferenceClick,
  maxHeight = 400,
}) => {
  const { t } = useI18n('tools');
  const groupedReferences = useMemo(() => {
    const groups = new Map<string, GroupedReferences>();

    references.forEach((ref) => {
      const filePath = ref.uri;
      const fileName = extractFileName(filePath);

      if (!groups.has(filePath)) {
        groups.set(filePath, {
          filePath,
          fileName,
          references: [],
        });
      }

      const group = groups.get(filePath)!;
      group.references.push({
        location: ref,
        lineNumber: ref.range.start.line + 1,
        preview: ref.text || t('lsp.referencesPanel.previewFallback'),
      });
    });

    const sortedGroups = Array.from(groups.values());
    sortedGroups.forEach((group) => {
      group.references.sort((a, b) => a.lineNumber - b.lineNumber);
    });
    sortedGroups.sort((a, b) => a.fileName.localeCompare(b.fileName));

    return sortedGroups;
  }, [references, t]);

  function extractFileName(uri: string): string {
    const match = uri.match(/[^/\\]+$/);
    return match ? match[0] : uri;
  }

  const handleReferenceClick = useCallback(
    (ref: ReferenceLocation) => {
      onReferenceClick(ref);
    },
    [onReferenceClick]
  );

  const panelStyle = useMemo(() => {
    const style: React.CSSProperties = {
      maxHeight: `${maxHeight}px`,
    };

    const viewportWidth = window.innerWidth;
    const panelWidth = 500;

    if (position.x + panelWidth > viewportWidth - 20) {
      style.right = `${viewportWidth - position.x}px`;
    } else {
      style.left = `${position.x}px`;
    }

    const viewportHeight = window.innerHeight;
    if (position.y + maxHeight > viewportHeight - 20) {
      style.bottom = `${viewportHeight - position.y}px`;
    } else {
      style.top = `${position.y}px`;
    }

    return style;
  }, [position, maxHeight]);

  if (references.length === 0) {
    return (
      <div className="references-panel" style={panelStyle}>
        <div className="references-panel__header">
          <div className="references-panel__title">
            <FileText size={16} />
            <span>{t('lsp.referencesPanel.emptyTitle')}</span>
          </div>
          <IconButton 
            className="references-panel__close" 
            onClick={onClose}
            size="small"
            variant="ghost"
          >
            <X size={16} />
          </IconButton>
        </div>
        <div className="references-panel__empty">
          {symbolName
            ? t('lsp.referencesPanel.emptyWithSymbol', { symbol: symbolName })
            : t('lsp.referencesPanel.emptyDescription')}
        </div>
      </div>
    );
  }

  return (
    <div className="references-panel" style={panelStyle}>
      <div className="references-panel__header">
        <div className="references-panel__title">
          <FileText size={16} />
          <span>
            {symbolName
              ? t('lsp.referencesPanel.titleWithSymbol', { symbol: symbolName })
              : t('lsp.referencesPanel.title')}
            <span className="references-panel__count">({references.length})</span>
          </span>
        </div>
        <IconButton 
          className="references-panel__close" 
          onClick={onClose}
          size="small"
          variant="ghost"
        >
          <X size={16} />
        </IconButton>
      </div>

      <div className="references-panel__content">
        {groupedReferences.map((group) => (
          <div key={group.filePath} className="references-panel__file-group">
            <div className="references-panel__file-header" title={group.filePath}>
              <FileText size={14} />
              <div className="references-panel__file-path">
                {group.filePath.replace(/^file:\/\/\//, '')}
              </div>
              <div className="references-panel__file-count">
                {group.references.length}
              </div>
            </div>

            <div className="references-panel__reference-list">
              {group.references.map((ref, index) => (
                <div
                  key={`${group.filePath}-${index}`}
                  className="references-panel__reference-item"
                  onClick={() => handleReferenceClick(ref.location)}
                >
                  <ChevronRight size={14} className="references-panel__reference-icon" />
                  <div className="references-panel__reference-line">
                    {ref.lineNumber}
                  </div>
                  <div className="references-panel__reference-preview">
                    {ref.preview.trim() || t('lsp.referencesPanel.emptyLine')}
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

/**
 * References Panel Controller
 * Creates and manages the panel under document.body.
 */
export class ReferencesPanelController {
  private container: HTMLDivElement | null = null;
  private root: Root | null = null;

  /**
   * Show the references panel.
   */
  show(
    references: ReferenceLocation[],
    position: { x: number; y: number },
    options: {
      symbolName?: string;
      onReferenceClick: (ref: ReferenceLocation) => void;
    }
  ): void {
    if (!this.container) {
      this.container = document.createElement('div');
      this.container.className = 'references-panel-container';
      
      this.container.style.position = 'fixed';
      this.container.style.top = '0';
      this.container.style.left = '0';
      this.container.style.width = '100%';
      this.container.style.height = '100%';
      this.container.style.pointerEvents = 'none';
      this.container.style.zIndex = '99999';
      
      document.body.appendChild(this.container);

      this.root = createRoot(this.container);

      document.addEventListener('mousedown', this.handleOutsideClick);
      document.addEventListener('keydown', this.handleEscapeKey);
    }

    if (this.root) {
      this.root.render(
        <ReferencesPanel
          references={references}
          symbolName={options.symbolName}
          position={position}
          onClose={() => this.hide()}
          onReferenceClick={options.onReferenceClick}
        />
      );
    } else {
      log.error('Root is null, cannot render');
    }
  }

  /**
   * Close when clicking outside.
   */
  private handleOutsideClick = (event: MouseEvent) => {
    if (this.container && !this.container.contains(event.target as Node)) {
      this.hide();
    }
  };

  /**
   * Close on Escape.
   */
  private handleEscapeKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      this.hide();
    }
  };

  /**
   * Hide the panel.
   */
  hide(): void {
    document.removeEventListener('mousedown', this.handleOutsideClick);
    document.removeEventListener('keydown', this.handleEscapeKey);

    if (this.root) {
      this.root.unmount();
      this.root = null;
    }

    if (this.container) {
      document.body.removeChild(this.container);
      this.container = null;
    }
  }

  /**
   * Whether the panel is currently visible.
   */
  isVisible(): boolean {
    return this.container !== null;
  }
}
