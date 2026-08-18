/**
 * EmptyState component.
 * Empty state display.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { Tooltip } from '@/component-library';
import './EmptyState.scss';

export interface EmptyStateProps {
  onClose?: () => void;
  children?: React.ReactNode;
}

export const EmptyState: React.FC<EmptyStateProps> = ({ onClose, children }) => {
  const { t } = useTranslation('components');
  const hasEmbeddedContent = children !== undefined && children !== null;

  const handleClose = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onClose?.();
  }, [onClose]);

  return (
    <div data-bf-component="content-canvas" data-bf-part="empty" data-bf-state="empty" className="canvas-empty-state">
      {onClose && (
        <div className="canvas-empty-state__toolbar" data-bf-component="content-canvas" data-bf-part="emptyToolbar">
          <Tooltip content={t('tabs.close')}>
            <button
              className="canvas-empty-state__close-btn"
              onClick={handleClose}
            >
              <X size={14} />
            </button>
          </Tooltip>
        </div>
      )}
      <div
        className={`canvas-empty-state__content${hasEmbeddedContent ? ' canvas-empty-state__content--embedded' : ''}`}
        data-bf-component="content-canvas"
        data-bf-part="emptyContent"
      >
        {hasEmbeddedContent ? children : (
          <div className="canvas-empty-state__message">
            <p>{t('canvas.noContentOpen')}</p>
          </div>
        )}
      </div>
    </div>
  );
};

EmptyState.displayName = 'EmptyState';

export default EmptyState;
