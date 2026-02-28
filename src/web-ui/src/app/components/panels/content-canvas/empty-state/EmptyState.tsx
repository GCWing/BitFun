/**
 * EmptyState component.
 * Empty state display.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import './EmptyState.scss';

export interface EmptyStateProps {
  // No callbacks needed
}

export const EmptyState: React.FC<EmptyStateProps> = () => {
  const { t } = useTranslation('components');

  return (
    <div className="canvas-empty-state">
      <div className="canvas-empty-state__content">
        {/* Message */}
        <div className="canvas-empty-state__message">
          <p>{t('canvas.noContentOpen')}</p>
        </div>
      </div>
    </div>
  );
};

EmptyState.displayName = 'EmptyState';

export default EmptyState;
