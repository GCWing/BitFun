import React from 'react';

export interface ConfigSectionProps {
   
  title?: string;
   
  description?: string;
   
  icon?: React.ReactNode;
   
  children: React.ReactNode;
   
  style?: React.CSSProperties;
   
  className?: string;
   
  collapsible?: boolean;
   
  defaultCollapsed?: boolean;
}

export const ConfigSection: React.FC<ConfigSectionProps> = ({
  title,
  description,
  icon,
  children,
  style,
  className = '',
  collapsible = false,
  defaultCollapsed = false,
}) => {
  const [collapsed, setCollapsed] = React.useState(defaultCollapsed);
  
  const sectionClass = `config-form-section ${className}`.trim();
  
  const handleToggle = () => {
    if (collapsible) {
      setCollapsed(!collapsed);
    }
  };

  const sectionStyle: React.CSSProperties = {
    ...style
  };

  const contentClass = 'config-form';
  const contentStyle: React.CSSProperties = {};

  return (
    <div className={sectionClass} style={sectionStyle}>
      {(title || description) && (
        <div style={{ flexShrink: 0 }}>
          {title && (
            <button
              type="button"
              className="config-form-section-title"
              disabled={!collapsible}
              onClick={handleToggle}
              aria-expanded={collapsible ? !collapsed : undefined}
            >
              {icon}
              {title}
              {collapsible && (
                <span className="config-form-section-chevron" aria-hidden="true">
                  ▼
                </span>
              )}
            </button>
          )}
          {description && (
            <p className="config-form-section-description">
              {description}
            </p>
          )}
        </div>
      )}
      
      <div
        className="config-form-section-collapse"
        data-open={!collapsible || !collapsed ? 'true' : 'false'}
        aria-hidden={collapsible && collapsed}
        {...(collapsible && collapsed ? { inert: '' } : {})}
      >
        <div className={contentClass} style={contentStyle}>
          {children}
        </div>
      </div>
    </div>
  );
};

ConfigSection.displayName = 'ConfigSection';
