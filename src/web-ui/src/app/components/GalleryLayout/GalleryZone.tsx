import React from 'react';

interface GalleryZoneProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
  id?: string;
  title: string;
  titleAdornment?: React.ReactNode;
  subtitle?: React.ReactNode;
  tools?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

const GalleryZone: React.FC<GalleryZoneProps> = ({
  id,
  title,
  titleAdornment,
  subtitle,
  tools,
  children,
  className,
  ...sectionProps
}) => (
  <section {...sectionProps} id={id} className={['gallery-zone', className].filter(Boolean).join(' ')}>
    <div className="gallery-zone__header">
      <div className="gallery-zone__heading">
        <div className="gallery-zone__title-row">
          <span className="gallery-zone__title">{title}</span>
          {titleAdornment ? (
            <span className="gallery-zone__title-adornment">{titleAdornment}</span>
          ) : null}
        </div>
        {subtitle ? <span className="gallery-zone__subtitle">{subtitle}</span> : null}
      </div>
      {tools ? <div className="gallery-zone__tools">{tools}</div> : null}
    </div>
    {children}
  </section>
);

export default GalleryZone;
export type { GalleryZoneProps };
