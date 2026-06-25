import React from 'react';
import './Empty.scss';

export interface EmptyProps {
  /** Description text */
  description?: React.ReactNode;
  /** Custom icon */
  image?: React.ReactNode;
  /** Image size */
  imageSize?: 'small' | 'medium' | 'large' | number;
  /** Footer actions */
  children?: React.ReactNode;
  /** Custom class name */
  className?: string;
  /** Custom style */
  style?: React.CSSProperties;
}

const DefaultImage: React.FC<{ size: number }> = ({ size }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 120 120"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <circle cx="60" cy="60" r="50" fill="var(--card-bg-accent)" />
    <path
      d="M40 50C40 44.4772 44.4772 40 50 40H70C75.5228 40 80 44.4772 80 50V70C80 75.5228 75.5228 80 70 80H50C44.4772 80 40 75.5228 40 70V50Z"
      fill="color-mix(in srgb, var(--color-accent-500) 20%, transparent)"
    />
    <circle cx="52" cy="55" r="4" fill="var(--color-accent-400)" />
    <circle cx="68" cy="55" r="4" fill="var(--color-accent-400)" />
    <path
      d="M52 68C52 65.7909 53.7909 64 56 64H64C66.2091 64 68 65.7909 68 68"
      stroke="var(--color-accent-400)"
      strokeWidth="3"
      strokeLinecap="round"
    />
  </svg>
);

export const Empty: React.FC<EmptyProps> = ({
  description = 'No data',
  image,
  imageSize = 'medium',
  children,
  className = '',
  style,
}) => {
  const getImageSize = () => {
    if (typeof imageSize === 'number') return imageSize;
    const sizes = { small: 80, medium: 120, large: 160 };
    return sizes[imageSize];
  };

  const size = getImageSize();

  return (
    <div className={`bitfun-empty ${className}`} style={style}>
      <div className="bitfun-empty__image">
        {image || <DefaultImage size={size} />}
      </div>
      {description && (
        <div className="bitfun-empty__description">{description}</div>
      )}
      {children && <div className="bitfun-empty__footer">{children}</div>}
    </div>
  );
};

Empty.displayName = 'Empty';
