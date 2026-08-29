import React from 'react';
import { ScrollArea } from '@bitfun/ui';
import './GalleryLayout.scss';

interface GalleryLayoutProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  className?: string;
}

const GalleryLayout: React.FC<GalleryLayoutProps> = ({ children, className, ...rootProps }) => (
  <div
    data-bf-component="gallery-layout"
    data-bf-part="root"
    {...rootProps}
    className={['gallery-layout', className].filter(Boolean).join(' ')}
  >
    <ScrollArea className="gallery-layout__body" data-bf-component="gallery-layout" data-bf-part="body">
      <div className="gallery-layout__body-inner" data-bf-component="gallery-layout" data-bf-part="content">
        {children}
      </div>
    </ScrollArea>
  </div>
);

export default GalleryLayout;
export type { GalleryLayoutProps };
