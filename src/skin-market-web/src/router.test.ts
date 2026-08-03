import { describe, expect, it } from 'vitest';
import { appearancePath, catalogPath, parseMarketRoute } from './router';

describe('Skin Market routing', () => {
  it('recognizes the catalog with or without its trailing slash', () => {
    expect(parseMarketRoute('/skin')).toEqual({ kind: 'catalog' });
    expect(parseMarketRoute('/skin/')).toEqual({ kind: 'catalog' });
  });

  it('recognizes stable appearance detail slugs', () => {
    expect(parseMarketRoute('/skin/appearances/ocean-night/')).toEqual({
      kind: 'detail',
      slug: 'ocean-night',
    });
    expect(parseMarketRoute('/skin/appearances/Ocean_Night')).toEqual({ kind: 'not-found' });
  });

  it('builds base-aware catalog and detail links', () => {
    expect(appearancePath('ocean-night')).toBe('/skin/appearances/ocean-night');
    expect(catalogPath('?mode=dark')).toBe('/skin/?mode=dark');
  });
});
