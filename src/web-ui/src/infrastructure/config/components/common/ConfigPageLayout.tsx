 

import React from 'react';
import './ConfigPageLayout.scss';

export interface ConfigPageLayoutProps {
   
  children: React.ReactNode;
   
  className?: string;
}

 
export const ConfigPageLayout: React.FC<ConfigPageLayoutProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`bitfun-config-page-layout ${className}`}>
      {children}
      {/* Real DOM spacer: keeps a guaranteed blank tail at the end of the scroll range. */}
      <div className="bitfun-config-page-layout__scroll-end-spacer" aria-hidden="true" />
    </div>
  );
};

export interface ConfigPageContentProps {
   
  children: React.ReactNode;
   
  className?: string;
}

 
export const ConfigPageContent: React.FC<ConfigPageContentProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`bitfun-config-page-content ${className}`}>
      <div className="bitfun-config-page-content__inner">
        {children}
      </div>
    </div>
  );
};

export interface ConfigPageSectionProps {
  title: string;
  /** Renders inline after the title (e.g. status badge). */
  titleSuffix?: React.ReactNode;
  description?: React.ReactNode;
  extra?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

export const ConfigPageSection: React.FC<ConfigPageSectionProps> = ({
  title,
  titleSuffix,
  description,
  extra,
  children,
  className = '',
}) => {
  return (
    <section className={`bitfun-config-page-section ${className}`}>
      <div className="bitfun-config-page-section__header">
        <div className="bitfun-config-page-section__heading">
          <div className="bitfun-config-page-section__title-row">
            <h3 className="bitfun-config-page-section__title">{title}</h3>
            {titleSuffix}
          </div>
          {description && (
            <p className="bitfun-config-page-section__description">{description}</p>
          )}
        </div>
        {extra && (
          <div className="bitfun-config-page-section__extra">
            {extra}
          </div>
        )}
      </div>
      <div className="bitfun-config-page-section__body">
        {children}
      </div>
    </section>
  );
};

export interface ConfigPageRowProps {
  label: React.ReactNode;
  description?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  align?: 'start' | 'center';
  /** Stack label above control for multi-line editors (textarea, code blocks, etc.) */
  multiline?: boolean;
  /** Flip to 3/7 ratio giving the control column more space */
  wide?: boolean;
  /**
   * ~40% label / ~60% control — middle ground between default (7:3) and wide (2:8).
   * Use when the label must stay on one line (e.g. two-word titles) and controls need room.
   */
  balanced?: boolean;
}

export const ConfigPageRow: React.FC<ConfigPageRowProps> = ({
  label,
  description,
  children,
  className = '',
  align = 'start',
  multiline = false,
  wide = false,
  balanced = false,
}) => {
  const cls = [
    'bitfun-config-page-row',
    `bitfun-config-page-row--${align}`,
    multiline && 'bitfun-config-page-row--multiline',
    wide && 'bitfun-config-page-row--wide',
    balanced && 'bitfun-config-page-row--balanced',
    className,
  ].filter(Boolean).join(' ');

  const gridStyle: React.CSSProperties | undefined = wide
    ? { gridTemplateColumns: 'minmax(0, 2fr) minmax(0, 8fr)' }
    : balanced
    ? { gridTemplateColumns: 'minmax(0, 2fr) minmax(0, 3fr)' }
    : multiline
    ? { gridTemplateColumns: '1fr' }
    : undefined;

  return (
    <div className={cls} style={gridStyle}>
      <div className="bitfun-config-page-row__meta">
        <p className="bitfun-config-page-row__label">{label}</p>
        {description && (
          <p className="bitfun-config-page-row__description">{description}</p>
        )}
      </div>
      <div className="bitfun-config-page-row__control">
        {children}
      </div>
    </div>
  );
};

export default ConfigPageLayout;



