import reactUmd from '../../../node_modules/react/umd/react.production.min.js?raw';
import reactDomUmd from '../../../node_modules/react-dom/umd/react-dom.production.min.js?raw';
import bitfunCanvasRuntimeBundle from 'virtual:bitfun-canvas-runtime-bundle';
import { buildCanvasRuntimeInstallerScript } from './runtime/canvasRuntimeInstaller';

interface ReactCanvasRuntimeOptions {
  title: string;
}

interface ExtractedCanvasScript {
  code: string;
  revision?: string;
}

export interface ReactCanvasHtmlResult {
  html?: string;
  runtime: 'react' | 'legacy' | 'empty';
  revision?: string;
}

const COMPONENT_SCRIPT_PATTERN =
  /<script\b[^>]*\bdata-revision=(["'])(.*?)\1[^>]*>([\s\S]*?)<\/script>/i;

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function sanitizeInlineScript(value: string): string {
  return value.replace(/<\/script/gi, '<\\/script');
}

export function extractCanvasComponentScript(html?: string): ExtractedCanvasScript | null {
  if (!html) return null;
  const match = COMPONENT_SCRIPT_PATTERN.exec(html);
  if (!match) return null;
  return {
    revision: match[2],
    code: match[3].trim(),
  };
}

export function buildReactCanvasHtml(
  compiledHtml: string | undefined,
  options: ReactCanvasRuntimeOptions,
): string | undefined {
  return buildReactCanvasHtmlResult(compiledHtml, options).html;
}

export function buildReactCanvasHtmlResult(
  compiledHtml: string | undefined,
  options: ReactCanvasRuntimeOptions,
): ReactCanvasHtmlResult {
  const componentScript = extractCanvasComponentScript(compiledHtml);
  if (!componentScript) {
    return {
      html: compiledHtml,
      runtime: compiledHtml ? 'legacy' : 'empty',
    };
  }

  const revision = componentScript.revision ?? '';

  return {
    runtime: 'react',
    revision: componentScript.revision,
    html: `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; font-src 'none'; frame-src 'none';">
  <meta name="bitfun-canvas-revision" content="${escapeHtml(revision)}">
  <title>${escapeHtml(options.title)}</title>
  <style>${bitfunCanvasRuntimeBundle.css}</style>
</head>
<body>
  <div id="bitfun-canvas-root">
    <main class="bf-canvas-boot">Canvas runtime is starting...</main>
  </div>
  <script>${sanitizeInlineScript(reactCanvasEarlyBridge(revision))}</script>
  <script>${sanitizeInlineScript(reactUmd)}</script>
  <script>window.__bitfunCanvasPost?.('bitfun-canvas-react-loaded', { hasReact: Boolean(window.React) });</script>
  <script>${sanitizeInlineScript(reactDomUmd)}</script>
  <script>window.__bitfunCanvasPost?.('bitfun-canvas-react-dom-loaded', { hasReactDOM: Boolean(window.ReactDOM), hasCreateRoot: Boolean(window.ReactDOM?.createRoot) });</script>
  <script>${sanitizeInlineScript(buildCanvasRuntimeInstallerScript(revision))}</script>
  <script>${sanitizeInlineScript(bitfunCanvasRuntimeBundle.js)}</script>
  <script>${sanitizeInlineScript(wrapUserCanvasScript(componentScript.code))}</script>
</body>
</html>`,
  };
}

function reactCanvasEarlyBridge(revision: string): string {
  return `
(function () {
  const sourceRevision = ${JSON.stringify(revision)};
  function errorPayload(error) {
    if (error && typeof error === 'object') {
      return {
        message: String(error.message || error),
        name: String(error.name || ''),
        stack: String(error.stack || '')
      };
    }
    return { message: String(error || 'Canvas runtime error') };
  }
  window.__bitfunCanvasPost = function (type, payload) {
    window.parent?.postMessage({ type, sourceRevisionSeen: sourceRevision, ...(payload || {}) }, '*');
  };
  window.addEventListener('error', event => {
    window.__bitfunCanvasPost('bitfun-canvas-early-error', {
      ...errorPayload(event.error || event.message),
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno
    });
  });
  window.addEventListener('unhandledrejection', event => {
    window.__bitfunCanvasPost('bitfun-canvas-early-error', errorPayload(event.reason || 'Canvas runtime promise rejection'));
  });
  window.__bitfunCanvasPost('bitfun-canvas-boot-started');
})();
`;
}

function wrapUserCanvasScript(componentCode: string): string {
  return `
(function () {
  try {
    window.BitfunCanvasRuntime.moduleStarted();
${componentCode}
  } catch (error) {
    window.BitfunCanvasRuntime.reportRuntimeError(error);
  }
})();
`;
}
