import { ArrowRight, DownloadSimple } from '@phosphor-icons/react';
import type { Translate } from './i18n';
import { OPENBITFUN_DOWNLOAD_URL } from './links';

// An appearance package only applies inside the desktop client, so the catalog
// and every listing carry the same visible route to the official download page.
export type GetOpenBitFunPlacement = 'catalog' | 'listing';

export function GetOpenBitFunCta({
  placement,
  t,
}: {
  placement: GetOpenBitFunPlacement;
  t: Translate;
}) {
  return (
    <a
      className={`get-openbitfun get-openbitfun--${placement}`}
      href={OPENBITFUN_DOWNLOAD_URL}
      target="_blank"
      rel="noreferrer"
    >
      <span className="get-openbitfun__icon">
        <DownloadSimple size={20} weight="bold" aria-hidden="true" />
      </span>
      <span className="get-openbitfun__copy">
        <strong>{t('getOpenBitFunTitle')}</strong>
        <span>
          {t(placement === 'listing' ? 'getOpenBitFunListingNote' : 'getOpenBitFunCatalogNote')}
        </span>
        <span className="get-openbitfun__action">
          {t('getOpenBitFunAction')}
          <ArrowRight size={17} weight="bold" aria-hidden="true" />
        </span>
      </span>
    </a>
  );
}
