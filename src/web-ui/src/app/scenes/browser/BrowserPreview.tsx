import './BrowserPreview.scss';
import type { BrowserViewportBounds } from './browserViewportGeometry';

/** Decorative frozen frame, below all DOM floating surfaces and native content. */
export function BrowserPreview({ src, bounds }: { src: string | null; bounds: BrowserViewportBounds | null }) {
  return src && bounds ? (
    <img
      className="browser-preview"
      data-openbitfun-component="browser-preview"
      data-openbitfun-part="image"
      src={src}
      style={bounds}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  ) : null;
}
