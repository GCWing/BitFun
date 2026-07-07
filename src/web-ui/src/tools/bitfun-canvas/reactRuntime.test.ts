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
const { Stack } = window.BitfunCanvasSDK;
const { h } = window.BitfunCanvasRuntime;
function Canvas() { return h(Stack, null, 'Hello'); }
window.BitfunCanvasRuntime.mount(Canvas);
  </script>
</body>
</html>`;

async function runCanvasHtml(html: string) {
  const dom = new JSDOM(html, {
    pretendToBeVisual: true,
    runScripts: 'outside-only',
    url: 'https://bitfun-canvas.local/',
  });
  const messages: unknown[] = [];

  Object.defineProperty(dom.window, 'parent', {
    configurable: true,
    value: {
      postMessage(message: unknown) {
        messages.push(message);
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
      dom.window.setTimeout(resolve, 25);
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
        'const { Stack } = window.BitfunCanvasSDK;',
        'const { h } = window.BitfunCanvasRuntime;',
        "function Canvas() { return h(Stack, null, 'Hello'); }",
        'window.BitfunCanvasRuntime.mount(Canvas);',
      ].join('\n'),
    });
  });

  it('wraps compiled Canvas JS in the React runtime shell', () => {
    const result = buildReactCanvasHtmlResult(compiledHtml, { title: 'Canvas <Test>' });
    const html = result.html;

    expect(result.runtime).toBe('react');
    expect(result.revision).toBe('rev_test');
    expect(html).toContain('<title>Canvas &lt;Test&gt;</title>');
    expect(html).toContain('window.BitfunCanvasSDK');
    expect(html).toContain('window.BitfunCanvasRuntime');
    expect(html).toContain('window.BitfunCanvasSDKAdapters');
    expect(html).toContain('ReactDOM.createRoot');
    expect(html).toContain('<meta name="bitfun-canvas-revision" content="rev_test">');
    expect(html).toContain("function Canvas() { return h(Stack, null, 'Hello'); }");
    expect(html).not.toContain('process.env.NODE_ENV');
    expect(html).not.toContain('jsxDEV');
    expect(html).not.toContain('jsxRuntime');
    expect(html).not.toContain('legacy runtime');
  });

  it('uses a light centered standalone shell by default', () => {
    const html = buildReactCanvasHtml(compiledHtml, { title: 'Standalone' });

    expect(html).toContain('color-scheme:light');
    expect(html).toContain('--bitfun-canvas-bg: Canvas');
    expect(html).toContain('.bf-canvas-stack{max-width:min(100%,980px);margin-inline:auto}');
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

  it('bridges host theme variables into the iframe runtime', () => {
    const html = buildReactCanvasHtml(compiledHtml, { title: 'Theme' });

    expect(html).toContain('nextTheme.vars');
    expect(html).toContain("rootStyle.setProperty(name, value.trim())");
    expect(html).toContain('--color-bg-primary');
  });

  it('exposes semantic theme tokens through useHostTheme().tokens', async () => {
    const html = buildReactCanvasHtml(`<!DOCTYPE html>
<script type="module" data-revision="rev_tokens">
const { useHostTheme } = window.BitfunCanvasSDK;
const { h } = window.BitfunCanvasRuntime;
function Canvas() {
  const { tokens } = useHostTheme();
  return h('svg', null,
    h('rect', { 'data-testid': 'node', width: 20, height: 20, fill: tokens.bg.elevated }),
    h('text', { 'data-testid': 'label', fill: tokens.text.primary }, 'Node')
  );
}
window.BitfunCanvasRuntime.mount(Canvas);
</script>`, { title: 'Theme tokens' });

    const { dom, messages } = await runCanvasHtml(html ?? '');

    try {
      expect(messages).toContainEqual(expect.objectContaining({ type: 'bitfun-canvas-ready' }));
      expect(messages).not.toContainEqual(expect.objectContaining({ type: 'bitfun-canvas-runtime-error' }));
      expect(dom.window.document.querySelector('[data-testid="node"]')?.getAttribute('fill')).toBe(
        'var(--bitfun-canvas-panel)',
      );
      expect(dom.window.document.querySelector('[data-testid="label"]')?.getAttribute('fill')).toBe(
        'var(--bitfun-canvas-fg)',
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
const { BarChart } = window.BitfunCanvasSDK;
const { h } = window.BitfunCanvasRuntime;
function Canvas() { return h(BarChart, { data: [1, 2] }); }
window.BitfunCanvasRuntime.mount(Canvas);
    </script>`;
    const wrapped = buildReactCanvasHtml(html, { title: 'Chart' });

    expect(wrapped).toContain('window.BitfunCanvasSDKAdapters');
    expect(wrapped).toContain('BarChart:');
    expect(wrapped).not.toContain('function BarChart(props = {})');
    expect(wrapped).toContain('<meta name="bitfun-canvas-revision" content="rev_chart">');
    expect(wrapped).toContain('ReactDOM.createRoot');
    expect(wrapped).not.toContain('legacy runtime');
  });

  it('supports string diff payloads used by PR review canvases', () => {
    const html = `<!DOCTYPE html>
<script>legacy runtime</script>
<script type="module" data-revision="rev_diff">
const { DiffStats, DiffView } = window.BitfunCanvasSDK;
const { h } = window.BitfunCanvasRuntime;
const diffLines = '+added\\n-removed\\n unchanged';
function Canvas() {
  return h('div', null, h(DiffStats, { additions: 1, deletions: -1 }), h(DiffView, { lines: diffLines }));
}
window.BitfunCanvasRuntime.mount(Canvas);
    </script>`;
    const wrapped = buildReactCanvasHtml(html, { title: 'Diff' });

    expect(wrapped).toContain('window.BitfunCanvasSDKAdapters');
    expect(wrapped).toContain('normalizeDiffLines:');
    expect(wrapped).not.toContain('function normalizeDiffLines(lines)');
    expect(wrapped).not.toContain('legacy runtime');
  });

  it('smoke-renders bundled SDK components in the iframe shell', async () => {
    const html = buildReactCanvasHtml(`<!DOCTYPE html>
<script type="module" data-revision="rev_smoke">
const { Stack, Card, CardHeader, CardBody, BarChart, CollapsibleSection, DependencyGraph, Empty, FlowDiagram, Input, Tabs, Text } = window.BitfunCanvasSDK;
const { h } = window.BitfunCanvasRuntime;
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
window.BitfunCanvasRuntime.mount(Canvas);
</script>`, { title: 'Smoke' });

    expect(html).toBeTruthy();
    const { dom, messages } = await runCanvasHtml(html ?? '');

    try {
      expect(messages).toContainEqual(expect.objectContaining({ type: 'bitfun-canvas-module-started' }));
      expect(messages).toContainEqual(expect.objectContaining({ type: 'bitfun-canvas-ready' }));
      expect(messages).not.toContainEqual(expect.objectContaining({ type: 'bitfun-canvas-runtime-error' }));
      expect(dom.window.document.body.textContent).toContain('Runtime smoke');
      expect(dom.window.document.querySelector('.bf-chart')).toBeTruthy();
      expect(dom.window.document.querySelector('.bf-collapsible-section')).toBeTruthy();
      expect(dom.window.document.querySelector('.bf-diagram')).toBeTruthy();
      expect(dom.window.document.querySelector('.bitfun-tabs')).toBeTruthy();
      expect(dom.window.document.querySelector('.bitfun-input-wrapper')).toBeTruthy();
      expect(dom.window.document.querySelector('.bitfun-empty')).toBeTruthy();
    } finally {
      dom.window.close();
    }
  });
});
