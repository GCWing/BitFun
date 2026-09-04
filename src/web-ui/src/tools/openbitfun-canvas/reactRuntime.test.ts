import { JSDOM } from 'jsdom';
import { describe, expect, it } from 'vitest';

import {
  buildReactCanvasHtml,
  buildReactCanvasHtmlResult,
  extractCanvasComponentScript,
} from './reactRuntime';

const compiledHtml = `<!DOCTYPE html>
<html>
<body>
  <script>legacy runtime</script>
  <script type="module" data-revision="rev_test">
const { Stack } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
function Canvas() { return h(Stack, null, 'Hello'); }
window.OpenBitFunCanvasRuntime.mount(Canvas);
  </script>
</body>
</html>`;

async function runCanvasHtml(html: string, state: Record<string, unknown> = {}) {
  const dom = new JSDOM(html, {
    pretendToBeVisual: true,
    runScripts: 'outside-only',
    url: 'https://openbitfun-canvas.local/',
  });
  const messages: unknown[] = [];

  Object.defineProperty(dom.window, 'parent', {
    configurable: true,
    value: {
      postMessage(message: unknown) {
        messages.push(message);
        const record = message as { type?: string; requestId?: string };
        if (record?.type === 'openbitfun-canvas-load-state') {
          dom.window.setTimeout(() => {
            dom.window.dispatchEvent(new dom.window.MessageEvent('message', {
              data: {
                type: 'openbitfun-canvas-load-state-result',
                requestId: record.requestId,
                state: {
                  canvasId: 'canvas_test',
                  sourceRevisionSeen: 'legacy_revision',
                  values: state,
                  valueVersions: {},
                  updatedAt: 1,
                  schemaVersion: 1,
                },
              },
            }));
          }, 0);
        }
      },
    },
  });
  Object.defineProperty(dom.window, 'process', {
    configurable: true,
    value: { env: { NODE_ENV: 'test' } },
  });
  dom.window.requestAnimationFrame = callback =>
    dom.window.setTimeout(() => callback(Date.now()), 0);
  dom.window.cancelAnimationFrame = timer => dom.window.clearTimeout(timer);

  const scripts = Array.from(dom.window.document.querySelectorAll('script'))
    .map(script => script.textContent ?? '');

  try {
    for (const script of scripts) {
      dom.window.eval(script);
    }
    await new Promise<void>(resolve => {
      const deadline = Date.now() + 500;
      const poll = () => {
        const settled = messages.some(message => {
          const type = (message as { type?: string })?.type;
          return type === 'openbitfun-canvas-ready' || type === 'openbitfun-canvas-runtime-error';
        });
        if (settled || Date.now() >= deadline) {
          resolve();
          return;
        }
        dom.window.setTimeout(poll, 10);
      };
      poll();
    });
    return { dom, messages };
  } catch (error) {
    dom.window.close();
    throw error;
  }
}

