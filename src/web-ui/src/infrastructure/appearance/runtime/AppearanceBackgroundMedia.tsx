import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ResolvedAppearanceBackgroundMedia } from '../types';

export function AppearanceBackgroundMediaLayer({
  media,
  revision,
  retainRevision,
}: {
  media: Readonly<ResolvedAppearanceBackgroundMedia> | undefined;
  revision?: number;
  retainRevision?: (revision: number) => () => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [presentedUrl, setPresentedUrl] = useState<string | null>(null);

  useLayoutEffect(() => {
    if (revision === undefined || !retainRevision) return undefined;
    return retainRevision(revision);
  }, [retainRevision, revision]);

  useEffect(() => {
    const video = videoRef.current;
    const mediaUrl = media?.url;
    if (!video || !mediaUrl) return undefined;
    const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null;
    let cancelled = false;
    let videoFrameCallback: number | null = null;
    let outerAnimationFrame: number | null = null;
    let innerAnimationFrame: number | null = null;

    const cancelPresentedFrame = (): void => {
      if (videoFrameCallback !== null && typeof video.cancelVideoFrameCallback === 'function') {
        video.cancelVideoFrameCallback(videoFrameCallback);
      }
      if (outerAnimationFrame !== null) window.cancelAnimationFrame(outerAnimationFrame);
      if (innerAnimationFrame !== null) window.cancelAnimationFrame(innerAnimationFrame);
      videoFrameCallback = null;
      outerAnimationFrame = null;
      innerAnimationFrame = null;
    };
    const revealPresentedFrame = (): void => {
      if (cancelled) return;
      cancelPresentedFrame();
      if (typeof video.requestVideoFrameCallback === 'function') {
        videoFrameCallback = video.requestVideoFrameCallback(() => {
          videoFrameCallback = null;
          if (!cancelled) setPresentedUrl(mediaUrl);
        });
        return;
      }
      outerAnimationFrame = window.requestAnimationFrame(() => {
        outerAnimationFrame = null;
        innerAnimationFrame = window.requestAnimationFrame(() => {
          innerAnimationFrame = null;
          if (!cancelled) setPresentedUrl(mediaUrl);
        });
      });
    };
    const synchronizePlayback = (): void => {
      cancelPresentedFrame();
      if (document.hidden || reducedMotion?.matches) {
        setPresentedUrl(null);
        video.pause();
        return;
      }
      setPresentedUrl(null);
      void video.play().then(revealPresentedFrame).catch(() => {
        if (!cancelled) setPresentedUrl(null);
      });
    };
    document.addEventListener('visibilitychange', synchronizePlayback);
    reducedMotion?.addEventListener?.('change', synchronizePlayback);
    synchronizePlayback();
    return () => {
      cancelled = true;
      cancelPresentedFrame();
      document.removeEventListener('visibilitychange', synchronizePlayback);
      reducedMotion?.removeEventListener?.('change', synchronizePlayback);
      video.pause();
    };
  }, [media?.url]);

  if (!media?.url || !media.posterUrl) return null;
  return (
    <div
      className="openbitfun-appearance-background-media"
      aria-hidden="true"
      data-openbitfun-background-media="video"
      style={{
        backgroundImage: `url("${media.posterUrl}")`,
        backgroundPosition: media.position ?? 'center',
        backgroundSize: media.fit ?? 'cover',
      }}
    >
      <video
        ref={videoRef}
        className="openbitfun-appearance-background-media__video"
        src={media.url}
        poster={media.posterUrl}
        autoPlay
        loop
        muted
        playsInline
        preload="auto"
        disablePictureInPicture
        tabIndex={-1}
        data-media-ready={presentedUrl === media.url ? 'true' : 'false'}
        style={{
          objectFit: media.fit ?? 'cover',
          objectPosition: media.position ?? 'center',
        }}
      />
    </div>
  );
}
