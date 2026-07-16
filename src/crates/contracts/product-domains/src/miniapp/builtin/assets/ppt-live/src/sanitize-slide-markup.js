const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
const DELETE_WITH_CONTENT_TAGS = new Set([
  'script', 'iframe', 'object', 'embed', 'base', 'meta', 'link', 'template',
  'frame', 'frameset', 'portal',
]);

export const HTML_ALLOWED_TAGS = new Set([
  'html', 'head', 'body', 'title', 'style',
  'div', 'section', 'article', 'main', 'header', 'footer', 'nav', 'aside',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'p', 'span', 'strong', 'b', 'em', 'i',
  'u', 's', 'small', 'mark', 'sub', 'sup', 'code', 'pre', 'blockquote',
  'ul', 'ol', 'li', 'dl', 'dt', 'dd', 'br', 'hr', 'a',
  'table', 'caption', 'colgroup', 'col', 'thead', 'tbody', 'tfoot', 'tr', 'th', 'td',
  'figure', 'figcaption', 'img', 'label', 'time',
]);

export const SVG_ALLOWED_TAGS = new Set([
  'svg', 'g', 'defs', 'desc', 'title',
  'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon', 'path',
  'text', 'tspan', 'textpath', 'use', 'foreignobject', 'symbol', 'marker',
  'lineargradient', 'radialgradient', 'stop', 'pattern', 'clippath', 'mask', 'filter',
  'feblend', 'fecolormatrix', 'fecomponenttransfer', 'fecomposite', 'feconvolvematrix',
  'fediffuselighting', 'fedisplacementmap', 'fedistantlight', 'fedropshadow',
  'feflood', 'fefunca', 'fefuncb', 'fefuncg', 'fefuncr', 'fegaussianblur',
  'feimage', 'femerge', 'femergenode', 'femorphology', 'feoffset',
  'fepointlight', 'fespecularlighting', 'fespotlight', 'fetile', 'feturbulence',
]);

export const GLOBAL_ALLOWED_ATTRIBUTES = new Set([
  'id', 'class', 'style', 'title', 'role', 'lang', 'dir', 'hidden',
  'tabindex', 'draggable', 'spellcheck', 'contenteditable',
]);

export const TAG_ALLOWED_ATTRIBUTES = Object.freeze({
  img: new Set(['src', 'alt', 'width', 'height', 'loading', 'decoding']),
  ol: new Set(['start', 'reversed', 'type']),
  li: new Set(['value']),
  col: new Set(['span']),
  th: new Set(['colspan', 'rowspan', 'scope', 'headers', 'abbr']),
  td: new Set(['colspan', 'rowspan', 'headers']),
  label: new Set(['for']),
  time: new Set(['datetime']),
});

export const SVG_GLOBAL_ALLOWED_ATTRIBUTES = new Set([
  'id', 'class', 'style', 'transform', 'fill', 'fill-opacity', 'fill-rule',
  'stroke', 'stroke-width', 'stroke-opacity', 'stroke-linecap', 'stroke-linejoin',
  'stroke-dasharray', 'stroke-dashoffset', 'opacity', 'filter', 'clip-path',
  'clip-rule', 'mask', 'color', 'color-interpolation', 'color-interpolation-filters',
  'visibility', 'display', 'font-family', 'font-size', 'font-style', 'font-weight',
  'text-anchor', 'dominant-baseline', 'pointer-events', 'vector-effect',
  'paint-order', 'shape-rendering', 'text-rendering', 'stop-color', 'stop-opacity',
  'flood-color', 'flood-opacity', 'lighting-color',
  'marker-start', 'marker-mid', 'marker-end',
]);
const SVG_RESOURCE_PRESENTATION_ATTRIBUTES = new Set([
  'fill', 'stroke', 'filter', 'clip-path', 'mask',
  'marker-start', 'marker-mid', 'marker-end',
]);

