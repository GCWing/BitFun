import { ArrowRight, DownloadSimple } from '@phosphor-icons/react';
import type { Translate } from './i18n';
import { BITFUN_DOWNLOAD_URL } from './links';

// An appearance package only applies inside the desktop client, so the catalog
// and every listing carry the same visible route to the official download page.
export type GetBitfunPlacement = 'catalog' | 'listing';

export function GetBitfunCta({
  placement,
  t,
}: {
  placement: GetBitfunPlacement;
  t: Translate;
}) {
  return (
    <a
      className={`get-bitfun get-bitfun--${placement}`}
      href={BITFUN_DOWNLOAD_URL}
      target="_blank"
      rel="noreferrer"
    >
      <span className="get-bitfun__icon">
        <DownloadSimple size={20} weight="bold" aria-hidden="true" />
      </span>
      <span className="get-bitfun__copy">
        <strong>{t('getBitfunTitle')}</strong>
        <span>
          {t(placement === 'listing' ? 'getBitfunListingNote' : 'getBitfunCatalogNote')}
        </span>
        <span className="get-bitfun__action">
          {t('getBitfunAction')}
          <ArrowRight size={17} weight="bold" aria-hidden="true" />
        </span>
      </span>
    </a>
  );
}
