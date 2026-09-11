import { useEffect, useState, useSyncExternalStore } from 'react';
import { i18nService } from '@/infrastructure/i18n';
import { getActiveSurfaceScope, onSurfaceActivated } from '@/infrastructure/peer-device/deviceSurface';

export type SessionImageReader = (path: string, refresh?: boolean) => Promise<string>;

/** Session providers own these bytes; neither loading nor fallback may read local files. */
export function SessionMarkdownImage({ path, alt, title, read, download }: {
  path: string; alt?: string; title?: string; read: SessionImageReader;
  download?: (path: string) => Promise<void>;
}) {
  const scope = useSyncExternalStore(onSurfaceActivated, getActiveSurfaceScope, getActiveSurfaceScope);
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{
    read: SessionImageReader; path: string; epoch: number; src?: string; error?: string;
  }>();
  const current = result?.read === read && result?.path === path && result?.epoch === scope.epoch ? result : undefined;
  useEffect(() => {
    let cancelled = false;
    setResult(undefined);
    (attempt ? read(path, true) : read(path)).then(src => {
      if (!/^data:image\/(?:png|jpeg|gif|webp|bmp|svg\+xml|avif|x-icon);base64,/i.test(src)) {
        throw new Error(i18nService.t('components:markdown.imageUnavailable'));
      }
      if (!cancelled && scope.isCurrent()) setResult({ read, path, epoch: scope.epoch, src });
    }).catch((error: unknown) => {
      if (!cancelled && scope.isCurrent()) setResult({ read, path, epoch: scope.epoch, error: String(error) });
    });
    return () => { cancelled = true; };
  }, [path, read, scope, attempt]);

  if (current?.src) return <img src={current.src} alt={alt || ''} title={title} loading="lazy"
    onError={() => setResult({ read, path, epoch: scope.epoch, error: i18nService.t('components:markdown.imageUnavailable') })} />;
  return <span className="markdown-image-fallback" data-openbitfun-component="markdown" data-openbitfun-part="imageFallback" title={current?.error}>
    {alt || path.split('/').pop()} · {i18nService.t(current?.error ? 'components:markdown.imageUnavailable' : 'common:status.loading')}
    {current?.error && <button type="button" onClick={() => setAttempt(value => value + 1)}>{i18nService.t('common:retry')}</button>}
    {download && <button type="button" onClick={() => { void download(path).catch(() => {}); }}>{i18nService.t('common:actions.download')}</button>}
  </span>;
}
