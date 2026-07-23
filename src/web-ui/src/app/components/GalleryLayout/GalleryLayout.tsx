import React from 'react';
import './GalleryLayout.scss';

interface GalleryLayoutProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  className?: string;
}

const GalleryLayout: React.FC<GalleryLayoutProps> = ({ children, className, ...rootProps }) => (
  <div {...rootProps} className={['gallery-layout', className].filter(Boolean).join(' ')}>
    <div className="gallery-layout__body">
      <div className="gallery-layout__body-inner">
        {children}
      </div>
    </div>
  </div>
);

export default GalleryLayout;
export type { GalleryLayoutProps };
