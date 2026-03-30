/**
 * Card component
 */

import React, { forwardRef } from 'react';
import './Card.scss';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Card variant */
  variant?: 'default' | 'elevated' | 'subtle' | 'accent' | 'purple';
  /** Interactive (hover effect) */
  interactive?: boolean;
  /** Padding size */
  padding?: 'none' | 'small' | 'medium' | 'large';
  /** Corner radius */
  radius?: 'small' | 'medium' | 'large';
  /** Full width */
  fullWidth?: boolean;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(({
  children,
  variant = 'default',
  interactive = false,
  padding = 'medium',
  radius = 'medium',
  fullWidth = false,
  className = '',
  ...props
}, ref) => {
  const classNames = [
    'v-card',
    `v-card--${variant}`,
    `v-card--padding-${padding}`,
    `v-card--radius-${radius}`,
    interactive && 'v-card--interactive',
    fullWidth && 'v-card--full-width',
    className
  ].filter(Boolean).join(' ');

  return (
    <div
      ref={ref}
      className={classNames}
      {...props}
    >
      {children}
    </div>
  );
});

Card.displayName = 'Card';

export interface CardHeaderProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  /** Title */
  title?: React.ReactNode;
  /** Subtitle */
  subtitle?: React.ReactNode;
  /** Right-side actions */
  extra?: React.ReactNode;
}

export const CardHeader = forwardRef<HTMLDivElement, CardHeaderProps>(({
  title,
  subtitle,
  extra,
  children,
  className = '',
  ...props
}, ref) => {
  return (
    <div ref={ref} className={`v-card-header ${className}`} {...props}>
      <div className="v-card-header__content">
        {title && <div className="v-card-header__title">{title}</div>}
        {subtitle && <div className="v-card-header__subtitle">{subtitle}</div>}
        {children}
      </div>
      {extra && <div className="v-card-header__extra">{extra}</div>}
    </div>
  );
});

CardHeader.displayName = 'CardHeader';

export type CardBodyProps = React.HTMLAttributes<HTMLDivElement>;

export const CardBody = forwardRef<HTMLDivElement, CardBodyProps>(({
  children,
  className = '',
  ...props
}, ref) => {
  return (
    <div ref={ref} className={`v-card-body ${className}`} {...props}>
      {children}
    </div>
  );
});

CardBody.displayName = 'CardBody';

export interface CardFooterProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Alignment */
  align?: 'left' | 'center' | 'right' | 'between';
}

export const CardFooter = forwardRef<HTMLDivElement, CardFooterProps>(({
  children,
  align = 'right',
  className = '',
  ...props
}, ref) => {
  return (
    <div 
      ref={ref} 
      className={`v-card-footer v-card-footer--${align} ${className}`} 
      {...props}
    >
      {children}
    </div>
  );
});

CardFooter.displayName = 'CardFooter';
