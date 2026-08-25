 

import React from 'react';
import './ConfigPageLayout.scss';

export interface ConfigPageLayoutProps extends React.HTMLAttributes<HTMLDivElement> {
   
  children: React.ReactNode;
   
  className?: string;
}

 
export const ConfigPageLayout: React.FC<ConfigPageLayoutProps> = ({
  children,
  className = '',
  ...props
}) => {
  return (
    <div className={`bitfun-config-page-layout ${className}`} data-bf-component="config" data-bf-part="root" {...props}>
      {children}
      {/* Real DOM spacer: keeps a guaranteed blank tail at the end of the scroll range. */}
      <div className="bitfun-config-page-layout__scroll-end-spacer" aria-hidden="true" />
    </div>
  );
};

export interface ConfigPageContentProps extends React.HTMLAttributes<HTMLDivElement> {
   
  children: React.ReactNode;
   
  className?: string;
  id?: string;
}

 
export const ConfigPageContent: React.FC<ConfigPageContentProps> = ({
  children,
  className = '',
  id,
  ...props
}) => {
  return (
    <div id={id} className={`bitfun-config-page-content ${className}`} data-bf-component="config" data-bf-part="content" {...props}>
      <div className="bitfun-config-page-content__inner" data-bf-component="config" data-bf-part="contentInner">
        {children}
      </div>
    </div>
  );
};

export interface ConfigPageSectionStackProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
}

/**
 * Keeps the standard page-level spacing when sections need a shared wrapper
 * for state, test hooks, or adjacent page controls.
 */
export const ConfigPageSectionStack: React.FC<ConfigPageSectionStackProps> = ({
  children,
  className = '',
  ...props
}) => {
  return (
    <div
      {...props}
      className={`bitfun-config-page-section-stack ${className}`.trim()}
      data-bf-component="config"
      data-bf-part="sectionStack"
    >
      {children}
    </div>
  );
};

export interface ConfigPageSectionProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
  title: string;
  /** Renders inline after the title (e.g. status badge). */
  titleSuffix?: React.ReactNode;
  description?: React.ReactNode;
  extra?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  /** Disable when children own the surface and the standard body background/radius should be removed. */
  mouseGlowSurface?: boolean;
}

export const ConfigPageSection: React.FC<ConfigPageSectionProps> = ({
  title,
  titleSuffix,
  description,
  extra,
  children,
  className = '',
  mouseGlowSurface = true,
  ...props
}) => {
  // A chrome-less body must drop the inset highlight too: the mouse glow treats any
  // inset shadow as a visual boundary and would trace a box around empty space.
  const bodyClassName = [
    'bitfun-config-page-section__body',
    !mouseGlowSurface && 'bitfun-config-page-section__body--flush',
  ].filter(Boolean).join(' ');

  return (
    <section className={`bitfun-config-page-section ${className}`} data-bf-component="config" data-bf-part="section" {...props}>
      <div className="bitfun-config-page-section__header" data-bf-component="config" data-bf-part="sectionHeader">
        <div className="bitfun-config-page-section__heading">
          <div className="bitfun-config-page-section__title-row">
            <h3 className="bitfun-config-page-section__title" data-bf-component="config" data-bf-part="sectionTitle">{title}</h3>
            {titleSuffix}
          </div>
          {description && (
            <p className="bitfun-config-page-section__description" data-bf-component="config" data-bf-part="sectionDescription">{description}</p>
          )}
        </div>
        {extra && (
          <div className="bitfun-config-page-section__extra">
            {extra}
          </div>
        )}
      </div>
      <div className={bodyClassName} data-bf-component="config" data-bf-part="sectionBody">
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
  const hasControl = children !== null && children !== undefined && children !== false;
  const cls = [
    'bitfun-config-page-row',
    `bitfun-config-page-row--${align}`,
    multiline && 'bitfun-config-page-row--multiline',
    wide && 'bitfun-config-page-row--wide',
    balanced && 'bitfun-config-page-row--balanced',
    !hasControl && 'bitfun-config-page-row--no-control',
    className,
  ].filter(Boolean).join(' ');

  const gridStyle: React.CSSProperties | undefined = !hasControl
    ? { gridTemplateColumns: 'minmax(0, 1fr)' }
    : wide
    ? { gridTemplateColumns: 'minmax(0, 2fr) minmax(0, 8fr)' }
    : balanced
    ? { gridTemplateColumns: 'minmax(0, 2fr) minmax(0, 3fr)' }
    : multiline
    ? { gridTemplateColumns: '1fr' }
    : undefined;

  return (
    <div
      className={cls}
      style={gridStyle}
      data-bf-component="config"
      data-bf-part="row"
      data-bf-align={align}
      data-bf-layout={wide ? 'wide' : balanced ? 'balanced' : multiline ? 'multiline' : 'default'}
    >
      <div className="bitfun-config-page-row__meta">
        {/* div (not p): label may contain buttons; button-in-p freezes React event path */}
        <div className="bitfun-config-page-row__label" data-bf-component="config" data-bf-part="rowLabel">{label}</div>
        {description ? (
          <div className="bitfun-config-page-row__description" data-bf-component="config" data-bf-part="rowDescription">{description}</div>
        ) : null}
      </div>
      {hasControl ? (
        <div className="bitfun-config-page-row__control" data-bf-component="config" data-bf-part="rowControl">
          {children}
        </div>
      ) : null}
    </div>
  );
};

export default ConfigPageLayout;
