import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createDefaultAppearanceRegistry } from '../registry/defaultAppearanceRegistry';
import {
  APPEARANCE_SCHEMA_VERSION,
  type AppearancePackage,
} from '../types';
import { AppearancePackageParser } from './AppearancePackageParser';

function pngHeader(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(24);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  bytes.set([0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52], 8);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, width);
  view.setUint32(20, height);
  return bytes;
}

function webmHeader(): Uint8Array {
  return new Uint8Array([
    0x1a, 0x45, 0xdf, 0xa3, 0x42, 0x82, 0x84,
    0x77, 0x65, 0x62, 0x6d, 0x42, 0x87, 0x81, 0x04, 0x00,
  ]);
}

function manifest(): AppearancePackage {
  return {
    schema: 'bitfun.appearance',
    schemaVersion: 1,
    id: 'test.archive',
    name: 'Archive',
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
      'gallery-layout': {
        parts: {
          root: { base: { backgroundImage: { kind: 'asset', assetId: 'background' } } },
        },
      },
    },
    renderers: {
      'css-tokens': {
        version: 1,
        settings: {
          tokens: {
            '--bf-appearance-token-color-bg-primary': '#101820',
          },
        },
      },
    },
  };
}

async function archive(value: unknown, extra?: (zip: JSZip) => void): Promise<ArrayBuffer> {
  const zip = new JSZip();
  zip.file('appearance.json', JSON.stringify(value));
  zip.file('assets/background.png', pngHeader(320, 180));
  extra?.(zip);
  return zip.generateAsync({ type: 'arraybuffer' });
}

describe('AppearancePackageParser', () => {
  const parser = new AppearancePackageParser(createDefaultAppearanceRegistry());

  it('imports only declared, validated image assets', async () => {
    const source = await archive(manifest());
    const stored = await parser.parse(source);
    expect(stored.manifest.id).toBe('test.archive');
    expect(stored.assets.background).toMatchObject({
      mimeType: 'image/png',
      width: 320,
      height: 180,
    });
    expect(stored.archiveSchemaVersion).toBe(APPEARANCE_SCHEMA_VERSION);
    expect(stored.manifest).toMatchObject({
      schemaVersion: APPEARANCE_SCHEMA_VERSION,
      renderers: {
        'theme-tokens': {
          version: 1,
          settings: {
            tokens: {
              '--bf-color-surface-canvas': '#101820',
            },
          },
        },
      },
    });
    expect(stored.manifest.renderers).not.toHaveProperty('css-tokens');

    const canonicalZip = await JSZip.loadAsync(stored.archive);
    const canonicalManifest = JSON.parse(
      await canonicalZip.file('appearance.json')!.async('string'),
    ) as AppearancePackage;
    expect(canonicalManifest).toEqual(stored.manifest);
    expect(canonicalManifest.schemaVersion).toBe(APPEARANCE_SCHEMA_VERSION);
    expect(canonicalManifest.renderers).not.toHaveProperty('css-tokens');
  });

  it('rejects undeclared files before storage', async () => {
    const source = await archive(manifest(), zip => {
      zip.file('payload.css', '.button { display: none }');
    });
    await expect(parser.parse(source)).rejects.toThrow('Undeclared archive file: payload.css');
  });

  it('rejects unknown schemas', async () => {
    const unsupported = { ...manifest(), schema: 'example.unknown' };
    await expect(parser.parse(await archive(unsupported))).rejects.toThrow('Schema must be bitfun.appearance');
  });

  it('imports validated background video metadata and its poster', async () => {
    const videoManifest: AppearancePackage = {
      ...manifest(),
      requiredCapabilities: ['assets.v1', 'background-media.v1'],
      assets: {
        motion: {
          kind: 'video',
          mimeType: 'video/webm',
          source: { kind: 'package', path: 'assets/background.webm' },
        },
        poster: {
          kind: 'image',
          mimeType: 'image/png',
          source: { kind: 'package', path: 'assets/poster.png' },
        },
      },
      backgroundMedia: { kind: 'video', assetId: 'motion', posterAssetId: 'poster' },
      components: undefined,
    };
    const zip = new JSZip();
    zip.file('appearance.json', JSON.stringify(videoManifest));
    zip.file('assets/background.webm', webmHeader());
    zip.file('assets/poster.png', pngHeader(1920, 1080));
    const parser = new AppearancePackageParser(
      createDefaultAppearanceRegistry(),
      async () => ({ width: 1920, height: 1080, durationSeconds: 42 }),
    );

    const stored = await parser.parse(await zip.generateAsync({ type: 'arraybuffer' }));
    expect(stored.assets.motion).toMatchObject({
      mimeType: 'video/webm',
      width: 1920,
      height: 1080,
      durationSeconds: 42,
    });
  });

  it('rejects oversized expanded entries before JSZip materializes them', async () => {
    const source = new Uint8Array(await archive(manifest()));
    const view = new DataView(source.buffer);
    for (let offset = 0; offset + 46 <= source.length; offset += 1) {
      if (view.getUint32(offset, true) === 0x02014b50) {
        view.setUint32(offset + 24, 65 * 1024 * 1024, true);
        break;
      }
    }
    await expect(parser.parse(source.buffer)).rejects.toThrow('Appearance archive entry is too large');
  });
});
