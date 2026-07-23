import React from 'react';

interface GallerySkeletonProps {
  count?: number;
  cardHeight?: number;
  minCardWidth?: number;
  className?: string;
}

const GallerySkeleton: React.FC<GallerySkeletonProps> = ({
  count = 6,
  cardHeight = 140,
  minCardWidth = 320,
  className,
}) => (
  <div
    className={['gallery-grid', 'gallery-grid--skeleton', className].filter(Boolean).join(' ')}
    style={{
      '--gallery-grid-min': `${minCardWidth}px`,
      '--gallery-skeleton-height': `${cardHeight}px`,
    } as React.CSSProperties}
  >
    {Array.from({ length: count }).map((_, index) => (
      <div
        key={`gallery-skeleton-${index}`}
        className="gallery-skeleton-card"
        style={{ '--surface-stagger-index': index } as React.CSSProperties}
      />
    ))}
  </div>
);

export default GallerySkeleton;
export type { GallerySkeletonProps };
