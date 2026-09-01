export function normalizeRelayUrl(value: string): string | null {
  try {
    const normalized = value
      .replace(/^wss:\/\//, 'https://')
      .replace(/^ws:\/\//, 'http://')
      .replace(/\/ws\/?$/, '')
      .replace(/\/$/, '');
    const url = new URL(normalized);
    if (!['http:', 'https:'].includes(url.protocol)
      || !url.hostname
      || url.username
      || url.password
      || url.search
      || url.hash) {
      return null;
    }
    return url.toString().replace(/\/$/, '');
  } catch {
    return null;
  }
}

export function validPairingSecret(room: string | null, publicKey: string | null): boolean {
  return !!room
    && room.length <= 128
    && /^[A-Za-z0-9_-]+$/.test(room)
    && !['_store', 'page-data', 'pages'].includes(room)
    && !!publicKey
    && publicKey.length <= 512
    && /^[A-Za-z0-9+/=_-]+$/.test(publicKey);
}

/** Validate a Desktop-generated remote-control URL before navigating to it. */
export function parseScannedPairingLink(value: string, baseHref = window.location.href): string | null {
  try {
    const url = new URL(value.trim(), baseHref);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return null;
    if (!(url.hash === '#/pair' || url.hash.startsWith('#/pair?'))) return null;

    const params = new URLSearchParams(url.hash.replace(/^#\/pair\??/, ''));
    if (!validPairingSecret(params.get('room'), params.get('pk'))) return null;
    const relay = params.get('relay');
    if (relay && !normalizeRelayUrl(relay)) return null;
    return url.toString();
  } catch {
    return null;
  }
}
