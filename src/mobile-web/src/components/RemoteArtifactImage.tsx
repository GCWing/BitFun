import React, { createContext, useContext, useEffect, useState } from 'react';
import { MobileButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';

export type ReadArtifactImage = (path: string, refresh?: boolean) => Promise<string>;
// The provider is owned by the displayed session and control-target epoch.
export const ArtifactImageReader = createContext<ReadArtifactImage | undefined>(undefined);

export function RemoteArtifactImage({
  path, alt, title, onDownload,
}: {
  path: string;
  alt?: string;
  title?: string;
  onDownload?: (path: string) => Promise<void>;
}) {
  const read = useContext(ArtifactImageReader);
  const { t } = useI18n();
  const [result, setResult] = useState<{ read: typeof read; path: string; src?: string; error?: string }>();
  const [attempt, setAttempt] = useState(0);
  // A changed reader/path must hide old pixels before the replacement effect runs.
  const current = result && result.read === read && result.path === path ? result : undefined;
  useEffect(() => {
    let cancelled = false;
    setResult(undefined);
    if (!read) return;
    (attempt > 0 ? read(path, true) : read(path)).then(
      (src) => { if (!cancelled) setResult({ read, path, src }); },
      (error: unknown) => {
        if (!cancelled) setResult({ read, path, error: error instanceof Error ? error.message : String(error) });
      },
    );
    return () => { cancelled = true; };
  }, [read, path, attempt]);

  if (current?.src) {
    return <img className="markdown-output-image" src={current.src} alt={alt || ''} title={title} loading="lazy"
      onError={() => setResult({ read, path, error: t('chat.fileUnavailable') })} />;
  }
  return (
    <span className="remote-artifact-image" role="status" title={current?.error}>
      <span>{alt || path.split(/[\\/]/).pop()} · {current?.error || !read ? t('chat.fileUnavailable') : t('chat.fileLoading')}</span>
      {current?.error && read && (
        <MobileButton appearance="plain" onClick={() => setAttempt(value => value + 1)}>{t('devices.retry')}</MobileButton>
      )}
      {onDownload && (
        <MobileButton appearance="plain" onClick={() => { void onDownload(path).catch(() => {}); }}>{t('chat.clickToDownload')}</MobileButton>
      )}
    </span>
  );
}
