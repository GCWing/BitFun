import { useEffect, useState } from 'react';

/**
 * Keep the browser breakpoint aligned with the CSS master-detail breakpoint.
 * A detail pane needs comfortable room next to the 360px navigation pane.
 * Keep phones, tablets, and narrow browser windows on the drawer layout; only
 * desktop-sized viewports switch to persistent master-detail navigation.
 */
export const WIDE_LAYOUT_QUERY = '(min-width: 900px)';

function readWideLayout(): boolean {
  return typeof window !== 'undefined' && window.matchMedia(WIDE_LAYOUT_QUERY).matches;
}

export function useWideLayout(): boolean {
  const [isWide, setIsWide] = useState(readWideLayout);

  useEffect(() => {
    const query = window.matchMedia(WIDE_LAYOUT_QUERY);
    const update = (event: MediaQueryListEvent | MediaQueryList) => setIsWide(event.matches);
    update(query);
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);

  return isWide;
}
