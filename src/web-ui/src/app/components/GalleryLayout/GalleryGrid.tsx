import React from 'react';

interface GalleryGridProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  minCardWidth?: number;
  className?: string;
}

const GalleryGrid: React.FC<GalleryGridProps> = ({
  children,
  minCardWidth = 320,
  className,
  style,
  ...gridProps
}) => (
  <div
    {...gridProps}
    className={['gallery-grid', className].filter(Boolean).join(' ')}
    style={{
      ...style,
      '--gallery-grid-min': `${minCardWidth}px`,
    } as React.CSSProperties}
  >
    {children}
  </div>
);

export default GalleryGrid;
export type { GalleryGridProps };