describe('React Canvas runtime bridge', () => {
  it('extracts the compiled Canvas component script from legacy HTML', () => {
    const script = extractCanvasComponentScript(compiledHtml);

    expect(script).toEqual({
      revision: 'rev_test',
      code: [
        'const { Stack } = window.OpenBitFunCanvasSDK;',
        'const { h } = window.OpenBitFunCanvasRuntime;',
        "function Canvas() { return h(Stack, null, 'Hello'); }",
        'window.OpenBitFunCanvasRuntime.mount(Canvas);',
      ].join('\n'),
    });
  });

  it('wraps compiled Canvas JS in the React runtime shell', () => {
    const result = buildReactCanvasHtmlResult(compiledHtml, { title: 'Canvas <Test>' });
    const html = result.html;

    expect(result.runtime).toBe('react');
    expect(result.revision).toBe('rev_test');
    expect(html).toContain('<title>Canvas &lt;Test&gt;</title>');
    expect(html).toContain('window.OpenBitFunCanvasSDK');
    expect(html).toContain('window.OpenBitFunCanvasRuntime');
    expect(html).toContain('window.OpenBitFunCanvasSDKAdapters');
    expect(html).toContain('ReactDOM.createRoot');
    expect(html).toContain('<meta name="openbitfun-canvas-revision" content="rev_test">');
    expect(html).toContain("function Canvas() { return h(Stack, null, 'Hello'); }");
    expect(html).not.toContain('process.env.NODE_ENV');
    expect(html).not.toContain('jsxDEV');
    expect(html).not.toContain('jsxRuntime');
    expect(html).not.toContain('legacy runtime');
    expect(html).toContain('//# sourceURL=openbitfun-canvas-rev_test.js');
  });

  it('keeps a compiled payload on its matching legacy runtime', () => {
    const result = buildReactCanvasHtmlResult(compiledHtml, {
      title: 'Legacy Canvas',
      runtimeVersion: '0.1.0',
      sdkVersion: '0.2.0',
    });

    expect(result).toEqual({
      html: compiledHtml,
      runtime: 'legacy',
      revision: 'rev_test',
      compatibilityFallback: true,
    });
  });

  it('uses a light centered standalone shell by default', () => {
    const html = buildReactCanvasHtml(compiledHtml, { title: 'Standalone' });

    expect(html).toContain('color-scheme:light');
    expect(html).toContain('--openbitfun-canvas-bg: Canvas');
    expect(html).toContain('.openbitfun-canvas-stack{max-width:min(100%,980px);margin-inline:auto}');
  });

  it('emits syntactically valid inline scripts', () => {
    const html = buildReactCanvasHtml(compiledHtml, { title: 'Syntax' });
    const scripts = Array.from(html?.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/gi) ?? [])
      .map(match => match[1]);

    expect(scripts.length).toBeGreaterThan(0);
    scripts.forEach((script, index) => {
      try {
        new Function(script);
      } catch (error) {
        const numbered = script
          .split('\n')
          .map((line, lineIndex) => `${String(lineIndex + 1).padStart(4, ' ')} ${line}`)
          .join('\n');
        throw new Error(
          `Script ${index} is invalid: ${error instanceof Error ? `${error.message}\n${error.stack}` : String(error)}\n${numbered}`,
        );
      }
    });
  });

  it('bridges host appearance variables into the iframe runtime', () => {
    const html = buildReactCanvasHtml(compiledHtml, { title: 'Appearance' });

    expect(html).toContain('nextAppearance.vars');
    expect(html).toContain("rootStyle.setProperty(name, value.trim())");
    expect(html).toContain('--openbitfun-color-surface-canvas');
  });

  it('exposes semantic appearance tokens through useHostAppearance().tokens', async () => {
    const html = buildReactCanvasHtml(`<!DOCTYPE html>
<script type="module" data-revision="rev_tokens">
const { useHostAppearance } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
function Canvas() {
  const { tokens } = useHostAppearance();
  return h('svg', null,
    h('rect', { 'data-testid': 'node', width: 20, height: 20, fill: tokens.bg.elevated }),
    h('text', { 'data-testid': 'label', fill: tokens.text.primary }, 'Node')
  );
}
window.OpenBitFunCanvasRuntime.mount(Canvas);
</script>`, { title: 'Appearance tokens' });

    const { dom, messages } = await runCanvasHtml(html ?? '');

    try {
      expect(messages).toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-ready' }));
      expect(messages).not.toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-runtime-error' }));
      expect(dom.window.document.querySelector('[data-testid="node"]')?.getAttribute('fill')).toBe(
        'var(--openbitfun-canvas-panel)',
      );
      expect(dom.window.document.querySelector('[data-testid="label"]')?.getAttribute('fill')).toBe(
        'var(--openbitfun-canvas-fg)',
      );
    } finally {
      dom.window.close();
    }
  });

  it('falls back to the original HTML when no compiled component script exists', () => {
    const html = '<html><body>No canvas component</body></html>';
    const result = buildReactCanvasHtmlResult(html, { title: 'Fallback' });

    expect(result).toEqual({ html, runtime: 'legacy' });
    expect(buildReactCanvasHtml(html, { title: 'Fallback' })).toBe(html);
  });

  it('wraps chart canvases in the React runtime shell', () => {
    const html = `<!DOCTYPE html>
<script>legacy runtime</script>
<script type="module" data-revision="rev_chart">
const { BarChart } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
function Canvas() { return h(BarChart, { data: [1, 2] }); }
window.OpenBitFunCanvasRuntime.mount(Canvas);
    </script>`;
    const wrapped = buildReactCanvasHtml(html, { title: 'Chart' });

    expect(wrapped).toContain('window.OpenBitFunCanvasSDKAdapters');
    expect(wrapped).toContain('BarChart:');
    expect(wrapped).not.toContain('function BarChart(props = {})');
    expect(wrapped).toContain('<meta name="openbitfun-canvas-revision" content="rev_chart">');
    expect(wrapped).toContain('ReactDOM.createRoot');
    expect(wrapped).not.toContain('legacy runtime');
  });

  it('supports string diff payloads used by PR review canvases', () => {
    const html = `<!DOCTYPE html>
<script>legacy runtime</script>
<script type="module" data-revision="rev_diff">
const { DiffStats, DiffView } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
const diffLines = '+added\\n-removed\\n unchanged';
function Canvas() {
  return h('div', null, h(DiffStats, { additions: 1, deletions: -1 }), h(DiffView, { lines: diffLines }));
}
window.OpenBitFunCanvasRuntime.mount(Canvas);
    </script>`;
    const wrapped = buildReactCanvasHtml(html, { title: 'Diff' });

    expect(wrapped).toContain('window.OpenBitFunCanvasSDKAdapters');
    expect(wrapped).toContain('normalizeDiffLines:');
    expect(wrapped).not.toContain('function normalizeDiffLines(lines)');
    expect(wrapped).not.toContain('legacy runtime');
  });

  it('smoke-renders bundled SDK components in the iframe shell', async () => {
    const html = buildReactCanvasHtml(`<!DOCTYPE html>
<script type="module" data-revision="rev_smoke">
const { Stack, Card, CardHeader, CardBody, BarChart, CollapsibleSection, DependencyGraph, Empty, FlowDiagram, Input, Tabs, Text } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
function Canvas() {
  return h(Stack, { gap: 8 },
    h(Card, null,
      h(CardHeader, { trailing: 'ready' }, 'Runtime smoke'),
      h(CardBody, null,
        h(CollapsibleSection, { title: 'Chart', count: 1, defaultOpen: true },
          h(BarChart, { title: 'Builds', data: [3, 5], categories: ['A', 'B'] })
        ),
        h(DependencyGraph, {
          title: 'Graph',
          nodes: [{ id: 'runtime', label: 'Runtime' }, { id: 'sdk', label: 'SDK' }],
          edges: [{ from: 'runtime', to: 'sdk' }]
        }),
        h(FlowDiagram, { steps: ['Compile', { title: 'Render', description: 'iframe' }] }),
        h(Tabs, { items: [{ key: 'one', label: 'One', children: h(Text, null, 'Tab body') }] }),
        h(Input, { value: '', placeholder: 'Note', label: 'Note' }),
        h(Empty, { description: 'No gaps' })
      )
    )
  );
}
window.OpenBitFunCanvasRuntime.mount(Canvas);
</script>`, { title: 'Smoke' });

    expect(html).toBeTruthy();
    const { dom, messages } = await runCanvasHtml(html ?? '');

    try {
      expect(messages).toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-module-started' }));
      expect(messages).toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-ready' }));
      expect(messages).not.toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-runtime-error' }));
      expect(dom.window.document.body.textContent).toContain('Runtime smoke');
      expect(dom.window.document.querySelector('.openbitfun-chart')).toBeTruthy();
      expect(dom.window.document.querySelector('.openbitfun-collapsible-section')).toBeTruthy();
      expect(dom.window.document.querySelector('.openbitfun-diagram')).toBeTruthy();
      expect(dom.window.document.querySelector('.openbitfun-canvas-adapter-tabs')).toBeTruthy();
      expect(dom.window.document.querySelector('[data-openbitfun-component="tab-group"]')).toBeTruthy();
      expect(dom.window.document.querySelector('[data-openbitfun-component="input"]')).toBeTruthy();
      expect(dom.window.document.querySelector('[data-openbitfun-component="empty"]')).toBeTruthy();
    } finally {
      dom.window.close();
    }
  });

  it('hydrates state before first render and ignores incompatible persisted shapes', async () => {
    const html = buildReactCanvasHtml(`<!DOCTYPE html>
<script type="module" data-revision="rev_state">
const { Stack, Table, Tabs, Text, useCanvasState, useEffect } = window.OpenBitFunCanvasSDK;
const { h } = window.OpenBitFunCanvasRuntime;
function Canvas() {
  const [rows, setRows] = useCanvasState('rows', [{ label: 'Current' }]);
  useEffect(() => {
    setRows(current => [...current, { label: 'Added' }]);
  }, []);
  return h(Stack, null,
    h(Text, { 'data-testid': 'row-count' }, String(rows.length)),
    h(Tabs, { items: { invalid: true } }),
    h(Table, { headers: ['Label'], rows: { invalid: true } })
  );
}
window.OpenBitFunCanvasRuntime.mount(Canvas);
</script>`, { title: 'State compatibility' });

    const { dom, messages } = await runCanvasHtml(html ?? '', { rows: 'stale value' });
    try {
      expect(messages).toContainEqual(expect.objectContaining({
        type: 'openbitfun-canvas-state-warning',
        key: 'rows',
      }));
      expect(messages).toContainEqual(expect.objectContaining({
        type: 'openbitfun-canvas-prop-warning',
        component: 'Tabs',
        prop: 'items',
      }));
      expect(messages).toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-ready' }));
      expect(messages).not.toContainEqual(expect.objectContaining({ type: 'openbitfun-canvas-runtime-error' }));
      expect(dom.window.document.querySelector('[data-testid="row-count"]')?.textContent).toBe('2');
      expect(messages).toContainEqual(expect.objectContaining({
        type: 'openbitfun-canvas-save-state',
        values: { rows: [{ label: 'Current' }, { label: 'Added' }] },
      }));
    } finally {
      dom.window.close();
    }
  });
});
