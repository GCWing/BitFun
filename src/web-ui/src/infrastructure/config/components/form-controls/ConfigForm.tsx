import React from 'react';

export interface ConfigFormProps {
   
  children: React.ReactNode;
   
  onSubmit?: (e: React.FormEvent<HTMLFormElement>) => void;
   
  disabled?: boolean;
   
  style?: React.CSSProperties;
   
  className?: string;
   
  grid?: boolean;
   
  columns?: number;
   
  noValidate?: boolean;
}

export const ConfigForm: React.FC<ConfigFormProps> = ({
  children,
  onSubmit,
  disabled = false,
  style,
  className = '',
  grid = false,
  columns = 2,
  noValidate = true,
}) => {
  const formClass = `config-form ${grid ? 'config-form-grid' : ''} ${className}`.trim();
  
  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (onSubmit && !disabled) {
      onSubmit(e);
    }
  };

  const formStyle: React.CSSProperties = {
    ...style,
    ...(grid && {
      display: 'grid',
      gridTemplateColumns: `repeat(${columns}, 1fr)`,
      gap: 'var(--size-gap-4)'
    }),
    ...(disabled && {
      opacity: 0.6,
      pointerEvents: 'none' as const
    })
  };

  return (
    <form
      className={formClass}
      style={formStyle}
      onSubmit={handleSubmit}
      noValidate={noValidate}
    >
      {children}
    </form>
  );
};

ConfigForm.displayName = 'ConfigForm';


export interface ConfigFormRowProps {
   
  children: React.ReactNode;
   
  columns?: number;
   
  gap?: 'sm' | 'md' | 'lg';
   
  style?: React.CSSProperties;
   
  className?: string;
}

const gapSizes = {
  sm: 'var(--size-gap-2)', // 8px
  md: 'var(--size-gap-4)', // 16px
  lg: 'var(--size-gap-6)'  // 24px
};

export const ConfigFormRow: React.FC<ConfigFormRowProps> = ({
  children,
  columns = 2,
  gap = 'md',
  style,
  className = ''
}) => {
  const rowClass = `config-form-row ${className}`.trim();
  
  const rowStyle = {
    display: 'grid',
    gridTemplateColumns: `repeat(${columns}, 1fr)`,
    gap: gapSizes[gap],
    ...style
  };

  return (
    <div className={rowClass} style={rowStyle}>
      {children}
    </div>
  );
};

ConfigFormRow.displayName = 'ConfigFormRow';
