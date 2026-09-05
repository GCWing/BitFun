import React from 'react';
import { Icon, type IconSource } from '@openbitfun/ui';

interface GalleryEmptyProps extends React.HTMLAttributes<HTMLDivElement> {
  icon: IconSource;
  message: React.ReactNode;
  isError?: boolean;
  action?: React.ReactNode;
  className?: string;
  testId?: string;
}

const GalleryEmpty: React.FC<GalleryEmptyProps> = ({
  icon,
  message,
  isError = false,
  action,
  className,
  testId,
  ...rootProps
}) => (
  <div
    {...rootProps}
    className={['gallery-empty', isError && 'gallery-empty--error', className].filter(Boolean).join(' ')}
    data-testid={testId}
  >
    <span className="gallery-empty__icon" aria-hidden="true">
      <Icon {...icon} />
    </span>
    <span className="gallery-empty__message">{message}</span>
    {action}
  </div>
);

export default GalleryEmpty;
export type { GalleryEmptyProps };
