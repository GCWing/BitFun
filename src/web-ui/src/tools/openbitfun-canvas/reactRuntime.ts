import reactUmd from '../../../node_modules/react/umd/react.production.min.js?raw';
import reactDomUmd from '../../../node_modules/react-dom/umd/react-dom.production.min.js?raw';
import openbitfunCanvasRuntimeBundle from 'virtual:openbitfun-canvas-runtime-bundle';
import { buildCanvasRuntimeInstallerScript } from './runtime/canvasRuntimeInstaller';
import { CANVAS_RUNTIME_VERSION } from './runtime/sdk/contract.generated';

interface ReactCanvasRuntimeOptions {
  title: string;
  runtimeVersion?: string;
  sdkVersion?: string;
}

interface ExtractedCanvasScript {
  code: string;
  revision?: string;
}

export interface ReactCanvasHtmlResult {
  html?: string;
  runtime: 'react' | 'legacy' | 'empty';
  revision?: string;
  compatibilityFallback?: boolean;
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

  if (options.runtimeVersion && options.runtimeVersion !== CANVAS_RUNTIME_VERSION) {
    return {
      html: compiledHtml,
      runtime: 'legacy',
      revision: componentScript.revision,
      compatibilityFallback: true,
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
  <meta name="openbitfun-canvas-revision" content="${escapeHtml(revision)}">
  <title>${escapeHtml(options.title)}</title>
  <style>${openbitfunCanvasRuntimeBundle.css}</style>
</head>
<body>
  <div id="openbitfun-canvas-root">
    <main class="openbitfun-canvas-boot">Canvas runtime is starting...</main>
  </div>
  <script>${sanitizeInlineScript(reactCanvasEarlyBridge(revision))}</script>
  <script>${sanitizeInlineScript(reactUmd)}</script>
  <script>window.__openbitfunCanvasPost?.('openbitfun-canvas-react-loaded', { hasReact: Boolean(window.React) });</script>
  <script>${sanitizeInlineScript(reactDomUmd)}</script>
  <script>window.__openbitfunCanvasPost?.('openbitfun-canvas-react-dom-loaded', { hasReactDOM: Boolean(window.ReactDOM), hasCreateRoot: Boolean(window.ReactDOM?.createRoot) });</script>
  <script>${sanitizeInlineScript(buildCanvasRuntimeInstallerScript(revision))}</script>
  <script>${sanitizeInlineScript(openbitfunCanvasRuntimeBundle.js)}</script>
  <script>${sanitizeInlineScript(wrapUserCanvasScript(componentScript.code, revision))}</script>
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
  window.__openbitfunCanvasPost = function (type, payload) {
    window.parent?.postMessage({ type, sourceRevisionSeen: sourceRevision, ...(payload || {}) }, '*');
  };
  window.addEventListener('error', event => {
    window.__openbitfunCanvasPost('openbitfun-canvas-early-error', {
      ...errorPayload(event.error || event.message),
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno
    });
  });
  window.addEventListener('unhandledrejection', event => {
    window.__openbitfunCanvasPost('openbitfun-canvas-early-error', errorPayload(event.reason || 'Canvas runtime promise rejection'));
  });
  window.__openbitfunCanvasPost('openbitfun-canvas-boot-started');
})();
`;
}

function wrapUserCanvasScript(componentCode: string, revision: string): string {
  return `
(function () {
  try {
    window.OpenBitFunCanvasRuntime.moduleStarted();
${componentCode}
  } catch (error) {
    window.OpenBitFunCanvasRuntime.reportRuntimeError(error);
  }
})();
//# sourceURL=openbitfun-canvas-${revision || 'unknown'}.js
`;
}
