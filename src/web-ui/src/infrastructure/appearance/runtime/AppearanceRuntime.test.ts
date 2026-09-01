import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppearanceRegistry } from '../registry/AppearanceRegistry';
import type {
  AppearancePackage,
  AppearanceRendererAdapter,
  ThemeTokenAppearanceSettings,
} from '../types';
import { AppearanceRuntime } from './AppearanceRuntime';

function pkg(id: string, background: string): AppearancePackage {
  return {
    schema: 'bitfun.appearance',
    schemaVersion: 2,
    id,
    name: id,
    version: '1.0.0',
    mode: 'dark',
    renderers: {
      'theme-tokens': {
        version: 1,
        settings: { tokens: { '--bf-color-surface-canvas': background } },
      },
    },
  };
}

describe('AppearanceRuntime', () => {
  let dom: JSDOM;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><head></head><body></body></html>');
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    dom.window.close();
  });

  it('commits revisions atomically and rolls renderer adapters back on failure', async () => {
    const calls: Array<Readonly<ThemeTokenAppearanceSettings> | undefined> = [];
    const adapter: AppearanceRendererAdapter<'theme-tokens'> = {
      id: 'theme-tokens',
      validate: () => [],
      apply: async next => {
        calls.push(next ? { ...next } : undefined);
        if (next?.tokens['--bf-color-surface-canvas'] === 'fail') throw new Error('renderer failed');
      },
    };
    const registry = new AppearanceRegistry().registerRenderer(adapter).freeze();
    const runtime = new AppearanceRuntime(registry);
    const listener = vi.fn();
    runtime.subscribe(listener);

    await runtime.initialize(pkg('test.first', 'first'));
    expect(document.documentElement.getAttribute('data-bf-appearance')).toBe('test.first');
    expect(document.querySelectorAll('style[data-bf-appearance-runtime]')).toHaveLength(1);

    await expect(runtime.applyPackage(pkg('test.second', 'fail'))).rejects.toThrow('renderer failed');
    expect(runtime.getSnapshot()?.id).toBe('test.first');
    expect(document.documentElement.getAttribute('data-bf-appearance')).toBe('test.first');
    expect(document.querySelectorAll('style[data-bf-appearance-runtime]')).toHaveLength(1);
    expect(calls.at(-1)).toEqual({ tokens: { '--bf-color-surface-canvas': 'first' } });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('creates host-owned blob URLs for package assets and revokes replaced URLs', async () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValueOnce('blob:test-background');
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const registry = new AppearanceRegistry()
      .registerComponent({ id: 'surface', parts: [{ id: 'root' }] })
      .freeze();
    const runtime = new AppearanceRuntime(registry);
    const assetPackage: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 2,
      id: 'test.assets',
      name: 'Assets',
      version: '1.0.0',
      mode: 'dark',
      assets: {
        background: {
          kind: 'image',
          mimeType: 'image/png',
          source: { kind: 'package', path: 'assets/background.png' },
        },
      },
      components: {
        surface: {
          parts: {
            root: { base: { backgroundImage: { kind: 'asset', assetId: 'background' } } },
          },
        },
      },
    };
    await runtime.initialize(assetPackage, {
      background: {
        mimeType: 'image/png',
        bytes: new Uint8Array([1, 2, 3]).buffer,
        width: 1,
        height: 1,
      },
    });
    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(document.head.textContent).toContain('--bf-appearance-asset-background: url("blob:test-background")');

    await runtime.applyPackage({ ...assetPackage, id: 'test.assets-next', assets: undefined, components: undefined });
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:test-background');
  });

  it('keeps retired asset URLs alive until the UI releases their revision', async () => {
    vi.spyOn(URL, 'createObjectURL').mockReturnValueOnce('blob:test-background');
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const registry = new AppearanceRegistry()
      .registerComponent({ id: 'surface', parts: [{ id: 'root' }] })
      .freeze();
    const runtime = new AppearanceRuntime(registry);
    const assetPackage: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 2,
      id: 'test.assets',
      name: 'Assets',
      version: '1.0.0',
      mode: 'dark',
      assets: {
        background: {
          kind: 'image',
          mimeType: 'image/png',
          source: { kind: 'package', path: 'assets/background.png' },
        },
      },
      components: {
        surface: {
          parts: {
            root: { base: { backgroundImage: { kind: 'asset', assetId: 'background' } } },
          },
        },
      },
    };
    await runtime.initialize(assetPackage, {
      background: {
        mimeType: 'image/png',
        bytes: new Uint8Array([1, 2, 3]).buffer,
        width: 1,
        height: 1,
      },
    });
    revokeObjectURL.mockClear();
    const release = runtime.retainAssetRevision(1);

    await runtime.applyPackage({ ...assetPackage, id: 'test.assets-next', assets: undefined, components: undefined });
    expect(revokeObjectURL).not.toHaveBeenCalledWith('blob:test-background');

    release();
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:test-background');
  });

  it('resolves video background URLs without exposing the video as a CSS image variable', async () => {
    vi.spyOn(URL, 'createObjectURL')
      .mockReturnValueOnce('blob:test-motion')
      .mockReturnValueOnce('blob:test-poster');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const runtime = new AppearanceRuntime(new AppearanceRegistry().freeze());
    const assetPackage: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 2,
      id: 'test.video-background',
      name: 'Video Background',
      version: '1.0.0',
      mode: 'dark',
      requiredCapabilities: ['assets.v1', 'background-media.v1'],
      assets: {
        motion: { kind: 'video', mimeType: 'video/webm', source: { kind: 'package', path: 'assets/background.webm' } },
        poster: { kind: 'image', mimeType: 'image/png', source: { kind: 'package', path: 'assets/poster.png' } },
      },
      backgroundMedia: { kind: 'video', assetId: 'motion', posterAssetId: 'poster' },
    };

    const snapshot = await runtime.initialize(assetPackage, {
      motion: { mimeType: 'video/webm', bytes: new Uint8Array([1]).buffer, width: 1920, height: 1080, durationSeconds: 30 },
      poster: { mimeType: 'image/png', bytes: new Uint8Array([2]).buffer, width: 1920, height: 1080 },
    });

    expect(snapshot.backgroundMedia).toMatchObject({
      url: 'blob:test-motion',
      posterUrl: 'blob:test-poster',
      fit: 'cover',
      position: 'center',
    });
    expect(snapshot.cssText).not.toContain('--bf-appearance-asset-motion');
    expect(snapshot.cssText).toContain('--bf-appearance-asset-poster');
  });

  it('keeps a staged revision inactive until renderer transactions commit', async () => {
    const observations: Array<{ rootRevision: string | null; stagedCss: string }> = [];
    const adapter: AppearanceRendererAdapter<'theme-tokens'> = {
      id: 'theme-tokens',
      validate: () => [],
      apply: next => {
        if (next?.tokens['--bf-color-surface-canvas'] !== 'second') return;
        const stagedStyle = document.querySelector<HTMLStyleElement>(
          'style[data-bf-appearance-runtime="2"]',
        );
        observations.push({
          rootRevision: document.documentElement.getAttribute('data-bf-appearance-revision'),
          stagedCss: stagedStyle?.textContent ?? '',
        });
      },
    };
    const runtime = new AppearanceRuntime(
      new AppearanceRegistry().registerRenderer(adapter).freeze(),
    );

    await runtime.initialize(pkg('test.same-id', 'first'));
    await runtime.applyPackage(pkg('test.same-id', 'second'));

    expect(observations).toEqual([expect.objectContaining({ rootRevision: '1' })]);
    expect(observations[0].stagedCss).toContain(
      '[data-bf-appearance="test.same-id"][data-bf-appearance-revision="2"]',
    );
    expect(document.documentElement.getAttribute('data-bf-appearance-revision')).toBe('2');
    expect(document.querySelectorAll('style[data-bf-appearance-runtime]')).toHaveLength(1);
  });

  it('applies normal cascade after legacy styles and reserves important for explicit overrides', async () => {
    document.head.innerHTML = '<style>.legacy-gallery { color: rgb(255, 0, 0); }</style>';
    document.body.innerHTML = '<div class="legacy-gallery" data-bf-component="gallery-layout" data-bf-part="root">Run</div>';
    const registry = new AppearanceRegistry()
      .registerComponent({ id: 'gallery-layout', parts: [{ id: 'root' }] })
      .freeze();
    const runtime = new AppearanceRuntime(registry);
    const basePackage: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 2,
      id: 'test.computed-style',
      name: 'Computed Style',
      version: '1.0.0',
      mode: 'dark',
      components: {
        'gallery-layout': {
          parts: {
            root: { base: { color: { kind: 'hex', value: '#00ff00' } } },
          },
        },
      },
    };

    await runtime.initialize(basePackage);
    const gallery = document.querySelector('[data-bf-component="gallery-layout"]') as HTMLDivElement;
    expect(window.getComputedStyle(gallery).color).toBe('rgb(0, 255, 0)');

    gallery.style.color = 'rgb(255, 0, 0)';
    await runtime.applyPackage({
      ...basePackage,
      components: {
        'gallery-layout': {
          parts: {
            root: {
              cascade: 'override',
              base: { color: { kind: 'hex', value: '#0000ff' } },
            },
          },
        },
      },
    });
    expect(window.getComputedStyle(gallery).color).toBe('rgb(0, 0, 255)');
  });
});
