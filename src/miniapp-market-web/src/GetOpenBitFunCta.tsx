import { ArrowRight, DownloadSimple } from '@phosphor-icons/react';
import type { MessageKey } from './i18n';
import { OPENBITFUN_DOWNLOAD_URL } from './links';

// A .bfminiapp package is useless without the desktop client, so the catalog
// and every listing keep one explicit path to the official download page.
export type GetOpenBitFunPlacement = 'catalog' | 'listing';

export function GetOpenBitFunCta({
  placement,
  t,
}: {
  placement: GetOpenBitFunPlacement;
  t: (key: MessageKey) => string;
}) {
  return (
    <a
      className={`get-openbitfun get-openbitfun-${placement}`}
      href={OPENBITFUN_DOWNLOAD_URL}
      target="_blank"
      rel="noreferrer"
    >
      <span className="get-openbitfun-icon">
        <DownloadSimple weight="bold" aria-hidden="true" />
      </span>
      <span className="get-openbitfun-copy">
        <strong>{t('getOpenBitFunTitle')}</strong>
        <span>
          {t(placement === 'listing' ? 'getOpenBitFunListingNote' : 'getOpenBitFunCatalogNote')}
        </span>
        <span className="get-openbitfun-action">
          {t('getOpenBitFunAction')}
          <ArrowRight weight="bold" aria-hidden="true" />
        </span>
      </span>
    </a>
  );
}
