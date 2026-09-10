let identity: { id: string; name: string } | undefined;

/** One identity per browser page, shared by managers across target/reconnect changes. */
export function getControlClientIdentity(): { id: string; name: string } {
  if (identity) return identity;
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  const id = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? 'Edge'
    : /Firefox\/|FxiOS\//.test(ua) ? 'Firefox'
      : /Chrome\/|CriOS\//.test(ua) ? 'Chrome'
        : /Safari\//.test(ua) ? 'Safari' : 'Browser';
  const platform = /iPhone|iPad|iPod/.test(ua) ? 'iOS'
    : /Android/.test(ua) ? 'Android'
      : /Windows/.test(ua) ? 'Windows'
        : /Macintosh/.test(ua) ? 'macOS'
          : /Linux/.test(ua) ? 'Linux' : '';
  identity = { id, name: [browser, platform].filter(Boolean).join(' · ') };
  return identity;
}
