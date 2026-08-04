// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import listingFixture from '../../shared/appearance-market-contract-fixtures/listing-detail.json';
import type { Translate } from './i18n';
import type { AppearanceListingDetail } from './types';
import { DetailPage } from './DetailPage';

const mocks = vi.hoisted(() => ({
  detail: vi.fn(),
  yankRelease: vi.fn(),
  unpublishListing: vi.fn(),
}));

vi.mock('./api', async (importOriginal) => ({
  ...await importOriginal<typeof import('./api')>(),
  skinMarketApi: {
    detail: mocks.detail,
    yankRelease: mocks.yankRelease,
    unpublishListing: mocks.unpublishListing,
  },
}));

vi.mock('./PosterImage', () => ({
  PosterImage: ({ alt }: { alt: string }) => <div role="img" aria-label={alt} />,
}));

const detail: AppearanceListingDetail = {
  ...listingFixture,
  mode: 'dark',
};

const t = ((key: string) => key) as Translate;

describe('DetailPage moderation', () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    mocks.detail.mockReset().mockResolvedValue(detail);
    mocks.yankRelease.mockReset().mockResolvedValue(undefined);
    mocks.unpublishListing.mockReset().mockResolvedValue(undefined);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it('lets an admin permanently remove a release after supplying a reason', async () => {
    const onNavigate = vi.fn();
    await act(async () => {
      root.render(
        <DetailPage
          catalogSearch=""
          isAdmin
          locale="en-US"
          onNavigate={onNavigate}
          slug="ocean-night"
          t={t}
        />,
      );
    });
    await vi.waitFor(() => expect(container.textContent).toContain('Ocean Night'));

    const yankButton = [...container.querySelectorAll('button')]
      .find((button) => button.textContent?.includes('yankRelease'));
    const unpublishButton = [...container.querySelectorAll('button')]
      .find((button) => button.textContent?.includes('unpublishListing'));
    expect(yankButton?.disabled).toBe(true);
    expect(unpublishButton).toBeDefined();

    const reason = container.querySelector<HTMLTextAreaElement>('#moderation-reason');
    await act(async () => {
      if (!reason) throw new Error('moderation reason missing');
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        'value',
      )?.set;
      valueSetter?.call(reason, 'Unsafe package content');
      reason.dispatchEvent(new Event('input', { bubbles: true }));
    });

    expect(yankButton?.disabled).toBe(false);
    await act(async () => yankButton?.click());

    await vi.waitFor(() => expect(mocks.yankRelease).toHaveBeenCalledWith(
      'appearance-release-fixture-2',
      'Unsafe package content',
    ));
    expect(window.confirm).toHaveBeenCalledWith('yankConfirm');
    expect(onNavigate).toHaveBeenCalledWith('/skin/');
  });

  it('lets an admin unpublish the whole Skin listing after confirmation', async () => {
    const onNavigate = vi.fn();
    await act(async () => {
      root.render(
        <DetailPage
          catalogSearch="?q=ocean"
          isAdmin
          locale="en-US"
          onNavigate={onNavigate}
          slug="ocean-night"
          t={t}
        />,
      );
    });
    await vi.waitFor(() => expect(container.textContent).toContain('Ocean Night'));

    const reason = container.querySelector<HTMLTextAreaElement>('#moderation-reason');
    await act(async () => {
      if (!reason) throw new Error('moderation reason missing');
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        'value',
      )?.set;
      valueSetter?.call(reason, 'Repeated policy violations');
      reason.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const unpublishButton = [...container.querySelectorAll('button')]
      .find((button) => button.textContent?.includes('unpublishListing'));
    expect(unpublishButton?.disabled).toBe(false);
    await act(async () => unpublishButton?.click());

    await vi.waitFor(() => expect(mocks.unpublishListing).toHaveBeenCalledWith(
      'appearance-listing-fixture-1',
      'Repeated policy violations',
    ));
    expect(window.confirm).toHaveBeenCalledWith('unpublishConfirm');
    expect(onNavigate).toHaveBeenCalledWith('/skin/?q=ocean');
  });

  it('does not render moderation controls for a regular visitor', async () => {
    await act(async () => {
      root.render(
        <DetailPage
          catalogSearch=""
          isAdmin={false}
          locale="en-US"
          onNavigate={() => undefined}
          slug="ocean-night"
          t={t}
        />,
      );
    });
    await vi.waitFor(() => expect(container.textContent).toContain('Ocean Night'));

    expect(container.querySelector('#moderation-reason')).toBeNull();
    expect(container.textContent).not.toContain('yankRelease');
    expect(container.textContent).not.toContain('unpublishListing');
  });
});
