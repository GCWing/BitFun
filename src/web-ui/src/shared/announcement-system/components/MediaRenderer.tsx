import React, { useEffect, useRef } from 'react';
import type { MediaConfig } from '../types';

interface Props {
  media: MediaConfig;
  /** Whether this page is currently visible (affects autoplay). */
  active: boolean;
}

interface LottieProps {
  src: string;
  active: boolean;
}

const LottieRenderer: React.FC<LottieProps> = ({ src, active }) => {
  const [LottieComponent, setLottieComponent] =
    React.useState<React.ComponentType<any> | null>(null);
  const [loadError, setLoadError] = React.useState(false);

  useEffect(() => {
    let cancelled = false;
    const pkg = '@lottiefiles/dotlottie-react';
    import(/* @vite-ignore */ pkg)
      .then((mod: any) => {
        if (!cancelled) setLottieComponent(() => mod.DotLottieReact ?? mod.default);
      })
      .catch(() => {
        if (!cancelled) setLoadError(true);
      });
    return () => { cancelled = true; };
  }, []);

  if (loadError) {
    return (
      <div className="announcement-media">
        <div className="announcement-media__placeholder">
          Lottie library not available
        </div>
      </div>
    );
  }

  if (!LottieComponent) {
    return (
      <div className="announcement-media">
        <div className="announcement-media__placeholder">Loading…</div>
      </div>
    );
  }

  return (
    <div className="announcement-media">
      <LottieComponent
        src={src}
        autoplay={active}
        loop
        style={{ width: '100%', height: '100%' }}
      />
    </div>
  );
};

/**
 * Renders a media asset inside a modal page.
 *
 * Supported types:
 * - `image` / `gif` → <img>
 * - `video`         → <video autoplay loop muted>
 * - `lottie`        → lazily loaded via @lottiefiles/dotlottie-react
 *
 * For Lottie, the package is dynamically imported to avoid bundle bloat when
 * no Lottie assets are in use.
 */
const MediaRenderer: React.FC<Props> = ({ media, active }) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  // Pause/play video when the page becomes active or inactive.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (active) {
      video.play().catch(() => {/* autoplay blocked – silent */});
    } else {
      video.pause();
    }
  }, [active]);

  if (media.media_type === 'image' || media.media_type === 'gif') {
    return (
      <div className="announcement-media">
        <img src={media.src} alt="" draggable={false} />
      </div>
    );
  }

  if (media.media_type === 'video') {
    return (
      <div className="announcement-media">
        <video
          ref={videoRef}
          src={media.src}
          loop
          muted
          playsInline
          autoPlay={active}
        />
      </div>
    );
  }

  if (media.media_type === 'lottie') {
    return <LottieRenderer src={media.src} active={active} />;
  }

  return (
    <div className="announcement-media">
      <div className="announcement-media__placeholder">
        Unsupported media type: {media.media_type}
      </div>
    </div>
  );
};

export default MediaRenderer;
