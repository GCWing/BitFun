import React from 'react';

interface IconProps {
  size?: number;
  filled?: boolean;
  className?: string;
}

/**
 * Left panel icon.
 * - filled=false: left strip is outlined.
 * - filled=true: left strip is filled.
 */
export const PanelLeftIcon: React.FC<IconProps> = ({ 
  size = 14, 
  filled = false,
  className = '' 
}) => {
  return (
    <svg 
      width={size} 
      height={size} 
      viewBox="0 0 24 24" 
      fill="none" 
      stroke="currentColor" 
      strokeWidth="2" 
      strokeLinecap="round" 
      strokeLinejoin="round"
      className={className}
    >
      {/* Outer frame */}
      <rect x="3" y="3" width="18" height="18" rx="2" />
      {/* Divider is always visible */}
      <line x1="9" y1="3" x2="9" y2="21" />
      {/* Active state: fill left area with semi-transparent color */}
      {filled && (
        <rect 
          x="4" 
          y="4" 
          width="4.5" 
          height="16" 
          rx="1" 
          fill="currentColor" 
          fillOpacity="0.4"
          stroke="none"
        />
      )}
    </svg>
  );
};

/**
 * Right panel icon.
 * - filled=false: right strip is outlined.
 * - filled=true: right strip is filled.
 */
export const PanelRightIcon: React.FC<IconProps> = ({ 
  size = 14, 
  filled = false,
  className = '' 
}) => {
  return (
    <svg 
      width={size} 
      height={size} 
      viewBox="0 0 24 24" 
      fill="none" 
      stroke="currentColor" 
      strokeWidth="2" 
      strokeLinecap="round" 
      strokeLinejoin="round"
      className={className}
    >
      {/* Outer frame */}
      <rect x="3" y="3" width="18" height="18" rx="2" />
      {/* Divider is always visible */}
      <line x1="15" y1="3" x2="15" y2="21" />
      {/* Active state: fill right area with semi-transparent color */}
      {filled && (
        <rect 
          x="15.5" 
          y="4" 
          width="4.5" 
          height="16" 
          rx="1" 
          fill="currentColor" 
          fillOpacity="0.4"
          stroke="none"
        />
      )}
    </svg>
  );
};

/**
 * Center panel icon.
 * - filled=false: center strip is outlined.
 * - filled=true: center strip is filled.
 */
export const PanelCenterIcon: React.FC<IconProps> = ({
  size = 14,
  filled = false,
  className = ''
}) => {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      {/* Outer frame */}
      <rect x="3" y="3" width="18" height="18" rx="2" />
      {/* Center section dividers */}
      <line x1="9" y1="3" x2="9" y2="21" />
      <line x1="15" y1="3" x2="15" y2="21" />
      {/* Active state: fill middle area */}
      {filled && (
        <rect
          x="9.5"
          y="4"
          width="5"
          height="16"
          rx="1"
          fill="currentColor"
          fillOpacity="0.4"
          stroke="none"
        />
      )}
    </svg>
  );
};

