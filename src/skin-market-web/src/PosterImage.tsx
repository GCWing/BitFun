import { useEffect, useState } from 'react';
import { ImageBroken } from '@phosphor-icons/react';
import type { Translate } from './i18n';

interface PosterImageProps {
  alt: string;
  name: string;
  src: string;
  t: Translate;
  eager?: boolean;
}

export function PosterImage({ alt, name, src, t, eager = false }: PosterImageProps) {
  const [failed, setFailed] = useState(false);

  useEffect(() => setFailed(false), [src]);

  if (failed || !src) {
    return (
      <div className="poster-fallback" role="img" aria-label={`${alt}. ${t('previewUnavailable')}`}>
        <span className="poster-fallback__initial" aria-hidden="true">
          {Array.from(name.trim())[0]?.toUpperCase() ?? 'B'}
        </span>
        <span className="poster-fallback__label">
          <ImageBroken size={18} weight="regular" aria-hidden="true" />
          {t('previewUnavailable')}
        </span>
      </div>
    );
  }

  return (
    <img
      className="poster-image"
      src={src}
      alt={alt}
      loading={eager ? 'eager' : 'lazy'}
      decoding="async"
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
}
