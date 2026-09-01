// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppearancePackageValidationError } from '@/infrastructure/appearance';
import {
  AppearancePackageConfigSection,
  AppearancePackageFailurePanel,
} from './AppearancePackageConfigSection';

const getPreviewAssetMock = vi.hoisted(() => vi.fn());
const appearanceStateMock = vi.hoisted(() => ({
  selectedAppearanceId: 'sample.appearance',
}));

vi.mock('react-i18next', async importOriginal => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/infrastructure/appearance', async importOriginal => ({
  ...await importOriginal<typeof import('@/infrastructure/appearance')>(),
  SYSTEM_APPEARANCE_ID: 'system',
  useAppearance: () => ({
    appearances: [{
      id: 'sample.appearance',
      name: 'Sample Appearance',
      version: '1.0.0',
      author: 'Studio',
      mode: 'dark',
      source: 'imported',
      importedAt: '2026-07-28T00:00:00.000Z',
    }],
    selectedAppearanceId: appearanceStateMock.selectedAppearanceId,
    getPreviewAsset: getPreviewAssetMock,
    importPackage: vi.fn(),
    exportPackage: vi.fn(),
    activate: vi.fn(),
    deletePackage: vi.fn(),
  }),
}));

describe('AppearancePackageConfigSection', () => {
  afterEach(() => {
    appearanceStateMock.selectedAppearanceId = 'sample.appearance';
    getPreviewAssetMock.mockReset();
  });

  it('renders the selected package card and appearance management actions', () => {
    const html = renderToStaticMarkup(<AppearancePackageConfigSection />);
    expect(html).toContain('Sample Appearance');
    expect(html).toContain('data-testid="appearance-package-card"');
    expect(html).toContain('package.market.open');
    expect(html).toContain('package.import');
    expect(html).toContain('aria-label="package.export"');
    expect(html).toContain('aria-label="package.delete"');
    expect(html).toContain('accept=".bitfun-appearance,.zip,application/zip"');
    expect(html).toContain('data-bf-part="packageSection"');
    expect(html).toContain('data-bf-part="packageActions"');
    expect(html).toContain('data-bf-component="button"');
    expect(html.match(/data-bf-variant="fill"/g)).toHaveLength(2);
    expect(html.match(/data-size="md"/g)).toHaveLength(2);
    expect(html).toContain('bitfun-config-page-section');
    expect(html).not.toContain('appearance-package-config__action-button');
    expect(html).not.toContain('.bitfun-skin');
  });

  it('uses high-density artwork and a separate selection mark for the built-in package card', () => {
    appearanceStateMock.selectedAppearanceId = 'system';

    const html = renderToStaticMarkup(<AppearancePackageConfigSection />);

    expect(html).toContain('src="/assets/appearance/bitfun-default-preview@4x.png"');
    expect(html).toContain('appearance-package-config__card-preview--builtin');
    expect(html.match(/class="appearance-package-config__selected-mark"/g)).toHaveLength(1);
    expect(html).toContain('package.nativeName');
    expect(html).toContain('package.nativeDescription');

    const imageSrc = html.match(/<img[^>]+src="([^"]+)"/)?.[1];
    expect(imageSrc).toBeDefined();
    const artwork = readFileSync(resolve(process.cwd(), 'public', imageSrc!.slice(1)));
    expect(artwork.subarray(0, 8).toString('hex')).toBe('89504e470d0a1a0a');
    // Keep enough source pixels for the 240px card at high display scaling.
    expect(artwork.readUInt32BE(16)).toBeGreaterThanOrEqual(240 * 4);
    expect(artwork.readUInt32BE(20)).toBeGreaterThanOrEqual(120 * 4);
  });

  it('reuses the built-in high-density artwork in the hover preview without requesting a package asset', async () => {
    appearanceStateMock.selectedAppearanceId = 'system';
    const container = document.createElement('div');
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(<AppearancePackageConfigSection />);
      });

      const preview = container.querySelector<HTMLImageElement>('.appearance-package-config__card-preview img');
      await act(async () => {
        preview?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
        await new Promise(resolve => setTimeout(resolve, 220));
      });

      const largerPreview = document.querySelector<HTMLImageElement>(
        '[data-testid="appearance-package-preview-popover"] img',
      );
      expect(largerPreview?.getAttribute('src')).toBe('/assets/appearance/bitfun-default-preview@4x.png');
      expect(getPreviewAssetMock).not.toHaveBeenCalled();
    } finally {
      act(() => root.unmount());
    }
  });

  it('renders stored preview assets and releases their object URLs', async () => {
    const container = document.createElement('div');
    const root = createRoot(container);
    const createObjectURL = vi.fn(() => 'blob:appearance-preview');
    const revokeObjectURL = vi.fn();
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL });
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL });
    getPreviewAssetMock.mockResolvedValue({
      mimeType: 'image/webp',
      bytes: new Uint8Array([1, 2, 3]).buffer,
      width: 16,
      height: 9,
    });

    await act(async () => {
      root.render(<AppearancePackageConfigSection />);
      await Promise.resolve();
    });

    const preview = container.querySelector<HTMLImageElement>('.appearance-package-config__card-preview img');
    expect(preview?.src).toBe('blob:appearance-preview');
    expect(preview?.alt).toBe('');
    expect(preview?.closest('article')?.getAttribute('aria-label')).toBe('Sample Appearance');
    expect(container.querySelectorAll('.appearance-package-config__selected-mark')).toHaveLength(1);
    expect(container.querySelector('.appearance-package-config__card-preview--builtin')).toBeNull();
    expect(createObjectURL).toHaveBeenCalledOnce();

    await act(async () => {
      preview?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      await new Promise(resolve => setTimeout(resolve, 220));
    });

    const largerPreview = document.querySelector<HTMLImageElement>(
      '[data-testid="appearance-package-preview-popover"] img',
    );
    expect(largerPreview?.src).toBe('blob:appearance-preview');

    act(() => root.unmount());
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:appearance-preview');
  });

  it('renders validation failures as grouped component diagnostics', () => {
    const validationError = new AppearancePackageValidationError([
      {
        code: 'UNKNOWN_PART',
        path: 'components.toolbar-mode.parts.sessionMenu',
        message: 'Unknown part sessionMenu',
        context: {
          surfaceKind: 'component',
          surfaceId: 'toolbar-mode',
          partId: 'sessionMenu',
          allowedParts: ['root', 'header', 'content'],
        },
      },
      {
        code: 'UNKNOWN_PART',
        path: 'components.toolbar-mode.parts.input',
        message: 'Unknown part input',
        context: {
          surfaceKind: 'component',
          surfaceId: 'toolbar-mode',
          partId: 'input',
          allowedParts: ['root', 'header', 'content'],
        },
      },
    ]);
    const html = renderToStaticMarkup(
      <AppearancePackageFailurePanel
        failure={{ operation: 'import', validationError }}
        onDismiss={() => undefined}
      />,
    );

    expect(html).toContain('package.diagnostics.validationTitle');
    expect(html).toContain('package.diagnostics.componentGroup');
    expect(html).toContain('components.toolbar-mode.parts.sessionMenu');
    expect(html).toContain('components.toolbar-mode.parts.input');
    expect(html).toContain('packageDiagnosticAllowedParts');
    expect(html).not.toContain('Invalid appearance package');
  });
});
