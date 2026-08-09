/**
 * Generic panel header with centered title and optional action buttons.
 */

import React from 'react';
import './PanelHeader.scss';

export interface PanelHeaderProps {
  title: string;
  actions?: React.ReactNode;
  className?: string;
}

export const PanelHeader: React.FC<PanelHeaderProps> = ({
  title,
  actions,
  className = '',
}) => {
  return (
    <div data-bf-component="panel-header" data-bf-part="root" className={`bitfun-panel-header ${className}`}>
      <h3 className="bitfun-panel-header__title" data-bf-component="panel-header" data-bf-part="title">{title}</h3>
      {actions && (
        <div className="bitfun-panel-header__actions" data-bf-component="panel-header" data-bf-part="actions">
          {actions}
        </div>
      )}
    </div>
  );
};

PanelHeader.displayName = 'PanelHeader';

export default PanelHeader;
