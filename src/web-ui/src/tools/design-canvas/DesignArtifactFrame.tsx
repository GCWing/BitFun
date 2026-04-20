/**
 * Sandbox iframe preview for a Design Artifact.
 *
 * Unlike GenerativeWidgetFrame (which renders one HTML fragment) this frame
 * mounts the assembled multi-file project: the entry HTML is rewritten so that
 * `<link href="styles/main.css">` and `<script src="./app.js">` references are
 * resolved against the in-memory file cache, then everything is served via a
 * single srcDoc blob with virtual base tag tricks.
 *
 * The frame also supports an element-picker mode: clicks bubble out as
 * `bitfun-design-artifact:select` so the panel can populate Inspector state
 * and feed Continue-with-Agent.
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';

export interface DesignArtifactFrameProps {
  artifactId: string;
  entry: string;
  files: Record<string, string>;
  /** Binary assets keyed by artifact-relative path, expressed as data URLs. */
  assets?: Record<string, string>;
  viewport: 'desktop' | 'tablet' | 'mobile' | string;
  pickerActive?: boolean;
  onSelectElement?: (selection: {
    domPath: string;
    tagName: string;
    textExcerpt: string;
    computedStyle?: Record<string, string>;
    rect?: { x: number; y: number; width: number; height: number };
  }) => void;
  /** Design tokens extracted from :root computed styles. */
  onTokens?: (tokens: Record<string, string>) => void;
  onOpenFile?: (path: string) => void;
  className?: string;
  /** When true, exposes the frame via a ref hook for screenshotting. */
  frameRef?: React.MutableRefObject<HTMLIFrameElement | null>;
}

