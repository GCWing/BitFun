/**
 * Mermaid editor header controls
 */

import React from 'react';
import { Layers, Save, Download, Plus, Minus, Home, Edit3, MessageSquarePlus, Wrench } from 'lucide-react';
import { IconButton } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './MermaidEditorHeader.css';

export interface MermaidEditorHeaderProps {
  showComponentLibrary: boolean;
  isDirty: boolean;
  onToggleComponentLibrary: () => void;
  onSave: () => void;
  onExport: (format: string) => void;
  zoomLevel?: number;
  onZoomIn?: () => void;
  onZoomOut?: () => void;
  onResetView?: () => void;
  onAddToChatInput?: () => void;
  hasError?: boolean;
  onFixError?: () => void;
  isFixing?: boolean;
  fixProgress?: { current: number; total: number };
  /** Whether edit mode is enabled (clicking nodes opens the editor) */
  isEditMode?: boolean;
  /** Toggle edit mode */
  onToggleEditMode?: () => void;
}

export const MermaidEditorHeader: React.FC<MermaidEditorHeaderProps> = ({
  showComponentLibrary,
  isDirty,
  onToggleComponentLibrary,
  onSave,
  onExport,
  zoomLevel = 100,
  onZoomIn,
  onZoomOut,
  onResetView,
  onAddToChatInput,
  hasError = false,
  onFixError,
  isFixing = false,
  fixProgress = { current: 0, total: 0 },
  isEditMode = false,
  onToggleEditMode,
}) => {
  const { t } = useI18n('mermaid-editor');
  
  return (
    <div className="mermaid-editor-header">
      <div className="header-left">
        {onToggleEditMode && (
          <IconButton
            className={`edit-mode-btn ${isEditMode ? 'active' : ''}`}
            onClick={onToggleEditMode}
            tooltip={isEditMode ? t('header.exitEditMode') : t('header.enterEditMode')}
            tooltipPlacement="bottom"
            size="small"
          >
            <Edit3 size={16} />
          </IconButton>
        )}

        {isEditMode && (
          <div className="view-controls">
            <IconButton
              className={showComponentLibrary ? 'active' : ''}
              onClick={onToggleComponentLibrary}
              tooltip={t('header.componentLibrary')}
              tooltipPlacement="bottom"
              size="small"
            >
              <Layers size={16} />
            </IconButton>
          </div>
        )}
      </div>

      <div className="header-center" />

      <div className="header-right">
        <div className="action-controls">
          {(hasError || isFixing) && onFixError && (
            <IconButton
              onClick={onFixError}
              disabled={isFixing}
              tooltip={isFixing ? `${t('header.fixing')} (${fixProgress.current}/${fixProgress.total})` : t('header.oneClickFix')}
              tooltipPlacement="bottom"
              size="small"
            >
              <Wrench size={16} />
            </IconButton>
          )}

          {onAddToChatInput && (
            <IconButton
              onClick={onAddToChatInput}
              tooltip={t('header.addToChat')}
              tooltipPlacement="bottom"
              size="small"
            >
              <MessageSquarePlus size={16} />
            </IconButton>
          )}

          {(onZoomIn || onZoomOut || onResetView) && (
            <div className="zoom-controls-header">
              {onZoomOut && (
                <IconButton
                  className="zoom-btn"
                  onClick={onZoomOut}
                  tooltip={t('header.zoomOut')}
                  tooltipPlacement="bottom"
                  size="xs"
                >
                  <Minus size={14} />
                </IconButton>
              )}

              <span className="zoom-level-text">{zoomLevel}%</span>

              {onZoomIn && (
                <IconButton
                  className="zoom-btn"
                  onClick={onZoomIn}
                  tooltip={t('header.zoomIn')}
                  tooltipPlacement="bottom"
                  size="xs"
                >
                  <Plus size={14} />
                </IconButton>
              )}

              {onResetView && (
                <IconButton
                  className="reset-btn"
                  onClick={onResetView}
                  tooltip={t('header.reset')}
                  tooltipPlacement="bottom"
                  size="xs"
                >
                  <Home size={14} />
                </IconButton>
              )}
            </div>
          )}

          <IconButton
            className={`save-btn ${isDirty ? 'dirty' : ''}`}
            onClick={onSave}
            disabled={!isDirty}
            tooltip={isDirty ? t('header.save') : t('header.noChanges')}
            tooltipPlacement="bottom"
            size="small"
          >
            <Save size={16} />
          </IconButton>
          
          <IconButton
            className="export-btn"
            onClick={() => onExport('png')}
            tooltip={t('header.export')}
            tooltipPlacement="bottom"
            size="small"
            data-testid="mermaid-export-png"
          >
            <Download size={16} />
          </IconButton>
        </div>
      </div>
    </div>
  );
};
