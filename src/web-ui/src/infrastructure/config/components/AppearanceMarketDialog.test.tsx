// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppearanceMarketDialog } from './AppearanceMarketDialog';

const mocks = vi.hoisted(() => ({
  browse: vi.fn(),
  getListing: vi.fn(),
  downloadRelease: vi.fn(),
  importPackage: vi.fn(),
  activate: vi.fn(),
  confirmDialog: vi.fn(async () => true),
  appearanceState: {
    appearances: [] as any[],
    selectedAppearanceId: 'system',
    status: 'ready',
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Button: ({ children, isLoading: _isLoading, iconOnly: _iconOnly, ...props }: any) => (
    <button {...props}>{children}</button>
  ),
  Modal: ({ isOpen, title, children }: any) => isOpen ? (
    <section role="dialog" aria-label={title}>{children}</section>
  ) : null,
  Search: ({ value, onChange, onSearch, inputAriaLabel }: any) => (
    <input
      aria-label={inputAriaLabel}
      value={value}
      onChange={event => onChange(event.target.value)}
      onKeyDown={event => event.key === 'Enter' && onSearch(event.currentTarget.value)}
    />
  ),
  Select: () => <div />,
  confirmDialog: mocks.confirmDialog,
}));

vi.mock('@/infrastructure/api/service-api/AppearanceMarketAPI', () => ({
  appearanceMarketAPI: {
    browse: mocks.browse,
    getListing: mocks.getListing,
    downloadRelease: mocks.downloadRelease,
  },
}));

vi.mock('@/infrastructure/appearance', () => ({
  useAppearance: () => ({
    ...mocks.appearanceState,
    importPackage: mocks.importPackage,
    activate: mocks.activate,
  }),
  getAppearancePackageValidationError: () => null,
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { success: vi.fn(), error: vi.fn() },
}));

vi.mock('@/shared/utils/version', () => ({
  getVersionInfo: () => ({ version: '0.2.15' }),
}));

const summary = {
  listingId: 'listing-1',
  slug: 'tokyo-night',
  packageId: 'community.tokyo-night',
  name: 'Tokyo Night',
  description: 'A calm dark appearance',
  author: 'Community',
  mode: 'dark',
  packageVersion: '2.0.0',
  latestRelease: 2,
  minBitfunVersion: '0.1.0',
  requiredCapabilities: ['components.v1'],
  owner: { githubId: 1, login: 'studio', avatarUrl: '' },
  previewUrl: `https://market.openbitfun.com/skin/api/v1/artifacts/previews/${'a'.repeat(64)}`,
  downloadCount: 10,
  publishedAt: 1,
} as const;

const release = {
  releaseId: 'release-2',
  listingId: 'listing-1',
  releaseNumber: 2,
  packageVersion: '2.0.0',
  minBitfunVersion: '0.1.0',
  packageSha256: 'a'.repeat(64),
  packageSize: 100,
  reviewBundleHash: 'b'.repeat(64),
  publishedAt: 1,
  yanked: false,
};

describe('AppearanceMarketDialog', () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    mocks.browse.mockReset().mockResolvedValue({ items: [summary] });
    mocks.getListing.mockReset().mockResolvedValue({
      ...summary,
      changelog: 'More polished',
      license: { spdxExpression: 'MIT' },
      releases: [release],
    });
    mocks.downloadRelease.mockReset().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer);
    mocks.importPackage.mockReset().mockResolvedValue(undefined);
    mocks.activate.mockReset().mockResolvedValue(undefined);
    mocks.confirmDialog.mockClear();
    mocks.appearanceState.appearances = [{
      id: 'community.tokyo-night',
      name: 'Tokyo Night',
      version: '1.0.0',
      mode: 'dark',
      source: 'imported',
      marketOrigin: {
        listingId: 'listing-1', slug: 'tokyo-night', releaseId: 'release-1',
        releaseNumber: 1, packageId: 'community.tokyo-night', packageVersion: '1.0.0',
        packageSha256: 'c'.repeat(64),
      },
      localOverride: false,
    }];
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('browses, opens detail, and updates through binary download plus Appearance import', async () => {
    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(container.textContent).toContain('Tokyo Night'));
    expect(container.textContent).toContain('package.market.updateAvailable');

    const listingButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('Tokyo Night'));
    await act(async () => listingButton?.click());
    await vi.waitFor(() => expect(container.textContent).toContain('More polished'));

    const updateButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('package.market.update'));
    await act(async () => updateButton?.click());

    await vi.waitFor(() => expect(mocks.importPackage).toHaveBeenCalledOnce());
    expect(mocks.downloadRelease).toHaveBeenCalledWith({
      slug: 'tokyo-night',
      releaseNumber: 2,
      packageId: 'community.tokyo-night',
      packageVersion: '2.0.0',
      packageSha256: 'a'.repeat(64),
      packageSize: 100,
    });
    expect(mocks.importPackage).toHaveBeenCalledWith(expect.any(ArrayBuffer), {
      marketOrigin: {
        listingId: 'listing-1',
        slug: 'tokyo-night',
        releaseId: 'release-2',
        releaseNumber: 2,
        packageId: 'community.tokyo-night',
        packageVersion: '2.0.0',
        packageSha256: 'a'.repeat(64),
      },
    });
    expect(mocks.activate).not.toHaveBeenCalled();
    expect(container.textContent).toContain('package.market.noAutoApply');
  });
});
