import { browser, expect } from '@wdio/globals';

interface PopupCloseContractMetrics {
  ariaLabel: string | null;
  blockStartGap: number;
  edgeGap: number;
  iconEdgeGap: number;
  iconHeight: number;
  iconWidth: number;
  targetHeight: number;
  targetWidth: number;
  type: string;
}

/**
 * Assert the shared popup-dismiss geometry inside the real Desktop WebView.
 * The anchor can be the dialog itself or any descendant of its role=dialog root.
 */
export async function expectPopupCloseContract(anchorSelector: string): Promise<void> {
  const metrics = await browser.execute((selector: string) => {
    const anchor = document.querySelector<HTMLElement>(selector);
    if (!anchor) return null;

    const dialog = anchor.matches('[role="dialog"]')
      ? anchor
      : anchor.closest<HTMLElement>('[role="dialog"]') ?? anchor;
    const closeButton = dialog.querySelector<HTMLButtonElement>('[data-openbitfun-role="popup-close"]');
    const icon = closeButton?.querySelector<SVGElement>('svg');
    if (!closeButton || !icon) return null;

    const dialogRect = dialog.getBoundingClientRect();
    const closeRect = closeButton.getBoundingClientRect();
    const iconRect = icon.getBoundingClientRect();
    const closeStyle = window.getComputedStyle(closeButton);
    const iconStyle = window.getComputedStyle(icon);
    const direction = window.getComputedStyle(dialog).direction;
    const edgeGap = direction === 'rtl'
      ? closeRect.left - dialogRect.left
      : dialogRect.right - closeRect.right;
    const iconEdgeGap = direction === 'rtl'
      ? iconRect.left - dialogRect.left
      : dialogRect.right - iconRect.right;

    return {
      ariaLabel: closeButton.getAttribute('aria-label'),
      blockStartGap: closeRect.top - dialogRect.top,
      edgeGap,
      iconEdgeGap,
      iconHeight: Number.parseFloat(iconStyle.height),
      iconWidth: Number.parseFloat(iconStyle.width),
      targetHeight: Number.parseFloat(closeStyle.height),
      targetWidth: Number.parseFloat(closeStyle.width),
      type: closeButton.type,
    };
  }, anchorSelector);

  expect(metrics).not.toBeNull();
  expect(metrics?.targetWidth).toBeCloseTo(32, 1);
  expect(metrics?.targetHeight).toBeCloseTo(32, 1);
  expect(metrics?.iconWidth).toBeCloseTo(16, 1);
  expect(metrics?.iconHeight).toBeCloseTo(16, 1);
  expect(metrics?.blockStartGap).toBeGreaterThanOrEqual(3.5);
  expect(metrics?.edgeGap).toBeGreaterThanOrEqual(3.5);
  expect(Math.abs(
    (metrics?.edgeGap ?? Number.POSITIVE_INFINITY)
      - (metrics?.blockStartGap ?? Number.NEGATIVE_INFINITY),
  )).toBeLessThanOrEqual(1.1);
  expect((metrics?.iconEdgeGap ?? 0) - (metrics?.edgeGap ?? 0)).toBeGreaterThanOrEqual(7.5);
  expect((metrics?.iconEdgeGap ?? 0) - (metrics?.edgeGap ?? 0)).toBeLessThanOrEqual(9.5);
  expect(metrics?.ariaLabel?.trim().length).toBeGreaterThan(0);
  expect(metrics?.type).toBe('button');
}
