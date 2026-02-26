import React from 'react';
import './ConfigPageHeader.scss';

export interface ConfigPageHeaderProps {
  title: string;
  subtitle?: string;
  icon?: React.ReactNode;
  extra?: React.ReactNode;
  className?: string;
}

export const ConfigPageHeader: React.FC<ConfigPageHeaderProps> = ({
  title,
  subtitle: _subtitle,
  icon: _icon,
  extra,
  className = '',
}) => {
  return (
    <div className={`bitfun-config-page-header ${className}`}>
      <div className="bitfun-config-page-header__inner">
        <div className="bitfun-config-page-header__left">
          <div className="bitfun-config-page-header__info">
            <h2 className="bitfun-config-page-header__title">{title}</h2>
          </div>
        </div>
        {extra && (
          <div className="bitfun-config-page-header__extra">
            {extra}
          </div>
        )}
      </div>
    </div>
  );
};

export default ConfigPageHeader;
