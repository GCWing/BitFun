import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { GetBitfunCta, type GetBitfunPlacement } from './GetBitfunCta';
import type { MessageKey } from './i18n';
import { BITFUN_DOWNLOAD_URL } from './links';

const t = (key: MessageKey): string => key;

describe('GetBitfunCta', () => {
  it.each<GetBitfunPlacement>(['catalog', 'listing'])(
    'sends %s visitors to the official download page',
    (placement) => {
      const markup = renderToStaticMarkup(<GetBitfunCta placement={placement} t={t} />);

      expect(markup).toContain(`href="${BITFUN_DOWNLOAD_URL}"`);
      expect(markup).toContain('rel="noreferrer"');
      expect(markup).toContain('getBitfunTitle');
      expect(markup).toContain('getBitfunAction');
    },
  );

  it('explains the surface the visitor is actually looking at', () => {
    expect(renderToStaticMarkup(<GetBitfunCta placement="listing" t={t} />))
      .toContain('getBitfunListingNote');
    expect(renderToStaticMarkup(<GetBitfunCta placement="catalog" t={t} />))
      .toContain('getBitfunCatalogNote');
  });
});
