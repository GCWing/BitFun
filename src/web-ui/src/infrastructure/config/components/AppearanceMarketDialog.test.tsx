// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppearanceMarketDialog } from './AppearanceMarketDialog';

const mocks = vi.hoisted(() => ({
  browse: vi.fn(),
  getListing: vi.fn(),
  downloadRelease: vi.fn(),
  listSubmissions: vi.fn(),
  chooseSubmissionPackage: vi.fn(),
  submitPackage: vi.fn(),
  withdrawSubmission: vi.fn(),
  listReviewSubmissions: vi.fn(),
  getReviewSubmission: vi.fn(),
  reviewSubmission: vi.fn(),
  importPackage: vi.fn(),
  activate: vi.fn(),
  confirmDialog: vi.fn(async () => true),
  appearanceState: {
    appearances: [] as any[],
    selectedAppearanceId: 'system',
    status: 'ready',
  },
  accountState: {
    resolved: true,
    status: 'signed-in',
    me: {
      user: { githubId: 1, login: 'reviewer', avatarUrl: '' },
      isAdmin: true,
    },
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@openbitfun/ui', () => ({
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  Button: ({ children, isLoading: _isLoading, loading: _loading, iconOnly: _iconOnly, ...props }: any) => (
    <button {...props}>{children}</button>
  ),
  Dialog: ({ open, children }: any) => open ? <section role="dialog">{children}</section> : null,
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: () => <button type="button" aria-label="Close" />,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
  DialogHeading: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
  SearchField: ({ value, onValueChange, onSearch, 'aria-label': ariaLabel }: any) => (
    <input
      aria-label={ariaLabel}
      value={value}
      onChange={event => onValueChange(event.target.value)}
      onKeyDown={event => event.key === 'Enter' && onSearch(event.currentTarget.value)}
    />
  ),
  Select: ({ options, onValueChange, ...props }: any) => (
    <select {...props} onChange={event => onValueChange?.(event.target.value)}>
      {options.map((option: any) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
  ),
  Field: ({ label, children }: any) => <label>{label}{children}</label>,
  Input: ({ leading, trailing, onChange, onValueChange, ...props }: any) => (
    <span>
      {leading}
      <input
        {...props}
        onChange={event => {
          onChange?.(event);
          onValueChange?.(event.currentTarget.value);
        }}
      />
      {trailing}
    </span>
  ),
  Textarea: ({ label, hint, errorMessage, showCount: _showCount, onChange, onValueChange, ...props }: any) => (
    <label>
      {label}
      <textarea
        {...props}
        onChange={event => {
          onChange?.(event);
          onValueChange?.(event.currentTarget.value);
        }}
      />
      {hint ?? errorMessage}
    </label>
  ),
  Tooltip: ({ children }: any) => <>{children}</>,
}));

vi.mock('@/infrastructure/confirm-dialog', () => ({
  confirmDialog: mocks.confirmDialog,
}));

vi.mock('@/features/market-account', () => ({
  MarketAccountControls: () => <div data-testid="shared-market-account-controls" />,
}));

vi.mock('@/infrastructure/market-account', () => ({
  useMarketAccount: () => mocks.accountState,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
    formatDate: () => 'Jan 1, 2026',
  }),
}));

vi.mock('@/infrastructure/api/service-api/AppearanceMarketAPI', () => ({
  appearanceMarketAPI: {
    browse: mocks.browse,
    getListing: mocks.getListing,
    downloadRelease: mocks.downloadRelease,
    listSubmissions: mocks.listSubmissions,
    chooseSubmissionPackage: mocks.chooseSubmissionPackage,
    submitPackage: mocks.submitPackage,
    withdrawSubmission: mocks.withdrawSubmission,
    listReviewSubmissions: mocks.listReviewSubmissions,
    getReviewSubmission: mocks.getReviewSubmission,
    reviewSubmission: mocks.reviewSubmission,
  },
}));

vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => true,
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
  getVersionInfo: () => ({ version: '1.0.0' }),
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
  minOpenBitFunVersion: '1.0.0',
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
  minOpenBitFunVersion: '1.0.0',
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
    mocks.listSubmissions.mockReset().mockResolvedValue([]);
    mocks.chooseSubmissionPackage.mockReset()
      .mockResolvedValue('/tmp/ocean-night.openbitfun-appearance');
    mocks.submitPackage.mockReset().mockResolvedValue({
      submissionId: 'submission-upload',
      slug: 'ocean-night',
      releaseNumber: 1,
      packageId: 'community.ocean-night',
      name: 'Ocean Night',
      description: 'A calm blue appearance',
      mode: 'dark',
      packageVersion: '1.0.0',
      minOpenBitFunVersion: '1.0.0',
      requiredCapabilities: [],
      changelog: 'Initial release.',
      license: { spdxExpression: 'MIT' },
      status: 'submitted',
      createdAt: 1,
      updatedAt: 1,
    });
    mocks.withdrawSubmission.mockReset();
    mocks.listReviewSubmissions.mockReset().mockResolvedValue([]);
    mocks.getReviewSubmission.mockReset();
    mocks.reviewSubmission.mockReset();
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
    expect(container.querySelector('[data-testid="shared-market-account-controls"]')).not.toBeNull();
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

  it('holds the grid with placeholder cards while the first page loads', async () => {
    let resolveBrowse: (page: unknown) => void = () => undefined;
    mocks.browse.mockImplementation(() => new Promise(resolve => {
      resolveBrowse = resolve;
    }));

    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });

    // Placeholder cards stand in for the real ones so the dialog keeps one size,
    // and the empty state never flashes before the first page resolves.
    expect(container.querySelectorAll('.appearance-market__card--skeleton').length)
      .toBeGreaterThan(0);
    expect(container.textContent).not.toContain('package.market.empty');

    await act(async () => {
      resolveBrowse({ items: [summary] });
      await Promise.resolve();
    });

    await vi.waitFor(() => expect(container.textContent).toContain('Tokyo Night'));
    expect(container.querySelector('.appearance-market__card--skeleton')).toBeNull();
  });

  it('keeps an empty result set on the empty state once loading settles', async () => {
    mocks.browse.mockResolvedValue({ items: [] });

    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });

    await vi.waitFor(() => expect(container.textContent).toContain('package.market.empty'));
    expect(container.querySelector('.appearance-market__card--skeleton')).toBeNull();
  });

  it('shows the shared-account submissions and admin review workflows', async () => {
    const submission = {
      submissionId: 'submission-1',
      slug: 'tokyo-night',
      releaseNumber: 2,
      packageId: 'community.tokyo-night',
      name: 'Tokyo Night candidate',
      description: 'Candidate package',
      mode: 'dark',
      packageVersion: '2.0.0',
      minOpenBitFunVersion: '1.0.0',
      requiredCapabilities: ['components.v1'],
      changelog: 'More polished',
      license: { spdxExpression: 'MIT' },
      status: 'submitted',
      createdAt: 1,
      updatedAt: 2,
    };
    mocks.listSubmissions.mockResolvedValue([submission]);
    mocks.listReviewSubmissions.mockResolvedValue([submission]);
    mocks.getReviewSubmission.mockResolvedValue({
      submission,
      manifest: { id: 'community.tokyo-night' },
      packageSha256: 'a'.repeat(64),
      previewSha256: 'b'.repeat(64),
      reviewBundleHash: 'c'.repeat(64),
    });

    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });

    const submissionsTab = [...container.querySelectorAll('button')]
      .find(button => button.textContent === 'package.market.views.submissions');
    await act(async () => submissionsTab?.click());
    await vi.waitFor(() => expect(container.textContent).toContain('Tokyo Night candidate'));
    expect(mocks.listSubmissions).toHaveBeenCalledOnce();

    const reviewTab = [...container.querySelectorAll('button')]
      .find(button => button.textContent === 'package.market.views.review');
    await act(async () => reviewTab?.click());
    await vi.waitFor(() => expect(mocks.getReviewSubmission).toHaveBeenCalledWith('submission-1'));
    expect(mocks.listReviewSubmissions).toHaveBeenCalledOnce();
    expect(container.textContent).toContain('package.market.review.approve');
    expect(container.textContent).toContain('package.market.review.reject');
  });

  it('shows a moderated Skin as unpublished instead of approved', async () => {
    mocks.listSubmissions.mockResolvedValue([{
      submissionId: 'submission-unpublished',
      listingId: 'listing-unpublished',
      slug: 'unpublished-skin',
      releaseNumber: 1,
      name: 'Unpublished Skin',
      minOpenBitFunVersion: '1.0.0',
      requiredCapabilities: [],
      changelog: 'Initial release',
      license: { spdxExpression: 'MIT' },
      status: 'approved',
      publicationStatus: 'unpublished',
      createdAt: 1,
      updatedAt: 2,
    }]);

    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });
    const submissionsTab = [...container.querySelectorAll('button')]
      .find(button => button.textContent === 'package.market.views.submissions');
    await act(async () => submissionsTab?.click());

    await vi.waitFor(() => expect(container.textContent).toContain('Unpublished Skin'));
    expect(container.textContent).toContain('package.market.submissions.status.unpublished');
    expect(container.textContent).not.toContain('package.market.submissions.status.approved');
  });

  it('submits a local Appearance package from the client workflow', async () => {
    await act(async () => {
      root.render(<AppearanceMarketDialog isOpen onClose={() => undefined} />);
      await Promise.resolve();
    });

    const submissionsTab = [...container.querySelectorAll('button')]
      .find(button => button.textContent === 'package.market.views.submissions');
    await act(async () => submissionsTab?.click());
    await vi.waitFor(() => expect(mocks.listSubmissions).toHaveBeenCalledOnce());

    const openButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('package.market.submissions.manual.open'));
    await act(async () => openButton?.click());

    const chooseButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent === 'package.market.submissions.manual.choose');
    await act(async () => chooseButton?.click());
    await vi.waitFor(() => expect(mocks.chooseSubmissionPackage).toHaveBeenCalledOnce());

    const licenseLabel = [...container.querySelectorAll('label')]
      .find(label => label.textContent?.startsWith(
        'package.market.submissions.manual.spdxExpression',
      ));
    const licenseInput = licenseLabel?.querySelector('input');
    await act(async () => {
      if (!licenseInput) throw new Error('license input missing');
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        'value',
      )?.set;
      valueSetter?.call(licenseInput, 'MIT');
      licenseInput.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const submitButton = [...container.querySelectorAll('button')]
      .find(button => button.textContent?.includes('package.market.submissions.manual.submit'));
    await act(async () => submitButton?.click());

    await vi.waitFor(() => expect(mocks.submitPackage).toHaveBeenCalledWith({
      packagePath: '/tmp/ocean-night.openbitfun-appearance',
      slug: undefined,
      minOpenBitFunVersion: '1.0.0',
      changelog: undefined,
      license: { spdxExpression: 'MIT' },
      repositoryUrl: undefined,
    }));
    expect(container.textContent).toContain('Ocean Night');
  });
});