const SVG_GEOMETRY_ATTRIBUTES = new Set([
  'x', 'y', 'x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'r', 'rx', 'ry',
  'width', 'height', 'd', 'points', 'pathlength',
]);
const SVG_FILTER_ATTRIBUTES = new Set([
  'in', 'in2', 'result', 'operator', 'k1', 'k2', 'k3', 'k4', 'mode', 'type',
  'values', 'tablevalues', 'slope', 'intercept', 'amplitude', 'exponent', 'offset',
  'stddeviation', 'edgemode', 'kernelmatrix', 'kernelunitlength', 'targetx', 'targety',
  'order', 'preservealpha', 'surfacescale', 'diffuseconstant', 'specularconstant',
  'specularexponent', 'limitingconeangle', 'azimuth', 'elevation',
  'pointsatx', 'pointsaty', 'pointsatz', 'basefrequency', 'numoctaves', 'seed',
  'stitchtiles', 'scale', 'xchannelselector', 'ychannelselector', 'radius', 'dx', 'dy',
]);
export const SVG_TAG_ALLOWED_ATTRIBUTES = Object.freeze({
  svg: new Set(['viewbox', 'width', 'height', 'x', 'y', 'preserveaspectratio', 'xmlns', 'xmlns:xlink']),
  symbol: new Set(['viewbox', 'x', 'y', 'width', 'height', 'preserveaspectratio']),
  marker: new Set(['viewbox', 'markerwidth', 'markerheight', 'markerunits', 'orient', 'preserveaspectratio', 'refx', 'refy']),
  rect: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  circle: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  ellipse: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  line: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  polyline: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  polygon: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  path: new Set([...SVG_GEOMETRY_ATTRIBUTES]),
  text: new Set(['x', 'y', 'dx', 'dy', 'rotate', 'textlength', 'lengthadjust']),
  tspan: new Set(['x', 'y', 'dx', 'dy', 'rotate', 'textlength', 'lengthadjust']),
  textpath: new Set(['href', 'xlink:href', 'startoffset', 'method', 'spacing', 'side', 'textlength', 'lengthadjust']),
  use: new Set(['x', 'y', 'width', 'height', 'href', 'xlink:href']),
  foreignobject: new Set(['x', 'y', 'width', 'height']),
  lineargradient: new Set(['x1', 'y1', 'x2', 'y2', 'gradientunits', 'gradienttransform', 'spreadmethod', 'href', 'xlink:href']),
  radialgradient: new Set(['cx', 'cy', 'r', 'fx', 'fy', 'fr', 'gradientunits', 'gradienttransform', 'spreadmethod', 'href', 'xlink:href']),
  stop: new Set(['offset']),
  pattern: new Set(['x', 'y', 'width', 'height', 'patternunits', 'patterncontentunits', 'patterntransform', 'viewbox', 'preserveaspectratio', 'href', 'xlink:href']),
  clippath: new Set(['clippathunits']),
  mask: new Set(['x', 'y', 'width', 'height', 'maskunits', 'maskcontentunits']),
  filter: new Set(['x', 'y', 'width', 'height', 'filterunits', 'primitiveunits']),
});

function canonicalizeCssEscapes(value) {
  let current = String(value || '');
  for (let pass = 0; pass < 4; pass += 1) {
    const decoded = current
      .replace(/\\([0-9a-f]{1,6})(?:\r\n|[ \n\r\t\f])?/gi, (_match, hex) => (
        String.fromCodePoint(Number.parseInt(hex, 16) || 0)
      ))
      .replace(/\\([^\n\r\f0-9a-f])/gi, '$1');
    if (decoded === current) break;
    current = decoded;
  }
  return current;
}

