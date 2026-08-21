// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { AppearancePackageValidationError } from '@/infrastructure/appearance';
import {
  AppearancePackageConfigSection,
  AppearancePackageFailurePanel,
} from './AppearancePackageConfigSection';

const getPreviewAssetMock = vi.hoisted(() => vi.fn());

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
    selectedAppearanceId: 'sample.appearance',
    getPreviewAsset: getPreviewAssetMock,
    importPackage: vi.fn(),
    exportPackage: vi.fn(),
    activate: vi.fn(),
    deletePackage: vi.fn(),
  }),
}));

describe('AppearancePackageConfigSection', () => {
  it('renders a compact package selector with thumbnail and management actions', () => {
    const html = renderToStaticMarkup(<AppearancePackageConfigSection />);
    expect(html).toContain('Sample Appearance');
    expect(html).toContain('data-testid="appearance-package-select"');
    expect(html).toContain('data-testid="appearance-package-preview-thumbnail"');
    expect(html).toContain('aria-label="package.market.open"');
    expect(html).toContain('aria-label="package.import"');
    expect(html).toContain('aria-label="package.export"');
    expect(html).toContain('aria-label="package.delete"');
    expect(html).toContain('accept=".bitfun-appearance,.zip,application/zip"');
    expect(html).toContain('data-bf-part="packageSection"');
    expect(html).toContain('data-bf-part="packageSelect"');
    expect(html).not.toContain('data-bf-part="packageGrid"');
    expect(html).not.toContain('data-bf-part="packageCard"');
    expect(html).not.toContain('bitfun-config-page-section');
    expect(html).not.toContain('.bitfun-skin');
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

    const preview = container.querySelector<HTMLImageElement>('.appearance-package-config__preview img');
    expect(preview?.src).toBe('blob:appearance-preview');
    expect(preview?.alt).toBe('');
    expect(preview?.closest('[role="img"]')?.getAttribute('aria-label')).toBe('Sample Appearance');
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
