const OVERLAY_HOST_ID = 'bitfun-appearance-overlay-host';

export function getAppearanceOverlayHost(): HTMLDivElement {
  const existing = document.getElementById(OVERLAY_HOST_ID);
  if (existing instanceof HTMLDivElement) return existing;

  const host = document.createElement('div');
  host.id = OVERLAY_HOST_ID;
  host.setAttribute('data-bf-overlay-host', 'true');
  document.body.appendChild(host);
  return host;
}
