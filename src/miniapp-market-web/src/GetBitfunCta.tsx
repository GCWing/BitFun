import { ArrowRight, DownloadSimple } from '@phosphor-icons/react';
import type { MessageKey } from './i18n';
import { BITFUN_DOWNLOAD_URL } from './links';

// A .bfminiapp package is useless without the desktop client, so the catalog
// and every listing keep one explicit path to the official download page.
export type GetBitfunPlacement = 'catalog' | 'listing';

export function GetBitfunCta({
  placement,
  t,
}: {
  placement: GetBitfunPlacement;
  t: (key: MessageKey) => string;
}) {
  return (
    <a
      className={`get-bitfun get-bitfun-${placement}`}
      href={BITFUN_DOWNLOAD_URL}
      target="_blank"
      rel="noreferrer"
    >
      <span className="get-bitfun-icon">
        <DownloadSimple weight="bold" aria-hidden="true" />
      </span>
      <span className="get-bitfun-copy">
        <strong>{t('getBitfunTitle')}</strong>
        <span>
          {t(placement === 'listing' ? 'getBitfunListingNote' : 'getBitfunCatalogNote')}
        </span>
        <span className="get-bitfun-action">
          {t('getBitfunAction')}
          <ArrowRight weight="bold" aria-hidden="true" />
        </span>
      </span>
    </a>
  );
}