function hasUnsafeCss(value) {
  // Agent-authored CSS is untrusted. Decode CSS escapes first, then remove the
  // complete declaration container instead of attempting a partial CSS parser.
  const css = canonicalizeCssEscapes(value).toLowerCase();
  return /url\s*\(|@import\b|@font-face\b|expression\s*\(|behavior\s*:|-moz-binding\b|(?:-webkit-)?image-set\s*\(|(?:image|cross-fade|element|src)\s*\(/.test(css);
}

function isSafeRasterDataImage(value) {
  const candidate = String(value || '').trim();
  const match = candidate.match(/^data:image\/(png|jpeg|gif|webp)((?:;[a-z0-9._-]+=[a-z0-9._-]+)*)(;base64)?,([\s\S]*)$/i);
  if (!match) return false;
  const payload = match[4];
  if (!payload) return false;
  return match[3]
    ? /^[a-z0-9+/=\s]+$/i.test(payload)
    : !/[<>"'`]/.test(payload) && /^(?:%[0-9a-f]{2}|[a-z0-9!$&()*+,\-./:;=?@_~\s])+$/i.test(payload);
}

function isAllowedResource(node, name, value) {
  const localName = String(node.localName || '').toLowerCase();
  const isSvg = node.namespaceURI === SVG_NAMESPACE || node.closest?.('svg');
  if ((name === 'href' || name === 'xlink:href') && isSvg) {
    return /^#[a-z_][\w:.-]*$/i.test(canonicalizeCssEscapes(value).trim());
  }
  if (name === 'src') return localName === 'img' && isSafeRasterDataImage(value);
  return false;
}

function normalizedLocalPaintServer(value) {
  const canonical = canonicalizeCssEscapes(value).trim();
  const match = canonical.match(/^url\(\s*(#[a-z_][\w:.-]*)\s*\)$/i);
  return match ? `url(${match[1]})` : null;
}

function normalizedSvgResourceAttribute(name, value) {
  const canonical = canonicalizeCssEscapes(value).trim();
  const lower = canonical.toLowerCase();
  if (name === 'href' || name === 'xlink:href') {
    return /^#[a-z_][\w:.-]*$/i.test(canonical) ? canonical : null;
  }
  if (!SVG_RESOURCE_PRESENTATION_ATTRIBUTES.has(name)) return canonical;
  if (/\burl\s*\(/i.test(canonical)) return normalizedLocalPaintServer(canonical);
  if (name === 'fill' || name === 'stroke') {
    if (/(?:javascript|vbscript|file|data|blob|https?):|(?:^|[\s"'(])\/\//i.test(lower)) return null;
    if (/(?:-webkit-)?image-set\s*\(|(?:image|cross-fade|element|src)\s*\(/i.test(lower)) return null;
    return canonical || null;
  }
  return /^(?:none|inherit|initial|unset)$/i.test(canonical) ? canonical : null;
}

function hasSafeCustomAttributeValue(value) {
  const canonical = canonicalizeCssEscapes(value).trim().toLowerCase();
  return !/(?:javascript|vbscript|file|data|blob|https?):|(?:^|[\s"'(])\/\//.test(canonical)
    && !hasUnsafeCss(canonical);
}

function isSvgElement(node) {
  return node.namespaceURI === SVG_NAMESPACE;
}

function allowedAttributeName(node, name) {
  const tag = String(node.localName || '').toLowerCase();
  if (name.startsWith('data-') || name.startsWith('aria-')) return true;
  if (isSvgElement(node)) {
    if (GLOBAL_ALLOWED_ATTRIBUTES.has(name) || SVG_GLOBAL_ALLOWED_ATTRIBUTES.has(name)) return true;
    if (SVG_TAG_ALLOWED_ATTRIBUTES[tag]?.has(name)) return true;
    return tag.startsWith('fe') && (
      SVG_GEOMETRY_ATTRIBUTES.has(name) || SVG_FILTER_ATTRIBUTES.has(name)
    );
  }
  return GLOBAL_ALLOWED_ATTRIBUTES.has(name) || Boolean(TAG_ALLOWED_ATTRIBUTES[tag]?.has(name));
}

export function isAllowedSanitizedAttribute(node, rawName, value) {
  const name = String(rawName || '').toLowerCase();
  if (!name || name.startsWith('on') || !allowedAttributeName(node, name)) return false;
  if (name.startsWith('data-') || name.startsWith('aria-')) {
    return hasSafeCustomAttributeValue(value);
  }
  if (name === 'style') return !hasUnsafeCss(value);
  if (name === 'src' || name === 'href' || name === 'xlink:href') {
    return isAllowedResource(node, name, value);
  }
  if (isSvgElement(node) && SVG_RESOURCE_PRESENTATION_ATTRIBUTES.has(name)) {
    return normalizedSvgResourceAttribute(name, value) !== null;
  }
  if (name === 'xmlns') return value === SVG_NAMESPACE;
  if (name === 'xmlns:xlink') return value === 'http://www.w3.org/1999/xlink';
  return true;
}

function unwrapNode(node) {
  node.replaceWith(...node.childNodes);
}

export function sanitizeSlideDocument(parsed) {
  let repaired = false;
  [...parsed.querySelectorAll('*')].forEach((node) => {
    const tag = String(node.localName || node.tagName || '').toLowerCase();
    const allowedTags = isSvgElement(node) ? SVG_ALLOWED_TAGS : HTML_ALLOWED_TAGS;
    if (DELETE_WITH_CONTENT_TAGS.has(tag)) {
      repaired = true;
      node.remove();
    } else if (!allowedTags.has(tag)) {
      repaired = true;
      unwrapNode(node);
    }
  });
  parsed.querySelectorAll('style').forEach((style) => {
    if (hasUnsafeCss(style.textContent)) {
      repaired = true;
      style.remove();
    }
  });
  parsed.querySelectorAll('*').forEach((node) => {
    for (const attribute of [...node.attributes]) {
      if (!isAllowedSanitizedAttribute(node, attribute.name, attribute.value)) {
        repaired = true;
        node.removeAttribute(attribute.name);
      } else if (isSvgElement(node)) {
        const name = attribute.name.toLowerCase();
        if (SVG_RESOURCE_PRESENTATION_ATTRIBUTES.has(name)
          || name === 'href'
          || name === 'xlink:href') {
          const normalized = normalizedSvgResourceAttribute(name, attribute.value);
          if (normalized !== attribute.value) {
            repaired = true;
            node.setAttribute(attribute.name, normalized);
          }
        }
      }
    }
  });
  parsed._pptxSecurityDiagnostics = repaired ? [{
    severity: 'repaired',
    code: 'active_content_removed',
    message: 'Unsafe active content and resource references were removed.',
    sourceId: 'slide-document',
    phase: 'security-repair',
  }] : [];
  return parsed;
}

export function sanitizeSlideMarkup(markup) {
  const parsed = new DOMParser().parseFromString(String(markup || ''), 'text/html');
  sanitizeSlideDocument(parsed);
  return `<!doctype html>${parsed.documentElement.outerHTML}`;
}
