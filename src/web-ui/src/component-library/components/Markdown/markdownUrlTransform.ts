import { defaultUrlTransform } from 'react-markdown';

import { CANVAS_ARTIFACT_REF_SCHEME } from '@/shared/utils/canvasArtifactRef';

/**
 * Schemes the Markdown renderer handles itself. They must survive URL
 * normalization to reach the link handlers in `Markdown.tsx`; rehype-sanitize
 * still gates the result against the same list.
 */
const APP_LINK_SCHEMES = [
  CANVAS_ARTIFACT_REF_SCHEME,
  'computer',
  'file',
  'tab',
  'visualization',
];

/**
 * react-markdown's default transform blanks the href of any scheme outside
 * http(s)/mailto/xmpp, which silently kills the app's own link schemes — the
 * link renders, the click does nothing. Keep its behaviour for everything else.
 */
export function markdownUrlTransform(url: string): string {
  const colon = url.indexOf(':');
  if (colon > 0) {
    const slash = url.indexOf('/');
    const questionMark = url.indexOf('?');
    const numberSign = url.indexOf('#');
    const isScheme =
      (slash === -1 || colon < slash) &&
      (questionMark === -1 || colon < questionMark) &&
      (numberSign === -1 || colon < numberSign);
    if (isScheme && APP_LINK_SCHEMES.includes(url.slice(0, colon).toLowerCase())) {
      return url;
    }
  }
  return defaultUrlTransform(url);
}