const VIEWPORT_WIDTHS: Record<string, number | null> = {
  desktop: null,
  tablet: 768,
  mobile: 390,
};

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function normalizeRelativePath(raw: string): string {
  return raw.trim().replace(/^['"]|['"]$/g, '').replace(/^\.\//, '').replace(/^\//, '');
}

function dirname(path: string): string {
  const normalized = normalizeRelativePath(path);
  const idx = normalized.lastIndexOf('/');
  return idx >= 0 ? normalized.slice(0, idx) : '';
}

function joinRelative(baseFile: string, relativeImport: string): string {
  const rel = normalizeRelativePath(relativeImport);
  if (!rel) return rel;
  if (!rel.startsWith('.')) {
    return rel;
  }
  const baseDir = dirname(baseFile);
  const rawSegments = `${baseDir}/${rel}`.split('/');
  const resolved: string[] = [];
  for (const seg of rawSegments) {
    if (!seg || seg === '.') continue;
    if (seg === '..') {
      resolved.pop();
      continue;
    }
    resolved.push(seg);
  }
  return resolved.join('/');
}

function joinDocumentRelative(baseFile: string, rawReference: string): string {
  const value = rawReference.trim().replace(/^['"]|['"]$/g, '');
  if (!value) return '';
  const rootRelative = value.replace(/^\//, '');
  const seed = value.startsWith('/') ? rootRelative : `${dirname(baseFile)}/${rootRelative}`;
  const resolved: string[] = [];

  for (const seg of seed.split('/')) {
    if (!seg || seg === '.') continue;
    if (seg === '..') {
      resolved.pop();
      continue;
    }
    resolved.push(seg);
  }

  return resolved.join('/');
}

function inlineModuleGraph(
  entryPath: string,
  files: Record<string, string>,
  seen = new Set<string>()
): string | null {
  const normalizedEntry = normalizeRelativePath(entryPath);
  if (seen.has(normalizedEntry)) {
    return '';
  }
  const source = files[normalizedEntry];
  if (typeof source !== 'string') {
    return null;
  }
  seen.add(normalizedEntry);

  const importRegex =
    /^\s*import\s+(?:[\w*\s{},]*from\s+)?['"]([^'"]+)['"]\s*;?\s*$/gm;

  let inlinedDeps = '';
  let stripped = source.replace(importRegex, (_match, importPath) => {
    const resolvedImport = joinRelative(normalizedEntry, importPath);
    const child = inlineModuleGraph(resolvedImport, files, seen);
    if (typeof child === 'string') {
      inlinedDeps += `\n/* inlined from ${resolvedImport} */\n${child}\n`;
    }
    return '';
  });

  stripped = stripped.replace(/\bexport\s+\{[^}]+\};?\s*$/gm, '');
  stripped = stripped.replace(/\bexport\s+(?=(function|const|let|var|class)\b)/g, '');

  return `${inlinedDeps}\n${stripped}`.trim();
}

/**
 * Very small in-place resolver: replaces `href="./x"` / `src="./x"` style
 * references inside the entry HTML with inline contents or data: URLs, so the
 * sandboxed srcDoc can render the multi-file artifact without a dev server.
 *
 * NOTE: This is intentionally a minimal resolver. It handles CSS and JS
 * references; images must be provided as data: or remote URLs. Extending to
 * blob-URL asset serving is an incremental enhancement.
 */
function assembleDocument(
  entryPath: string,
  entryHtml: string,
  files: Record<string, string>,
  assets: Record<string, string>
): string {
  const textFiles = files;
  const binaryAssets = assets;

  const resolveText = (raw: string): string | null => {
    if (!raw) return null;
    const trimmed = normalizeRelativePath(raw);
    if (!trimmed || /^[a-z]+:/i.test(trimmed) || trimmed.startsWith('//') || trimmed.startsWith('data:')) {
      return null;
    }
    if (textFiles[trimmed] !== undefined) return trimmed;
    const relativeToEntry = joinDocumentRelative(entryPath, raw);
    return textFiles[relativeToEntry] !== undefined ? relativeToEntry : null;
  };

  const resolveAsset = (raw: string): string | null => {
    if (!raw) return null;
    const trimmed = normalizeRelativePath(raw);
    if (!trimmed || /^[a-z]+:/i.test(trimmed) || trimmed.startsWith('//') || trimmed.startsWith('data:')) {
      return null;
    }
    if (binaryAssets[trimmed]) return binaryAssets[trimmed];
    const relativeToEntry = joinDocumentRelative(entryPath, raw);
    return binaryAssets[relativeToEntry] ?? null;
  };

  let out = entryHtml;

  out = out.replace(
    /<link\s+[^>]*?href=(["'])([^"']+)\1[^>]*?>/gi,
    (match, _quote, href) => {
      const target = resolveText(href);
      if (target && /\.(css)$/i.test(target)) {
        return `<style data-design-artifact-file="${escapeHtml(target)}">${textFiles[target]}</style>`;
      }
      return match;
    }
  );

  out = out.replace(
    /<script\s+[^>]*?src=(["'])([^"']+)\1[^>]*?><\/script>/gi,
    (match, _quote, src) => {
      const target = resolveText(src);
      if (target && /\.(js|mjs)$/i.test(target)) {
        const isModule = /\btype=(["'])module\1/i.test(match);
        const bundled = isModule ? inlineModuleGraph(target, textFiles) : textFiles[target];
        // After inlining the full module graph we intentionally execute it as a
        // classic script, not as `type="module"`, so interactive handlers that
        // expose functions on `window` behave like a normal page preview.
        return `<script data-design-artifact-file="${escapeHtml(target)}">${bundled ?? textFiles[target]}</script>`;
      }
      return match;
    }
  );

  // Rewrite img src / source srcset / generic href references that point at
  // binary assets we have encoded as data URLs.
  out = out.replace(
    /(<(?:img|source|image|video|audio|link)[^>]*?\s(?:src|href|xlink:href)=)(["'])([^"']+)\2/gi,
    (match, prefix, quote, value) => {
      const dataUrl = resolveAsset(value);
      if (!dataUrl) return match;
      return `${prefix}${quote}${dataUrl}${quote}`;
    }
  );

  // Also inline asset references found inside inline style url(...).
  out = out.replace(/url\((['"]?)([^)'"]+)\1\)/gi, (match, quote, value) => {
    const dataUrl = resolveAsset(value);
    if (!dataUrl) return match;
    return `url(${quote}${dataUrl}${quote})`;
  });

  return out;
}

function collectReferencedAssetPaths(entryPath: string, entryHtml: string): string[] {
  const referenced = new Set<string>();
  const collect = (regex: RegExp) => {
    for (const match of entryHtml.matchAll(regex)) {
      const raw = String(match[1] || '').trim();
      if (!raw || /^[a-z]+:/i.test(raw) || raw.startsWith('//') || raw.startsWith('data:')) {
        continue;
      }
      referenced.add(joinDocumentRelative(entryPath, raw));
    }
  };

  collect(/<link\s+[^>]*?href=["']([^"']+)["']/gi);
  collect(/<script\s+[^>]*?src=["']([^"']+)["']/gi);
  collect(/<(?:img|source|image|video|audio|link)[^>]*?\s(?:src|href|xlink:href)=["']([^"']+)["']/gi);
  collect(/url\((?:['"])?([^)'"]+)(?:['"])?\)/gi);

  return Array.from(referenced).sort();
}

const PICKER_INSTRUMENT = `
  (function () {
    var pickerActive = false;
    var INSPECT_STYLE_PROPS = [
      'display', 'position', 'width', 'height', 'margin', 'padding',
      'color', 'background-color', 'background', 'border', 'border-radius',
      'font-family', 'font-size', 'font-weight', 'line-height', 'letter-spacing',
      'box-shadow', 'opacity', 'z-index', 'flex', 'gap', 'align-items', 'justify-content'
    ];
    function domPath(el) {
      if (!el || el.nodeType !== 1) return '';
      var stack = [];
      while (el && el.nodeType === 1 && stack.length < 16) {
        var part = el.tagName.toLowerCase();
        if (el.id) { part += '#' + el.id; stack.unshift(part); break; }
        var classes = (el.className || '').toString().trim().split(/\\s+/).filter(Boolean).slice(0, 2);
        if (classes.length) part += '.' + classes.join('.');
        var parent = el.parentNode;
        if (parent && parent.nodeType === 1) {
          var siblings = Array.prototype.slice.call(parent.children).filter(function (s) { return s.tagName === el.tagName; });
          if (siblings.length > 1) part += ':nth-of-type(' + (siblings.indexOf(el) + 1) + ')';
        }
        stack.unshift(part);
        el = parent;
      }
      return stack.join(' > ');
    }
    function computedStyleMap(el) {
      try {
        var cs = window.getComputedStyle(el);
        var out = {};
        for (var i = 0; i < INSPECT_STYLE_PROPS.length; i++) {
          var name = INSPECT_STYLE_PROPS[i];
          var val = cs.getPropertyValue(name);
          if (val) out[name] = val.trim();
        }
        return out;
      } catch (err) { return {}; }
    }
    function extractTokens() {
      try {
        var cs = window.getComputedStyle(document.documentElement);
        var out = {};
        for (var i = 0; i < cs.length; i++) {
          var name = cs.item(i);
          if (name && name.indexOf('--') === 0) {
            out[name] = cs.getPropertyValue(name).trim();
          }
        }
        return out;
      } catch (err) { return {}; }
    }
    function emitSelection(el, ev) {
      if (!el || el.nodeType !== 1) return;
      ev && ev.preventDefault();
      ev && ev.stopPropagation();
      var text = (el.textContent || '').trim().slice(0, 140);
      var rect = el.getBoundingClientRect();
      parent.postMessage({
        source: 'bitfun-design-artifact',
        type: 'bitfun-design-artifact:select',
        domPath: domPath(el),
        tagName: el.tagName,
        textExcerpt: text,
        computedStyle: computedStyleMap(el),
        rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      }, '*');
    }
    document.addEventListener('mouseover', function (e) {
      if (!pickerActive) return;
      var t = e.target;
      if (t && t.nodeType === 1) {
        t.setAttribute('data-design-picker-hover', 'true');
      }
    }, true);
    document.addEventListener('mouseout', function (e) {
      var t = e.target;
      if (t && t.nodeType === 1) {
        t.removeAttribute('data-design-picker-hover');
      }
    }, true);
    document.addEventListener('click', function (e) {
      if (!pickerActive) return;
      emitSelection(e.target, e);
    }, true);
    window.addEventListener('message', function (ev) {
      var data = ev.data;
      if (!data || data.type !== 'bitfun-design-artifact:picker') return;
      pickerActive = Boolean(data.active);
      document.documentElement.setAttribute('data-design-picker-on', pickerActive ? 'true' : 'false');
    });
    function sendTokens() {
      parent.postMessage({
        source: 'bitfun-design-artifact',
        type: 'bitfun-design-artifact:tokens',
        tokens: extractTokens(),
      }, '*');
    }
    window.addEventListener('load', function () { setTimeout(sendTokens, 80); });
    window.addEventListener('message', function (ev) {
      var data = ev.data;
      if (!data) return;
      if (data.type === 'bitfun-design-artifact:request-tokens') sendTokens();
    });
    parent.postMessage({ source: 'bitfun-design-artifact', type: 'bitfun-design-artifact:ready' }, '*');
  })();
`;

const PICKER_STYLE = `
  html[data-design-picker-on="true"] *:hover {
    outline: 2px solid rgba(96, 165, 250, 0.65) !important;
    outline-offset: 1px !important;
    cursor: crosshair !important;
  }
`;

export const DesignArtifactFrame: React.FC<DesignArtifactFrameProps> = ({
  artifactId,
  entry,
  files,
  assets,
  viewport,
  pickerActive = false,
  onSelectElement,
  onTokens,
  className = '',
  frameRef,
}) => {
  const internalRef = useRef<HTMLIFrameElement>(null);
  const iframeRef = frameRef ?? internalRef;
  const [isReady, setIsReady] = useState(false);
  const entryHtml = files[entry];
  const relevantAssetPaths = useMemo(() => {
    if (typeof entryHtml !== 'string' || !/\.(html?)$/i.test(entry)) {
      return [];
    }
    return collectReferencedAssetPaths(entry, entryHtml);
  }, [entry, entryHtml]);
  const textDependencySignature = useMemo(() => {
    if (typeof entryHtml !== 'string' || !/\.(html?)$/i.test(entry)) {
      return '';
    }
    return [
      entry,
      entryHtml,
      ...relevantAssetPaths
        .filter((path) => Object.prototype.hasOwnProperty.call(files, path))
        .map((path) => `${path}:${files[path] ?? ''}`),
    ].join('\u0001');
  }, [entry, entryHtml, files, relevantAssetPaths]);
  const assetDependencySignature = useMemo(
    () =>
      relevantAssetPaths
        .filter((path) => Object.prototype.hasOwnProperty.call(assets ?? {}, path))
        .map((path) => `${path}:${(assets ?? {})[path] ?? ''}`)
        .join('\u0001'),
    [assets, relevantAssetPaths]
  );

  const doc = useMemo(() => {
    if (typeof entryHtml !== 'string' || !/\.(html?)$/i.test(entry)) {
      return '<!doctype html><html><body><main style="font-family:Inter,system-ui,sans-serif;padding:24px;color:#111">Waiting for HTML entry...</main></body></html>';
    }
    const assembled = assembleDocument(entry, entryHtml, files, assets ?? {});
    const inject = `
      <style data-design-picker-style>${PICKER_STYLE}</style>
      <script data-design-picker-script>${PICKER_INSTRUMENT}<\/script>
    `;
    if (/<\/head>/i.test(assembled)) {
      return assembled.replace(/<\/head>/i, `${inject}</head>`);
    }
    return `<!doctype html><html><head><meta charset="utf-8">${inject}</head><body>${assembled}</body></html>`;
    }, [assetDependencySignature, entry, entryHtml, textDependencySignature]);

  useEffect(() => {
    const handler = (event: MessageEvent) => {
      const data = event.data as any;
      if (!data || data.source !== 'bitfun-design-artifact') return;
      if (event.source !== iframeRef.current?.contentWindow) return;
      if (data.type === 'bitfun-design-artifact:ready') {
        setIsReady(true);
        return;
      }
      if (data.type === 'bitfun-design-artifact:select' && onSelectElement) {
        onSelectElement({
          domPath: String(data.domPath || ''),
          tagName: String(data.tagName || ''),
          textExcerpt: String(data.textExcerpt || ''),
          computedStyle: (data.computedStyle || undefined) as Record<string, string> | undefined,
          rect: data.rect,
        });
        return;
      }
      if (data.type === 'bitfun-design-artifact:tokens' && onTokens) {
        onTokens(data.tokens || {});
      }
    };
    window.addEventListener('message', handler);
    return () => window.removeEventListener('message', handler);
  }, [onSelectElement, onTokens, iframeRef]);

  useEffect(() => {
    if (!isReady || !iframeRef.current?.contentWindow) return;
    iframeRef.current.contentWindow.postMessage(
      { type: 'bitfun-design-artifact:picker', active: pickerActive },
      '*'
    );
  }, [pickerActive, isReady]);

  const width = VIEWPORT_WIDTHS[viewport] ?? null;

  return (
    <div
      className={`bitfun-design-artifact-frame ${className}`.trim()}
      style={{
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        width: '100%',
        height: '100%',
        overflow: 'auto',
        background: 'var(--color-bg-scene, var(--color-bg-primary, #111))',
        padding: width ? '24px' : 0,
      }}
      data-artifact-id={artifactId}
    >
      <iframe
        ref={iframeRef}
        title={`design-artifact-${artifactId}`}
        srcDoc={doc}
        sandbox="allow-scripts allow-forms allow-modals allow-popups allow-same-origin"
        onLoad={() => setIsReady(true)}
        style={{
          width: width ? `${width}px` : '100%',
          maxWidth: '100%',
          height: width ? '720px' : '100%',
          minHeight: '100%',
          border: width ? '1px solid var(--border-base, rgba(255,255,255,0.16))' : 'none',
          borderRadius: width ? '8px' : 0,
          background: '#fff',
          boxShadow: width ? '0 8px 24px rgba(0,0,0,0.25)' : 'none',
        }}
      />
    </div>
  );
};

export default DesignArtifactFrame;
