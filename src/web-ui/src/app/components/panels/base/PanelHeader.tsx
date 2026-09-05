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
    <div data-openbitfun-component="panel-header" data-openbitfun-part="root" className={`openbitfun-panel-header ${className}`}>
      <h3 className="openbitfun-panel-header__title" data-openbitfun-component="panel-header" data-openbitfun-part="title">{title}</h3>
      {actions && (
        <div className="openbitfun-panel-header__actions" data-openbitfun-component="panel-header" data-openbitfun-part="actions">
          {actions}
        </div>
      )}
    </div>
  );
};

PanelHeader.displayName = 'PanelHeader';

export default PanelHeader;
