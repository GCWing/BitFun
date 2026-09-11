/** Presence belongs to the authenticated browser device, not a tab or heartbeat. */
export function getControlClientIdentity(controllerDeviceId: string): { id: string; name: string } {
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
  return { id: controllerDeviceId, name: [browser, platform].filter(Boolean).join(' · ') };
}
