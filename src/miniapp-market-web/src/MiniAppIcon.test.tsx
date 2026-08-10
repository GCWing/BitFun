import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { MiniAppIcon } from './MiniAppIcon';

describe('MiniAppIcon', () => {
  it.each([
    ['Aperture', 'lucide-aperture'],
    ['Grid3x3', 'lucide-grid-3x3'],
    ['git-pull-request', 'lucide-git-pull-request'],
  ])('renders the supported metadata identifier %s as an icon', (name, className) => {
    const markup = renderToStaticMarkup(<MiniAppIcon name={name} />);

    expect(markup).toContain('<svg');
    expect(markup).toContain(className);
    expect(markup).not.toContain(`>${name}<`);
  });

  it('uses a stable icon fallback for unknown or empty metadata', () => {
    expect(renderToStaticMarkup(<MiniAppIcon name="unknown-market-icon" />))
      .toContain('lucide-box');
    expect(renderToStaticMarkup(<MiniAppIcon name="" />))
      .toContain('lucide-box');
  });
});
