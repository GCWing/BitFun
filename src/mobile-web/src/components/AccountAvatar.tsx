import React, { useState } from 'react';

export default function AccountAvatar({ url }: { url?: string }) {
  const [failedUrl, setFailedUrl] = useState<string>();
  return <span className="mobile-account-avatar" aria-hidden="true">
    {url && url !== failedUrl
      ? <img src={url} alt="" referrerPolicy="no-referrer" onError={() => setFailedUrl(url)} />
      : <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="1.7"><circle cx="12" cy="8" r="4" /><path d="M4 22v-2a8 8 0 0 1 16 0v2" /></svg>}
  </span>;
}
