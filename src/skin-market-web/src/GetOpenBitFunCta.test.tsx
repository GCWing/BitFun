import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { GetOpenBitFunCta, type GetOpenBitFunPlacement } from './GetOpenBitFunCta';
import type { Translate } from './i18n';
import { OPENBITFUN_DOWNLOAD_URL } from './links';

const t = ((key: string) => key) as Translate;

describe('GetOpenBitFunCta', () => {
  it.each<GetOpenBitFunPlacement>(['catalog', 'listing'])(
    'sends %s visitors to the official download page',
    (placement) => {
      const markup = renderToStaticMarkup(<GetOpenBitFunCta placement={placement} t={t} />);

      expect(markup).toContain(`href="${OPENBITFUN_DOWNLOAD_URL}"`);
      expect(markup).toContain('rel="noreferrer"');
      expect(markup).toContain('getOpenBitFunTitle');
      expect(markup).toContain('getOpenBitFunAction');
    },
  );

  it('explains the surface the visitor is actually looking at', () => {
    expect(renderToStaticMarkup(<GetOpenBitFunCta placement="listing" t={t} />))
      .toContain('getOpenBitFunListingNote');
    expect(renderToStaticMarkup(<GetOpenBitFunCta placement="catalog" t={t} />))
      .toContain('getOpenBitFunCatalogNote');
  });
});
