import React from 'react';
import { PageHeader } from '@bitfun/ui';
import './ConfigPageHeader.scss';

export interface ConfigPageHeaderProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  title: string;
  subtitle?: string;
  icon?: React.ReactNode;
  extra?: React.ReactNode;
  className?: string;
}

export const ConfigPageHeader: React.FC<ConfigPageHeaderProps> = ({
  title,
  subtitle,
  icon: _icon,
  extra,
  className = '',
  ...props
}) => {
  return (
    <div className={`bitfun-config-page-header ${className}`} data-bf-component="config" data-bf-part="pageHeader" {...props}>
      <div className="bitfun-config-page-header__inner" data-bf-component="config" data-bf-part="pageHeaderInner">
        <div className="bitfun-config-page-header__left">
          <div className="bitfun-config-page-header__info" data-bf-component="config" data-bf-part="pageHeaderInfo">
            <PageHeader
              level={2}
              size="display"
              title={<span data-bf-component="config" data-bf-part="pageHeaderTitle">{title}</span>}
              description={subtitle ? (
                <span className="bitfun-config-page-header__subtitle" data-bf-component="config" data-bf-part="pageHeaderSubtitle">{subtitle}</span>
              ) : undefined}
            />
          </div>
        </div>
        {extra && (
          <div className="bitfun-config-page-header__extra" data-bf-component="config" data-bf-part="pageHeaderExtra">
            {extra}
          </div>
        )}
      </div>
    </div>
  );
};

export default ConfigPageHeader;
