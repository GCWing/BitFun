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
            <h4 
              className="config-form-section-title"
              style={{ 
                cursor: collapsible ? 'pointer' : 'default',
                userSelect: 'none'
              }}
              onClick={handleToggle}
            >
              {icon}
              {title}
              {collapsible && (
                <span 
                  style={{ 
                    marginLeft: '8px',
                    transform: collapsed ? 'rotate(-90deg)' : 'rotate(0deg)',
                    transition: 'transform 0.2s ease',
                    fontSize: '14px',
                    color: 'var(--color-text-muted)'
                  }}
                >
                  ▼
                </span>
              )}
            </h4>
          )}
          {description && (
            <p className="config-form-section-description">
              {description}
            </p>
          )}
        </div>
      )}
      
      {(!collapsible || !collapsed) && (
        <div className={contentClass} style={contentStyle}>
          {children}
        </div>
      )}
    </div>
  );
};

ConfigSection.displayName = 'ConfigSection';
