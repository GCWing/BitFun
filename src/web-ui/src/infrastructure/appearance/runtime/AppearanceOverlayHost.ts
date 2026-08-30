/**
 * Shared mount point for portaled overlays (menus, popovers, modals, tooltips).
 *
 * The host is a layer, not just a container: the stylesheet imported here gives
 * it a stacking context above every container that hosts app UI, which is what
 * keeps an overlay visible and clickable no matter how high the z-index of the
 * subtree it portaled out of. See AppearanceOverlayHost.scss.
 */

import './AppearanceOverlayHost.scss';

const OVERLAY_HOST_ID = 'bitfun-appearance-overlay-host';

export function getAppearanceOverlayHost(): HTMLDivElement {
  const existing = document.getElementById(OVERLAY_HOST_ID);
  const HtmlDivElement = document.defaultView?.HTMLDivElement;
  if (HtmlDivElement && existing instanceof HtmlDivElement) return existing;

  const host = document.createElement('div');
  host.id = OVERLAY_HOST_ID;
  host.setAttribute('data-bf-overlay-host', 'true');
  document.body.appendChild(host);
  return host;
}
