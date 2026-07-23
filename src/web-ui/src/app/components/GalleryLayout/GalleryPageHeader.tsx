import React from 'react';

interface GalleryPageHeaderProps {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  extraContent?: React.ReactNode;
  className?: string;
}

const GalleryPageHeader: React.FC<GalleryPageHeaderProps> = ({
  title,
  subtitle,
  actions,
  extraContent,
  className,
}) => (
  <div className={['gallery-page-header', className].filter(Boolean).join(' ')}>
    <div className="gallery-page-header__identity">
      <h2 className="gallery-page-header__title">{title}</h2>
      {subtitle ? <div className="gallery-page-header__subtitle">{subtitle}</div> : null}
      {extraContent ? <div className="gallery-page-header__extra">{extraContent}</div> : null}
    </div>
    {actions ? <div className="gallery-page-header__actions">{actions}</div> : null}
  </div>
);

export default GalleryPageHeader;
export type { GalleryPageHeaderProps };
