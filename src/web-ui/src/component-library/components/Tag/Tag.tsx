/**
 * Tag component
 */

import React from 'react';
import './Tag.scss';

export interface TagProps {
  children: React.ReactNode;
  color?: 'blue' | 'green' | 'red' | 'yellow' | 'purple' | 'gray';
  size?: 'small' | 'medium' | 'large';
  title?: string;
  closable?: boolean;
  onClose?: () => void;
  closeAriaLabel?: string;
  className?: string;
  style?: React.CSSProperties;
  rounded?: boolean;
}

export const Tag: React.FC<TagProps> = ({
  children,
  color = 'blue',
  size = 'medium',
  title,
  closable = false,
  onClose,
  closeAriaLabel,
  className = '',
  style,
  rounded = false,
}) => {
  const classNames = [
    'tag',
    `tag--${color}`,
    `tag--${size}`,
    rounded && 'tag--rounded',
    className
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <span className={classNames} title={title} style={style} data-bf-component="tag" data-bf-part="root" data-bf-variant={color} data-bf-size={size} data-bf-state={rounded ? 'rounded' : undefined}>
      <span className="tag__content" data-bf-component="tag" data-bf-part="content">{children}</span>
      {closable && (
        <button
          type="button"
          className="tag__close"
          onClick={onClose}
          aria-label={closeAriaLabel}
         data-bf-component="tag" data-bf-part="close">
          ×
        </button>
      )}
    </span>
  );
};
