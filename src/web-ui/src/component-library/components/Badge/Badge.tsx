/**
 * Badge component — borderless status label with semantic background color.
 */

import React from 'react';
import './Badge.scss';

export type BadgeVariant = 'neutral' | 'accent' | 'purple' | 'success' | 'warning' | 'error' | 'info';

export interface BadgeProps {
  children: React.ReactNode;
  variant?: BadgeVariant;
  className?: string;
}

export const Badge: React.FC<BadgeProps> = ({
  children,
  variant = 'neutral',
  className = '',
}) => {
  const classNames = ['badge', `badge--${variant}`, className]
    .filter(Boolean)
    .join(' ');

  return <span className={classNames}>{children}</span>;
};
