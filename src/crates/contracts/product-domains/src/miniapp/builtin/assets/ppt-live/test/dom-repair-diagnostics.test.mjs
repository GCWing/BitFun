import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

import { extractSlideDataFromDocument } from '../src/html2pptx-dom-core.js';
import { buildSlideFromExtracted } from '../src/pptx-html-build.js';
import { sanitizeSlideDocumentRoot } from '../src/sanitize-slide-html.js';
import {
  HTML_ALLOWED_TAGS,
  SVG_ALLOWED_TAGS,
  isAllowedSanitizedAttribute,
  sanitizeSlideDocument,
} from '../src/sanitize-slide-markup.js';
import {
  buildPageVisualFallbackRequest,
  buildRasterFallbackRequests,
  buildWholePageVisualFallbackRequest,
  renderRasterFallbackPlan,
  renderRasterFallbackLayers,
} from '../src/fallback-layer-render.js';
import {
  formatLocalizedExportDiagnostic,
  sanitizeDiagnosticSourceId,
  summarizePptxExportDiagnostics,
} from '../src/export-diagnostics.js';
import { STRINGS } from '../src/i18n.js';

const requireFromWebUi = createRequire(
  new URL('../../../../../../../../../web-ui/package.json', import.meta.url),
);
const { JSDOM, VirtualConsole } = requireFromWebUi('jsdom');

function createSilentDom(markup, options = {}) {
  return new JSDOM(markup, {
    ...options,
    virtualConsole: new VirtualConsole(),
  });
}

function createDocument(bodyHtml, css = '') {
  const dom = createSilentDom(`<!doctype html><html><head><style>
    html, body { width: 1280px; height: 720px; margin: 0; }
    body { font: 20px/1.3 Arial, sans-serif; }
    ${css}
  </style></head><body>${bodyHtml}</body></html>`, {
    pretendToBeVisual: true,
  });
  installMeasurableLayout(dom.window.document);
  return dom.window.document;
}

function installMeasurableLayout(doc) {
  const rect = (left, top, width, height) => ({
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON() {
      return { left, top, width, height };
    },
  });
  Object.defineProperties(doc.body, {
    scrollWidth: { configurable: true, value: 1280 },
    scrollHeight: { configurable: true, value: 720 },
  });
  doc.body.getBoundingClientRect = () => rect(0, 0, 1280, 720);
  [...doc.body.querySelectorAll('*')].forEach((element, index) => {
    element.getBoundingClientRect = () => rect(40, 30 + index * 36, 640, 30);
    Object.defineProperties(element, {
      offsetWidth: { configurable: true, value: 640 },
      offsetHeight: { configurable: true, value: 30 },
      scrollHeight: { configurable: true, value: 30 },
    });
  });
  doc.createRange = () => ({
    selectNodeContents(element) {
      this.element = element;
    },
    getBoundingClientRect() {
      return this.element?.getBoundingClientRect() || rect(0, 0, 0, 0);
    },
    detach() {},
  });
}

test('shared markup sanitizer removes active content and unsafe resource URLs for every export surface', () => {
  const dom = createSilentDom(`<!doctype html><html><head>
    <base href="https://attacker.invalid/"><meta http-equiv="refresh" content="0;url=https://attacker.invalid">
    <style>.card{background:url(javascript:alert(1))}</style>
    </head><body onload="alert(1)">
    <a href=" javascript:alert(1)" onclick="alert(1)">link</a>
    <img id="remote" src="https://attacker.invalid/image.png"><img id="local" src="assets/image.png"><iframe src="/frame"></iframe>
    <svg><a xlink:href="vbscript:msgbox(1)"><circle onmouseover="alert(1)"/></a><foreignObject>bad</foreignObject></svg>
    <math><maction actiontype="statusline">bad</maction></math>
    </body></html>`);
  const sanitized = sanitizeSlideDocument(dom.window.document).documentElement.outerHTML;

  for (const unsafe of ['<script', '<iframe', '<base', '<meta', 'onload=', 'onclick=', 'onmouseover=', 'javascript:', 'vbscript:', 'https://attacker.invalid', '<maction']) {
    assert.doesNotMatch(sanitized.toLowerCase(), new RegExp(unsafe.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }
  assert.match(sanitized, /<a>link<\/a>/);
  assert.match(sanitized, /id="local"/);
  assert.doesNotMatch(sanitized, /assets\/image\.png/);
});

test('sanitizer fail-closes escaped CSS and applies element-specific resource rules', () => {
  const dom = createSilentDom(`<!doctype html><html><head>
    <style id="escaped-import">\\40 import "https://evil.invalid/a.css";</style>
    <style id="safe-style">.card{background:linear-gradient(red,blue)}</style>
    </head><body>
    <form action="data:text/html,bad"><button formaction="blob:bad">Submit</button></form>
    <a id="anchor" href="#safe">Text remains</a>
    <img id="safe-png" src="data:image/png;base64,AA=="
      srcset="data:image/png;base64,AA== 1x" style="color:red">
    <img id="bad-svg" src="data:image/svg+xml,%3Csvg%20onload%3Dalert(1)%3E">
    <video id="poster" poster="data:image/webp;base64,AA=="></video>
    <div id="escaped-url" style="background:\\75\\72\\6c (javascript:alert(1));color:red"></div>
    <div id="escaped-expression" style="width:e\\78 pression(alert(1));height:10px"></div>
    <svg><use id="svg-local" xlink:href="#shape"></use><use id="svg-remote" href="data:image/png;base64,AA=="></use></svg>
    </body></html>`);

  const sanitized = sanitizeSlideDocument(dom.window.document);

  assert.equal(sanitized.querySelector('form'), null);
  assert.equal(sanitized.querySelector('#anchor').hasAttribute('href'), false);
  assert.equal(sanitized.querySelector('#anchor').textContent, 'Text remains');
  assert.equal(sanitized.querySelector('#safe-png').getAttribute('src'), 'data:image/png;base64,AA==');
  assert.equal(sanitized.querySelector('#safe-png').hasAttribute('srcset'), false);
  assert.equal(sanitized.querySelector('#bad-svg').hasAttribute('src'), false);
  assert.equal(sanitized.querySelector('#poster'), null);
  assert.equal(sanitized.querySelector('#escaped-url').hasAttribute('style'), false);
  assert.equal(sanitized.querySelector('#escaped-expression').hasAttribute('style'), false);
  assert.equal(sanitized.querySelector('#escaped-import'), null);
  assert.match(sanitized.querySelector('#safe-style').textContent, /linear-gradient/);
  assert.equal(sanitized.querySelector('#svg-local').getAttribute('xlink:href'), '#shape');
  assert.equal(sanitized.querySelector('#svg-remote').hasAttribute('href'), false);
});

test('sanitizer enforces tag and attribute allowlists plus embedded-resource CSS denial', () => {
  const dom = createSilentDom(`<!doctype html><html><head>
    <link rel="preload" imagesrcset="https://evil.invalid/a.png">
    <style id="image-set">.x{background:image-set("https://evil.invalid/a.png" 1x)}</style>
    <style id="escaped-image-set">.x{background:\\69 mage-set("data:image/png;base64,AA==" 1x)}</style>
    <style id="safe-css">.x{display:grid;transform:translateX(2px);color:#123;background:linear-gradient(red,blue)}</style>
    </head><body mystery="bad" data-safe="ok" data-url="javascript:alert(1)">
    <marquee weird="value">Visible <custom-tag unknown="x">child</custom-tag></marquee>
    <div id="layout" unknown-attr="bad" style="display:flex;transform:rotate(2deg);color:red"></div>
    <svg viewBox="0 0 100 100" unknown-svg="bad">
      <defs>
        <linearGradient id="paint"><stop offset="0" stop-color="red"/></linearGradient>
        <symbol id="icon" viewBox="0 0 10 10"><path d="M0 0L1 1"/></symbol>
        <marker id="arrow" markerWidth="4" markerHeight="4" refX="2" refY="2"><path d="M0 0L4 2L0 4Z"/></marker>
      </defs>
      <g transform="translate(2 3)" fill="#fff"><path d="M0 0L1 1" stroke="#000" marker-end="url(#arrow)"/></g>
      <use href="#paint" externalResourcesRequired="true"/>
      <text><textPath href="#route" startOffset="5%">Path text</textPath></text>
      <unknown-svg-tag odd="bad"><text x="2" y="3">SVG text</text></unknown-svg-tag>
    </svg>
    </body></html>`);

  const sanitized = sanitizeSlideDocument(dom.window.document);
  const body = sanitized.body;

  assert.equal(sanitized.querySelector('link,#image-set,#escaped-image-set'), null);
  assert.match(sanitized.querySelector('#safe-css').textContent, /linear-gradient/);
  assert.equal(body.hasAttribute('mystery'), false);
  assert.equal(body.getAttribute('data-safe'), 'ok');
  assert.equal(body.hasAttribute('data-url'), false);
  assert.equal(sanitized.querySelector('marquee,custom-tag,unknown-svg-tag'), null);
  assert.match(body.textContent, /Visible child/);
  assert.equal(sanitized.querySelector('#layout').hasAttribute('unknown-attr'), false);
  assert.match(sanitized.querySelector('#layout').getAttribute('style'), /display:\s*flex/);
  assert.equal(sanitized.querySelector('svg').hasAttribute('unknown-svg'), false);
  assert.equal(sanitized.querySelector('use').getAttribute('href'), '#paint');
  assert.equal(sanitized.querySelector('use').hasAttribute('externalResourcesRequired'), false);
  assert.ok(sanitized.querySelector('symbol,marker,textPath'));
  assert.equal(sanitized.querySelector('textPath').getAttribute('href'), '#route');
  assert.match(sanitized.querySelector('svg').textContent, /SVG text/);

  sanitized.querySelectorAll('*').forEach((node) => {
    const tag = node.localName.toLowerCase();
    const allowedTags = node.namespaceURI === 'http://www.w3.org/2000/svg'
      ? SVG_ALLOWED_TAGS
      : HTML_ALLOWED_TAGS;
    assert.ok(allowedTags.has(tag), `unexpected sanitized tag: ${tag}`);
    [...node.attributes].forEach((attribute) => {
      assert.equal(
        isAllowedSanitizedAttribute(node, attribute.name, attribute.value),
        true,
        `unexpected sanitized attribute: ${tag}.${attribute.name}`,
      );
    });
  });
});

test('whole-page visual request owns native visual suppression at construction', () => {
  const doc = createDocument('<div data-pptx-source-id="panel"></div><p data-pptx-source-id="copy">Text</p>');
  sanitizeSlideDocumentRoot(doc);
  const request = buildWholePageVisualFallbackRequest(
    doc,
    [{ code: 'canvas_overflow', severity: 'fallback' }],
    ['panel', 'image', 'panel'],
  );
  assert.deepEqual(request.suppressedNativeVisualIds, ['panel', 'image']);
});

test('all SVG paint-server and resource presentation attributes canonicalize to local fragments', () => {
  const dom = createSilentDom('<!doctype html><html><body><svg id="root"></svg></body></html>');
  const doc = dom.window.document;
  const svg = doc.querySelector('svg');
  const resourceAttributes = [
    'fill', 'stroke', 'filter', 'clip-path', 'mask',
    'marker-start', 'marker-mid', 'marker-end',
  ];
  const appendPath = (id, attribute, value) => {
    const path = doc.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('id', id);
    path.setAttribute('d', 'M0 0L1 1');
    path.setAttribute(attribute, value);
    svg.append(path);
    return path;
  };

  resourceAttributes.forEach((attribute, index) => {
    appendPath(`local-${index}`, attribute, 'url(#safe-paint)');
    appendPath(`external-${index}`, attribute, 'url(https://evil.invalid/paint.svg#x)');
    appendPath(`escaped-${index}`, attribute, '\\75\\72\\6c (https://evil.invalid/paint.svg#x)');
  });
  appendPath('fill-color', 'fill', '#123456');
  appendPath('stroke-color', 'stroke', 'currentColor');
  [
    ['protocol-relative', 'fill', 'url(//evil.invalid/x)'],
    ['data-resource', 'stroke', 'url(data:image/svg+xml,bad)'],
    ['blob-resource', 'filter', 'url(blob:secret)'],
    ['file-resource', 'mask', 'url(file:///tmp/secret.svg)'],
  ].forEach(([id, attribute, value]) => appendPath(id, attribute, value));
  const useLocal = doc.createElementNS('http://www.w3.org/2000/svg', 'use');
  useLocal.id = 'href-local';
  useLocal.setAttribute('href', '\\23 safe-paint');
  svg.append(useLocal);
  const useExternal = doc.createElementNS('http://www.w3.org/2000/svg', 'use');
  useExternal.id = 'href-external';
  useExternal.setAttribute('href', '\\68 ttps://evil.invalid/x.svg');
  svg.append(useExternal);

  const sanitized = sanitizeSlideDocument(doc);

  resourceAttributes.forEach((attribute, index) => {
    assert.equal(sanitized.querySelector(`#local-${index}`).getAttribute(attribute), 'url(#safe-paint)');
    assert.equal(sanitized.querySelector(`#external-${index}`).hasAttribute(attribute), false);
    assert.equal(sanitized.querySelector(`#escaped-${index}`).hasAttribute(attribute), false);
  });
  assert.equal(sanitized.querySelector('#fill-color').getAttribute('fill'), '#123456');
  assert.equal(sanitized.querySelector('#stroke-color').getAttribute('stroke'), 'currentColor');
  for (const id of ['protocol-relative', 'data-resource', 'blob-resource', 'file-resource']) {
    const element = sanitized.querySelector(`#${id}`);
    assert.equal(
      [...element.attributes].some((attribute) => resourceAttributes.includes(attribute.name)),
      false,
      id,
    );
  }
  assert.equal(sanitized.querySelector('#href-local').getAttribute('href'), '#safe-paint');
  assert.equal(sanitized.querySelector('#href-external').hasAttribute('href'), false);
  sanitized.querySelectorAll('svg *').forEach((node) => {
    [...node.attributes].forEach((attribute) => {
      assert.equal(isAllowedSanitizedAttribute(node, attribute.name, attribute.value), true);
    });
  });
});

test('repairs consecutive manual bullets into one semantic list with preserved styles', () => {
  const doc = createDocument(`
    <section>
      <p style="margin-left: 24px; color: rgb(12, 34, 56)">  • First item</p>
      <h3 style="margin-left: 24px"><strong>● Second</strong> item</h3>
      <p>Ordinary paragraph</p>
    </section>
  `);

  const { diagnostics } = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);

  const list = doc.querySelector('section > ul');
  assert.ok(list);
  assert.equal(list.children.length, 2);
  assert.deepEqual([...list.children].map((item) => item.textContent.trim()), ['First item', 'Second item']);
  assert.equal(list.children[0].style.marginLeft, '24px');
  assert.equal(doc.querySelector('section > p').textContent, 'Ordinary paragraph');
  assert.equal(slideData.elements.filter((element) => element.type === 'list').length, 1);
  assert.ok(!slideData.errors.some((message) => message.includes('starts with bullet symbol')));
  assert.ok(diagnostics.some((item) => item.code === 'manual_bullet_list' && item.severity === 'repaired'));
});

test('removes a manual bullet split from its text by inline formatting', () => {
  const doc = createDocument(`
    <section>
      <p><strong>•</strong> Item one</p>
      <p><span>●</span> Item two</p>
    </section>
  `);

  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);
  const list = slideData.elements.find((element) => element.type === 'list');

  assert.deepEqual([...doc.querySelectorAll('li')].map((item) => item.textContent.trim()), ['Item one', 'Item two']);
  assert.ok(list);
  assert.doesNotMatch(list.items.map((run) => run.text).join(''), /[•●]/u);
});

test('does not classify a dash without following whitespace as a manual bullet', () => {
  const doc = createDocument('<p>–40°C remains a normal sentence</p>');

  sanitizeSlideDocumentRoot(doc);

  assert.equal(doc.querySelectorAll('ul, ol').length, 0);
  assert.equal(doc.querySelector('p').textContent, '–40°C remains a normal sentence');
});

test('keeps standalone en/em dash asides but repairs consecutive dash lists', () => {
  const doc = createDocument(`
    <section id="asides">
      <p>– This is an aside</p>
      <p>Ordinary separator</p>
      <p>— This is another aside</p>
    </section>
    <section id="dash-list">
      <p>– First dash item</p>
      <p>— Second dash item</p>
    </section>
  `);

  sanitizeSlideDocumentRoot(doc);

  assert.equal(doc.querySelectorAll('#asides ul').length, 0);
  assert.deepEqual(
    [...doc.querySelectorAll('#asides > p')].map((item) => item.textContent),
    ['– This is an aside', 'Ordinary separator', '— This is another aside'],
  );
  assert.deepEqual(
    [...doc.querySelectorAll('#dash-list li')].map((item) => item.textContent),
    ['First dash item', 'Second dash item'],
  );
});

test('repairs consecutive ambiguous dash bullets but leaves a single dash sentence alone', () => {
  const doc = createDocument(`
    <section id="list"><p>- First</p><p>- Second</p></section>
    <section id="sentence"><p>- This standalone sentence is intentionally unchanged.</p></section>
  `);

  sanitizeSlideDocumentRoot(doc);

  assert.deepEqual(
    [...doc.querySelectorAll('#list li')].map((item) => item.textContent),
    ['First', 'Second'],
  );
  assert.equal(doc.querySelector('#sentence > p').textContent, '- This standalone sentence is intentionally unchanged.');
});

test('repairs direct text, decorated spans, nested paragraphs, and merge text without reordering', () => {
  const doc = createDocument(`
    <div id="direct">Alpha <em>middle</em> Omega</div>
    <div id="decorated"><span style="background-color: rgb(255, 0, 0)">Badge</span></div>
    <div data-pptx-merge="true">Lead <p>Second</p> Tail</div>
  `);
  const outer = doc.createElement('p');
  outer.append('Before ');
  const inner = doc.createElement('p');
  inner.textContent = 'Nested';
  outer.append(inner, ' After');
  doc.body.appendChild(outer);
  installMeasurableLayout(doc);
  const beforeText = doc.body.textContent.replace(/\s+/g, ' ').trim();

  const { diagnostics } = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);

  assert.equal(doc.body.textContent.replace(/\s+/g, ' ').trim(), beforeText);
  assert.equal(doc.querySelector('#direct > p')?.textContent.replace(/\s+/g, ' ').trim(), 'Alpha middle Omega');
  assert.equal(doc.querySelector('#direct > p > em')?.textContent, 'middle');
  assert.equal(doc.querySelector('#decorated > p')?.textContent, 'Badge');
  assert.equal(doc.querySelector('p p'), null);
  assert.deepEqual(
    [...doc.querySelector('[data-pptx-merge="true"]').children].map((node) => node.textContent.trim()),
    ['Lead', 'Second', 'Tail'],
  );
  const merged = slideData.elements.find((element) => element.type === 'merged-text');
  assert.ok(merged);
  assert.deepEqual(merged.items.map((run) => run.text.trim()), ['Lead', 'Second', 'Tail']);
  assert.equal(merged.items[0].options.breakLine, true);
  assert.equal(merged.items[1].options.breakLine, true);
  for (const code of ['direct_text_wrapped', 'decorated_inline_promoted', 'nested_paragraph_repaired']) {
    assert.ok(diagnostics.some((item) => item.code === code && item.severity === 'repaired'), code);
  }
});

test('preserves explicit source ids and assigns stable generated ids', () => {
  const doc = createDocument(`
    <p data-pptx-source-id="author-title">Title</p>
    <p>• Generated item</p>
  `);

  sanitizeSlideDocumentRoot(doc);
  const firstIds = [...doc.querySelectorAll('[data-pptx-source-id]')]
    .map((element) => element.dataset.pptxSourceId);
  sanitizeSlideDocumentRoot(doc);
  const secondIds = [...doc.querySelectorAll('[data-pptx-source-id]')]
    .map((element) => element.dataset.pptxSourceId);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);

  assert.ok(firstIds.includes('author-title'));
  assert.deepEqual(secondIds, firstIds);
  assert.equal(new Set(firstIds).size, firstIds.length);
  assert.ok(slideData.elements.some((element) => element.sourceId === 'author-title'));
  assert.ok(slideData.elements
    .filter((element) => ['p', 'list'].includes(element.type))
    .every((element) => element.sourceId));
});

test('repairs duplicate authored source ids uniquely and remains stable on rerun', () => {
  const doc = createDocument(`
    <p data-pptx-source-id="duplicate">First</p>
    <p data-pptx-source-id="duplicate">Second</p>
    <p data-pptx-source-id="pptx-source-1">Third</p>
    <p>Fourth</p>
  `);

  sanitizeSlideDocumentRoot(doc);
  const firstIds = [...doc.body.querySelectorAll('p')].map((element) => element.dataset.pptxSourceId);
  sanitizeSlideDocumentRoot(doc);
  const secondIds = [...doc.body.querySelectorAll('p')].map((element) => element.dataset.pptxSourceId);

  assert.equal(firstIds[0], 'duplicate');
  assert.equal(new Set(firstIds).size, firstIds.length);
  assert.deepEqual(secondIds, firstIds);
});

test('classifies unsupported visual capabilities as fallback diagnostics', () => {
  const doc = createDocument(`
    <div id="gradient" style="background-image: linear-gradient(red, blue)">Gradient</div>
    <div id="filter" style="filter: blur(2px)">Filtered</div>
    <svg id="complex-svg"><defs><filter id="blur"></filter></defs><path d="M0 0 L10 10"></path></svg>
    <div id="pseudo" class="with-pseudo">Pseudo</div>
  `, '.with-pseudo::before { content: "prefix"; }');

  const { diagnostics } = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const extracted = extractSlideDataFromDocument(doc);

  for (const code of ['css_gradient', 'css_filter', 'complex_svg_raster', 'generated_content']) {
    const diagnostic = diagnostics.find((item) => item.code === code);
    assert.equal(diagnostic?.severity, 'fallback', code);
    assert.ok(diagnostic.sourceId, code);
    assert.ok(diagnostic.tag, code);
  }
  assert.ok(!diagnostics.some((item) => item.severity === 'blocking'));
  for (const code of ['css_filter', 'complex_svg_raster']) {
    assert.equal(
      extracted.diagnostics.find((item) => item.code === code)?.severity,
      'fallback',
      code,
    );
  }
});

test('comprehensive DOM fixture preserves editable text and every native or fallback visual', async () => {
  const doc = createDocument(`
    <div data-pptx-source-id="background-panel" style="background-color: rgb(10, 20, 30); z-index: 0"></div>
    <section data-pptx-source-id="content-layer" style="z-index: 2">
      <p data-pptx-source-id="manual-one">• Manual one</p>
      <p data-pptx-source-id="manual-two">• Manual <strong>two</strong></p>
      <p data-pptx-source-id="mixed-run">Plain <strong>bold</strong> <em>italic</em></p>
      <div data-pptx-source-id="merge-copy" data-pptx-merge="true">
        <p>Merged first</p><p>Merged second</p>
      </div>
      <table data-pptx-source-id="table">
        <tr><th data-pptx-source-id="table-head">Header text</th></tr>
        <tr><td data-pptx-source-id="table-cell">Cell text</td></tr>
      </table>
      <img data-pptx-source-id="photo" alt="Photo" src="data:image/png;base64,AA==" />
      <div data-pptx-source-id="css-shape" style="background-color: rgb(1, 2, 3); border: 2px solid rgb(4, 5, 6)"></div>
      <div data-pptx-source-id="css-triangle" style="
        width: 0; height: 0; border-left: 20px solid transparent;
        border-right: 20px solid transparent; border-bottom: 30px solid rgb(255, 0, 0);
      "></div>
      <svg data-pptx-source-id="basic-svg" viewBox="0 0 100 100">
        <rect data-pptx-source-id="svg-rect" x="5" y="5" width="30" height="20" fill="#00ff00"/>
        <line data-pptx-source-id="svg-line" x1="0" y1="50" x2="90" y2="50" stroke="#0000ff"/>
        <text data-pptx-source-id="svg-text" x="5" y="90">Basic SVG text</text>
      </svg>
      <svg data-pptx-source-id="complex-svg" viewBox="0 0 100 100">
        <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
        <path data-pptx-source-id="complex-path" d="M0 0L90 90" filter="url(#blur)"/>
        <text data-pptx-source-id="complex-label" x="5" y="90">Complex SVG label</text>
      </svg>
      <div data-pptx-source-id="gradient" style="background-image: linear-gradient(red, blue)">
        <p data-pptx-source-id="gradient-label">Gradient label</p>
      </div>
      <div data-pptx-source-id="filtered" style="filter: blur(2px)">
        <p data-pptx-source-id="filtered-label">Filtered label</p>
      </div>
      <div data-pptx-source-id="pseudo" class="fixture-pseudo"><p>Pseudo label</p></div>
    </section>
    <p data-pptx-source-id="foreground-copy" style="z-index: 9">Foreground copy</p>
  `, '.fixture-pseudo::before { content: "Generated star"; color: red; }');
  const beforeText = doc.body.textContent.replace(/\s+/g, ' ').trim();

  const repair = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const extracted = extractSlideDataFromDocument(doc);
  const allDiagnostics = [...repair.diagnostics, ...extracted.diagnostics];
  const localRequests = buildRasterFallbackRequests(doc, allDiagnostics);
  const rendered = await renderRasterFallbackLayers(
    localRequests,
    async (_html, _slideIndex, metadata) => Buffer.from(`png:${metadata.sourceId}`).toString('base64'),
    0,
  );
  const preparedModel = {
    ...extracted,
    fallbackLayers: [...(extracted.fallbackLayers || []), ...rendered.layers],
    diagnostics: allDiagnostics,
  };

  const repairedDomText = doc.body.textContent.replace(/\s+/g, ' ').trim();
  for (const phrase of ['Manual one', 'Manual two', 'Plain bold italic', 'Merged first', 'Merged second']) {
    assert.ok(beforeText.includes(phrase) && repairedDomText.includes(phrase), phrase);
  }
  const editableText = preparedModel.elements.flatMap((element) => {
    if (typeof element.text === 'string') return [element.text];
    const runs = element.items || element.text || [];
    return Array.isArray(runs) ? [runs.map((run) => run.text).join('')] : [];
  }).join(' | ');
  for (const text of [
    'Manual one', 'Manual two', 'Plain bold italic', 'Merged first', 'Merged second',
    'Header text', 'Cell text', 'Basic SVG text', 'Complex SVG label',
    'Gradient label', 'Filtered label', 'Pseudo label', 'Foreground copy',
  ]) {
    assert.match(editableText, new RegExp(text), text);
  }

  const diagnosticCodes = new Set(preparedModel.diagnostics.map((item) => item.code));
  for (const code of ['manual_bullet_list', 'css_gradient', 'css_filter', 'complex_svg_raster', 'generated_content']) {
    assert.ok(diagnosticCodes.has(code), code);
  }
  assert.ok(preparedModel.diagnostics.some((item) => item.severity === 'repaired'));
  assert.ok(preparedModel.diagnostics.some((item) => item.severity === 'fallback'));
  assert.ok(!preparedModel.diagnostics.some((item) => item.severity === 'blocking'));

  const nativeBySource = new Map(preparedModel.elements.map((item) => [item.sourceId, item]));
  assert.equal(nativeBySource.get('photo')?.type, 'image');
  assert.equal(nativeBySource.get('css-shape')?.type, 'shape');
  assert.equal(nativeBySource.get('css-triangle')?.svgType, 'triangle');
  assert.equal(nativeBySource.get('svg-rect')?.type, 'svg-shape');
  assert.equal(nativeBySource.get('svg-line')?.type, 'line');
  const fallbackSources = new Set(preparedModel.fallbackLayers.map((item) => item.sourceId));
  for (const sourceId of ['complex-svg', 'gradient', 'filtered', 'pseudo']) {
    assert.ok(fallbackSources.has(sourceId), sourceId);
  }
  assert.equal(preparedModel.background.value, '0a141e');
  const background = nativeBySource.get('css-shape');
  const foreground = nativeBySource.get('foreground-copy');
  assert.ok(background.paintOrder < foreground.paintOrder);
  assert.ok(background.zIndex < foreground.zIndex);
});

test('returns blocking diagnostics for an unmeasurable slide canvas', () => {
  const doc = createDocument('<p>Unreadable geometry</p>');
  doc.body.getBoundingClientRect = () => ({
    left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0,
  });

  const slideData = extractSlideDataFromDocument(doc);
  const diagnostic = slideData.diagnostics.find((item) => item.code === 'unmeasurable_canvas');

  assert.equal(diagnostic?.severity, 'blocking');
  assert.equal(diagnostic?.kind, 'blocking');
  assert.equal(diagnostic?.tag, 'body');
});

test('returns a located unreadable_document blocking diagnostic', () => {
  const slideData = extractSlideDataFromDocument(null);
  const diagnostic = slideData.diagnostics.find((item) => item.code === 'unreadable_document');

  assert.equal(diagnostic?.severity, 'blocking');
  assert.equal(diagnostic?.kind, 'blocking');
  assert.equal(diagnostic?.sourceId, 'slide-document');
  assert.equal(diagnostic?.tag, 'document');
  assert.deepEqual(slideData.errors, [diagnostic.message]);
});

test('emits a structured pptx_serialization blocking diagnostic from the production builder', async () => {
  const serializationFailure = new Error('addText serialization failed');
  const targetSlide = {
    addText() {
      throw serializationFailure;
    },
  };
  const slideData = {
    background: { type: 'color', value: 'FFFFFF' },
    elements: [{
      type: 'p',
      text: 'Serializable text',
      position: { x: 1, y: 1, w: 4, h: 1 },
      style: { fontSize: 20, fontFace: 'Arial', color: '111111', align: 'left' },
      sourceId: 'source-text',
    }],
    placeholders: [],
    diagnostics: [],
    errors: [],
  };

  await assert.rejects(
    buildSlideFromExtracted(
      slideData,
      { width: 1280, height: 720, errors: [] },
      { addSlide: () => targetSlide, ShapeType: {} },
    ),
    (error) => {
      assert.equal(error, serializationFailure);
      assert.equal(error.diagnostic?.severity, 'blocking');
      assert.equal(error.diagnostic?.code, 'pptx_serialization');
      assert.match(error.diagnostic?.message || '', /addText serialization failed/);
      assert.ok(error.diagnostics.includes(error.diagnostic));
      return true;
    },
  );
});

test('keeps editable HTML objects and emits native basic SVG primitives with source paint metadata', () => {
  const doc = createDocument(`
    <div id="panel" style="background-color: rgb(10, 20, 30); border: 2px solid rgb(40, 50, 60)">
      <p id="title">Editable title</p>
      <img id="photo" src="data:image/png;base64,AA==" />
    </div>
    <svg id="art" viewBox="0 0 100 100">
      <rect id="rect" x="10" y="20" width="30" height="40" fill="#ff0000" stroke="#000000"/>
      <circle id="circle" cx="60" cy="30" r="10" fill="#00ff00"/>
      <line id="line" x1="0" y1="0" x2="100" y2="100" stroke="#0000ff"/>
      <text id="label" x="10" y="90">SVG label</text>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const extracted = extractSlideDataFromDocument(doc);
  const svgElements = extracted.elements.filter((element) => (
    element.type === 'svg-shape' || element.type === 'svg-text' || element.type === 'line'
  ));

  assert.ok(extracted.elements.some((element) => element.type === 'p' && element.text === 'Editable title'),
    'editable HTML text must remain when SVG exists');
  assert.ok(extracted.elements.some((element) => element.type === 'image'),
    'editable HTML image must remain when SVG exists');
  assert.ok(extracted.elements.some((element) => element.type === 'shape'));
  assert.equal(svgElements.length, 4);
  assert.deepEqual(svgElements.map((element) => element.kind), ['native', 'native', 'native', 'native']);
  assert.ok(svgElements.every((element) => element.sourceId && Number.isFinite(element.zIndex)));
  assert.equal(svgElements.find((element) => element.text === 'SVG label')?.type, 'svg-text');
});

test('serializes fallback layers in paint order without removing editable objects', async () => {
  const calls = [];
  const slide = {
    addText(value, options) { calls.push({ op: 'text', value, options }); },
    addImage(options) { calls.push({ op: 'image', options }); },
    addShape(type, options) { calls.push({ op: 'shape', type, options }); },
  };
  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [
      {
        type: 'p',
        text: 'Editable',
        sourceId: 'text',
        zIndex: 20,
        kind: 'native',
        position: { x: 1, y: 1, w: 4, h: 1 },
        style: { fontSize: 20, fontFace: 'Arial', color: '111111', align: 'left' },
      },
    ],
    fallbackLayers: [{
      sourceId: 'complex-visual',
      kind: 'raster',
      zIndex: 10,
      bbox: { x: 0, y: 0, w: 2, h: 2 },
      data: 'data:image/png;base64,AA==',
      diagnostics: [{ severity: 'fallback', code: 'css_gradient', message: 'gradient' }],
    }],
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: { line: 'line', rect: 'rect', roundRect: 'roundRect' },
  });

  assert.deepEqual(calls.map((call) => call.op), ['image', 'text']);
  assert.equal(calls[0].options.x, 0);
  assert.equal(calls[1].value, 'Editable');
});

test('maps SVG polyline and recognized polygons to editable geometry with local fill fidelity', () => {
  const doc = createDocument(`
    <p>Editable stays</p>
    <svg viewBox="0 0 100 100" style="z-index: 7">
      <polyline data-pptx-source-id="route" points="0,0 20,10 40,0" fill="none" stroke="#123456"/>
      <polygon data-pptx-source-id="triangle" points="50,40 70,80 30,80" fill="#ff0000"/>
      <polygon data-pptx-source-id="diamond" points="80,10 95,25 80,40 65,25" fill="#00ff00"/>
      <polygon data-pptx-source-id="freeform" points="5,50 20,45 32,62 18,85 3,70" fill="#abcdef" stroke="#010203"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const slideData = extractSlideDataFromDocument(doc);
  const route = slideData.elements.filter((element) => element.sourceId === 'route');
  const triangle = slideData.elements.find((element) => element.sourceId === 'triangle');
  const diamond = slideData.elements.find((element) => element.sourceId === 'diamond');
  const freeform = slideData.elements.filter((element) => element.sourceId === 'freeform');
  const fidelity = slideData.fallbackLayers?.find((layer) => layer.sourceId === 'freeform');

  assert.equal(route.length, 2);
  assert.ok(route.every((element) => element.type === 'line' && element.kind === 'native'));
  assert.equal(triangle?.svgType, 'triangle');
  assert.equal(triangle?.kind, 'native');
  assert.equal(diamond?.svgType, 'diamond');
  assert.equal(freeform.length, 5);
  assert.ok(freeform.every((element) => element.type === 'line' && element.kind === 'native'));
  assert.equal(fidelity?.kind, 'svg-image');
  assert.equal(fidelity?.zIndex, 7);
  assert.match(fidelity?.data || '', /^data:image\/svg\+xml/);
  assert.ok(slideData.elements.some((element) => element.text === 'Editable stays'));
});

test('applies viewBox plus translate scale and rotate transforms to SVG coordinates', () => {
  const doc = createDocument(`
    <svg viewBox="10 20 100 50">
      <line data-pptx-source-id="translated" x1="10" y1="20" x2="20" y2="20" transform="translate(5 10)" stroke="red"/>
      <line data-pptx-source-id="scaled" x1="10" y1="20" x2="20" y2="20" transform="scale(2)" stroke="green"/>
      <line data-pptx-source-id="rotated" x1="10" y1="20" x2="20" y2="20" transform="rotate(90 10 20)" stroke="blue"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const svg = doc.querySelector('svg');
  svg.getBoundingClientRect = () => ({
    x: 100, y: 50, left: 100, top: 50, right: 300, bottom: 150, width: 200, height: 100,
  });

  const slideData = extractSlideDataFromDocument(doc);
  const translated = slideData.elements.find((element) => element.sourceId === 'translated');
  const scaled = slideData.elements.find((element) => element.sourceId === 'scaled');
  const rotated = slideData.elements.find((element) => element.sourceId === 'rotated');

  assert.deepEqual(
    [translated.x1, translated.y1, translated.x2, translated.y2].map((value) => Number(value.toFixed(4))),
    [1.1458, 0.7292, 1.3542, 0.7292],
  );
  assert.deepEqual(
    [scaled.x1, scaled.y1, scaled.x2, scaled.y2].map((value) => Number(value.toFixed(4))),
    [1.25, 0.9375, 1.6667, 0.9375],
  );
  assert.deepEqual(
    [rotated.x1, rotated.y1, rotated.x2, rotated.y2].map((value) => Number(value.toFixed(4))),
    [1.0417, 0.5208, 1.0417, 0.7292],
  );
});

test('serializes path-only complex SVG as a local movable vector layer without duplicating SVG text', () => {
  const doc = createDocument(`
    <p>HTML text remains editable</p>
    <svg id="logo" viewBox="0 0 100 100" style="z-index: 4">
      <path data-pptx-source-id="curve" d="M 5 80 C 20 5, 80 5, 95 80" fill="#ffee00" stroke="#111111"/>
      <text data-pptx-source-id="logo-label" x="10" y="95">Editable SVG text</text>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const slideData = extractSlideDataFromDocument(doc);
  const layer = slideData.fallbackLayers?.find((item) => item.sourceId === 'curve');
  const decoded = decodeURIComponent(String(layer?.data || '').split(',').slice(1).join(','));

  assert.equal(layer?.kind, 'svg-image');
  assert.equal(layer?.zIndex, 4);
  assert.match(decoded, /<path/);
  assert.doesNotMatch(decoded, /Editable SVG text/);
  assert.ok(slideData.elements.some((element) => element.type === 'svg-text' && element.text === 'Editable SVG text'));
  assert.ok(slideData.elements.some((element) => element.text === 'HTML text remains editable'));
});

test('maps a common CSS border triangle to a native editable triangle', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="arrow" style="
      width: 0; height: 0;
      border-left: 20px solid transparent;
      border-right: 20px solid transparent;
      border-bottom: 30px solid rgb(255, 0, 0);
      z-index: 9;
    "></div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const triangle = extractSlideDataFromDocument(doc).elements
    .find((element) => element.sourceId === 'arrow' && element.svgType === 'triangle');

  assert.equal(triangle?.kind, 'native');
  assert.equal(triangle?.zIndex, 9);
  assert.equal(triangle?.shape.fill, 'ff0000');
});

test('fake PPTX receives native shapes text lines and local SVG images while preserving metadata', async () => {
  const calls = [];
  const slide = {
    addText(value, options) { calls.push({ op: 'text', value, options }); },
    addImage(options) { calls.push({ op: 'image', options }); },
    addShape(type, options) { calls.push({ op: 'shape', type, options }); },
  };
  const elements = [
    {
      type: 'svg-shape', svgType: 'triangle', kind: 'native', sourceId: 'triangle', zIndex: 1, paintOrder: 1,
      text: '', position: { x: 1, y: 1, w: 1, h: 1 },
      shape: { fill: 'FF0000', line: null, rectRadius: 0 },
    },
    {
      type: 'line', kind: 'native', sourceId: 'route', zIndex: 3, paintOrder: 3,
      x1: 1, y1: 1, x2: 2, y2: 2, color: '000000', width: 1,
    },
    {
      type: 'svg-text', kind: 'native', sourceId: 'label', zIndex: 4, paintOrder: 4,
      text: 'Vector text', position: { x: 1, y: 1, w: 2, h: 1 },
      style: { fontSize: 16, fontFace: 'Arial', color: '111111', align: 'left' },
    },
  ];
  const fallbackLayers = [{
    sourceId: 'curve', zIndex: 2, paintOrder: 2, kind: 'svg-image',
    bbox: { x: 0, y: 0, w: 3, h: 2 },
    data: 'data:image/svg+xml,%3Csvg%3E%3Cpath%20d%3D%22M0%200L1%201%22%2F%3E%3C%2Fsvg%3E',
    diagnostics: [],
  }];

  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements,
    fallbackLayers,
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: {
      line: 'line', rect: 'rect', roundRect: 'roundRect', ellipse: 'ellipse',
      triangle: 'triangle', diamond: 'diamond',
    },
  });

  assert.deepEqual(calls.map((call) => call.op), ['shape', 'image', 'shape', 'text']);
  assert.equal(calls[0].options.shape, 'triangle');
  assert.equal(calls[1].options.data.startsWith('data:image/svg+xml'), true);
  assert.equal(calls[2].type, 'line');
  assert.equal(calls[3].value, 'Vector text');
  assert.deepEqual(
    elements.map(({ sourceId, zIndex, kind }) => ({ sourceId, zIndex, kind })),
    [
      { sourceId: 'triangle', zIndex: 1, kind: 'native' },
      { sourceId: 'route', zIndex: 3, kind: 'native' },
      { sourceId: 'label', zIndex: 4, kind: 'native' },
    ],
  );
});

test('builds one transparent text-hidden raster HTML request per unsupported source visual', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="gradient" style="background-image: linear-gradient(red, blue)">
      <p>Editable gradient label</p>
    </div>
    <div data-pptx-source-id="filtered" style="filter: blur(2px)">
      <p>Editable filter label</p>
    </div>
    <div data-pptx-source-id="native" style="background: rgb(1, 2, 3)">Unrelated visual</div>
  `);
  const repair = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);
  const diagnostics = [...repair.diagnostics, ...slideData.diagnostics];

  const requests = buildRasterFallbackRequests(doc, diagnostics);

  assert.deepEqual(requests.map((request) => request.sourceId).sort(), ['filtered', 'gradient']);
  assert.ok(requests.every((request) => request.kind === 'raster' && Number.isFinite(request.zIndex)));
  assert.ok(requests.every((request) => request.bbox.w > 0 && request.bbox.h > 0));
  assert.ok(requests.every((request) => request.html === undefined && typeof request.buildHtml === 'function'));
  assert.ok(requests.every((request) => /background:\s*transparent\s*!important/i.test(request.buildHtml())));
  assert.ok(requests.every((request) => /pptx-raster-hide-editable-text/.test(request.buildHtml())));
  assert.ok(requests.every((request) => /data-pptx-raster-target="1"/.test(request.buildHtml())));
  assert.ok(requests.every((request) => /visibility:\s*hidden\s*!important/.test(request.buildHtml())));
  assert.ok(slideData.elements.some((element) => element.text === 'Editable gradient label'));
  assert.ok(slideData.elements.some((element) => element.text === 'Editable filter label'));
});

test('request-build failures escalate without silently dropping unsupported visuals', async (t) => {
  const fullPageRequest = {
    sourceId: 'slide-1',
    zIndex: 0,
    paintOrder: 0,
    kind: 'raster',
    phase: 'full-page',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
    html: '<html><body>full page</body></html>',
    diagnostics: [],
  };
  const runPlan = async (doc, diagnostics, renderRaster) => {
    const localRequests = buildRasterFallbackRequests(doc, diagnostics);
    return {
      localRequests,
      pageVisualRequest: buildPageVisualFallbackRequest(doc, localRequests),
      result: await renderRasterFallbackPlan({
        localRequests,
        pageVisualRequest: buildPageVisualFallbackRequest(doc, localRequests),
        fullPageRequest,
      }, renderRaster, 0),
    };
  };

  await t.test('missing source element records local and page build failures before full-page fallback', async () => {
    const doc = createDocument(`
      <div data-pptx-source-id="valid-gradient" style="background-image:linear-gradient(red,blue)"></div>
      <p>Editable text survives in source HTML</p>
    `);
    const phases = [];
    const { localRequests, pageVisualRequest, result } = await runPlan(doc, [
      { severity: 'fallback', code: 'css_gradient', sourceId: 'missing-gradient' },
      { severity: 'fallback', code: 'css_gradient', sourceId: 'valid-gradient' },
    ], async (_html, _index, metadata) => {
      phases.push(metadata.phase);
      return metadata.phase === 'full-page' ? 'full-page-png' : 'local-png';
    });

    assert.equal(localRequests.length, 2);
    const missingRequest = localRequests.find((request) => request.sourceId === 'missing-gradient');
    assert.equal(missingRequest.buildFailure?.code, 'local_raster_target_missing');
    assert.equal(missingRequest.buildFailure?.stage, 'request-build');
    assert.match(missingRequest.buildFailure?.reason || '', /missing-gradient/);
    assert.equal(pageVisualRequest.buildFailure?.code, 'page_visual_target_missing');
    assert.deepEqual(phases, ['local-visual', 'full-page']);
    assert.equal(result.blocking, false);
    assert.equal(result.fullPageFallback?.data, 'data:image/png;base64,full-page-png');
    assert.ok(result.diagnostics.some((item) => (
      item.code === 'local_raster_target_missing'
        && item.sourceId === 'missing-gradient'
        && item.stage === 'request-build'
        && item.reason
    )));
    assert.ok(result.diagnostics.some((item) => (
      item.code === 'page_visual_target_missing'
        && item.sourceId === 'slide-visuals'
        && item.stage === 'request-build'
        && item.reason
    )));
    assert.ok(result.diagnostics.some((item) => item.code === 'full_page_fallback'));

    const blocked = await runPlan(doc, [
      { severity: 'fallback', code: 'css_gradient', sourceId: 'missing-gradient' },
      { severity: 'fallback', code: 'css_gradient', sourceId: 'valid-gradient' },
    ], async () => {
      throw new Error('full-page host failed');
    });
    assert.equal(blocked.result.blocking, true);
    assert.equal(blocked.result.fullPageFallback, null);
    assert.ok(blocked.result.diagnostics.some((item) => (
      item.code === 'full_page_raster_failed'
        && item.severity === 'blocking'
        && item.stage === 'render'
        && /full-page host failed/.test(item.reason)
    )));
  });

  await t.test('zero-size source skips local renderer but invokes page-visual renderer with source coverage', async () => {
    const doc = createDocument(`
      <div data-pptx-source-id="zero-gradient" style="background-image:linear-gradient(red,blue)">
        <p>Editable zero-size label</p>
      </div>
    `);
    doc.querySelector('[data-pptx-source-id="zero-gradient"]').getBoundingClientRect = () => ({
      x: 40, y: 40, left: 40, top: 40, right: 40, bottom: 40, width: 0, height: 0,
    });
    const phases = [];
    const { localRequests, result } = await runPlan(doc, [
      { severity: 'fallback', code: 'css_gradient', sourceId: 'zero-gradient' },
    ], async (_html, _index, metadata) => {
      phases.push(metadata.phase);
      return 'page-visual-png';
    });

    assert.equal(localRequests[0].buildFailure?.code, 'local_raster_unmeasurable');
    assert.deepEqual(phases, ['page-visual']);
    assert.equal(result.fullPageFallback, null);
    assert.equal(result.layers[0].data, 'data:image/png;base64,page-visual-png');
    assert.deepEqual(result.layers[0].sourceIds, ['zero-gradient']);
    assert.ok(result.diagnostics.some((item) => (
      item.code === 'local_raster_unmeasurable'
        && item.sourceId === 'zero-gradient'
        && item.stage === 'request-build'
    )));
    assert.ok(result.diagnostics.some((item) => item.code === 'page_visual_fallback'));
  });

  await t.test('serialization failure records both build stages and reaches full-page renderer', async () => {
    const doc = createDocument(`
      <div data-pptx-source-id="broken-filter" style="filter:blur(2px)">
        <p>Editable filtered label</p>
      </div>
    `);
    doc.body.cloneNode = () => {
      throw new Error('DOM clone failed');
    };
    const phases = [];
    const { localRequests, pageVisualRequest, result } = await runPlan(doc, [
      { severity: 'fallback', code: 'css_filter', sourceId: 'broken-filter' },
    ], async (_html, _index, metadata) => {
      phases.push(metadata.phase);
      return 'full-page-after-clone-error';
    });

    assert.equal(localRequests[0].buildFailure?.code, 'local_raster_serialize_failed');
    assert.equal(pageVisualRequest.buildFailure?.code, 'page_visual_serialize_failed');
    assert.deepEqual(phases, ['full-page']);
    assert.equal(result.fullPageFallback?.data, 'data:image/png;base64,full-page-after-clone-error');
    for (const code of ['local_raster_serialize_failed', 'page_visual_serialize_failed']) {
      assert.ok(result.diagnostics.some((item) => (
        item.code === code
          && item.stage === 'request-build'
          && /DOM clone failed/.test(item.reason)
      )), code);
    }
  });
});

test('renders local PNG fallback requests independently and preserves layer metadata', async () => {
  const calls = [];
  const requests = [
    {
      sourceId: 'gradient', zIndex: 2, paintOrder: 3, kind: 'raster',
      bbox: { x: 1, y: 1, w: 2, h: 1 }, html: '<html>gradient</html>', diagnostics: [],
    },
    {
      sourceId: 'filter', zIndex: 5, paintOrder: 8, kind: 'raster',
      bbox: { x: 4, y: 2, w: 3, h: 2 }, html: '<html>filter</html>', diagnostics: [],
    },
  ];

  const result = await renderRasterFallbackLayers(
    requests,
    async (html, slideIndex, metadata) => {
      calls.push({ html, slideIndex, metadata });
      return `png-${metadata.sourceId}`;
    },
    6,
  );

  assert.equal(result.failures.length, 0);
  assert.deepEqual(result.layers.map((layer) => layer.sourceId), ['gradient', 'filter']);
  assert.deepEqual(result.layers.map((layer) => layer.data), [
    'data:image/png;base64,png-gradient',
    'data:image/png;base64,png-filter',
  ]);
  assert.deepEqual(result.layers.map(({ bbox, zIndex, paintOrder, kind }) => ({
    bbox, zIndex, paintOrder, kind,
  })), requests.map(({ bbox, zIndex, paintOrder, kind }) => ({
    bbox, zIndex, paintOrder, kind,
  })));
  assert.deepEqual(calls.map((call) => call.metadata.sourceId), ['gradient', 'filter']);
  assert.ok(calls.every((call) => call.slideIndex === 6));
});

test('escalates local raster failure to page visuals and then full-page fallback with located diagnostics', async () => {
  const calls = [];
  const localRequests = [{
    sourceId: 'gradient-card', zIndex: 3, paintOrder: 4, kind: 'raster', phase: 'local-visual',
    bbox: { x: 1, y: 1, w: 2, h: 2 }, html: '<html>local</html>', diagnostics: [],
  }];
  const pageVisualRequest = {
    sourceId: 'slide-visuals', zIndex: 3, paintOrder: 4, kind: 'raster', phase: 'page-visual',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 }, html: '<html>visuals</html>', diagnostics: [],
  };
  const fullPageRequest = {
    sourceId: 'slide-7', zIndex: 0, paintOrder: 0, kind: 'raster', phase: 'full-page',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 }, html: '<html>full</html>', diagnostics: [],
  };

  const result = await renderRasterFallbackPlan({
    localRequests,
    pageVisualRequest,
    fullPageRequest,
  }, async (_html, _index, metadata) => {
    calls.push(metadata);
    if (metadata.phase !== 'full-page') throw new Error(`${metadata.phase} failed`);
    return 'full-page-png';
  }, 6);

  assert.deepEqual(calls.map((call) => call.phase), ['local-visual', 'page-visual', 'full-page']);
  assert.equal(result.layers.length, 0);
  assert.equal(result.fullPageFallback?.data, 'data:image/png;base64,full-page-png');
  assert.equal(result.fullPageFallback?.sourceId, 'slide-7');
  assert.equal(result.blocking, false);
  assert.ok(result.diagnostics.some((diagnostic) => (
    diagnostic.code === 'full_page_fallback'
      && diagnostic.slideNumber === 7
      && diagnostic.sourceId === 'slide-7'
      && diagnostic.phase === 'full-page'
  )));
  assert.ok(result.diagnostics.some((diagnostic) => (
    diagnostic.code === 'local_raster_failed'
      && diagnostic.sourceId === 'gradient-card'
      && /local-visual failed/.test(diagnostic.reason)
  )));
});

test('uses one page-visual layer after a local failure and does not render full page', async () => {
  const phases = [];
  const result = await renderRasterFallbackPlan({
    localRequests: [{
      sourceId: 'pseudo', zIndex: 2, paintOrder: 5, kind: 'raster', phase: 'local-visual',
      bbox: { x: 1, y: 1, w: 1, h: 1 }, html: '<html>local</html>', diagnostics: [],
    }],
    pageVisualRequest: {
      sourceId: 'slide-visuals', zIndex: 2, paintOrder: 5, kind: 'raster', phase: 'page-visual',
      bbox: { x: 0, y: 0, w: 13.333, h: 7.5 }, html: '<html>visual</html>', diagnostics: [],
    },
    fullPageRequest: {
      sourceId: 'slide-1', kind: 'raster', phase: 'full-page',
      bbox: { x: 0, y: 0, w: 13.333, h: 7.5 }, html: '<html>full</html>', diagnostics: [],
    },
  }, async (_html, _index, metadata) => {
    phases.push(metadata.phase);
    if (metadata.phase === 'local-visual') throw new Error('local failed');
    return 'visual-png';
  }, 0);

  assert.deepEqual(phases, ['local-visual', 'page-visual']);
  assert.equal(result.fullPageFallback, null);
  assert.equal(result.layers.length, 1);
  assert.equal(result.layers[0].phase, 'page-visual');
  assert.equal(result.layers[0].data, 'data:image/png;base64,visual-png');
  assert.ok(result.diagnostics.some((diagnostic) => diagnostic.code === 'page_visual_fallback'));
});

test('records every fallback level and keeps all source coverage when local rendering partially fails', async (t) => {
  const localRequests = [
    {
      sourceId: 'gradient', zIndex: 1, paintOrder: 1, kind: 'raster', phase: 'local-visual',
      bbox: { x: 1, y: 1, w: 2, h: 1 }, html: '<html>gradient</html>',
      suppressedNativeVisualIds: ['gradient'], diagnostics: [],
    },
    {
      sourceId: 'filtered', zIndex: 2, paintOrder: 2, kind: 'raster', phase: 'local-visual',
      bbox: { x: 4, y: 1, w: 2, h: 1 }, html: '<html>filtered</html>',
      suppressedNativeVisualIds: ['filtered', 'filtered-image'], diagnostics: [],
    },
  ];
  const pageVisualRequest = {
    sourceId: 'slide-visuals',
    sourceIds: ['gradient', 'filtered'],
    suppressedNativeVisualIds: ['gradient', 'filtered', 'filtered-image'],
    zIndex: 1,
    paintOrder: 1,
    kind: 'raster',
    phase: 'page-visual',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
    html: '<html>all unsupported visuals</html>',
    diagnostics: [],
  };
  const fullPageRequest = {
    sourceId: 'slide-4',
    zIndex: 0,
    paintOrder: 0,
    kind: 'raster',
    phase: 'full-page',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
    html: '<html>full page</html>',
    diagnostics: [],
  };

  await t.test('page visual replaces the complete unsupported source set after one local failure', async () => {
    const attempts = [];
    const result = await renderRasterFallbackPlan({
      localRequests,
      pageVisualRequest,
      fullPageRequest,
    }, async (_html, _slideIndex, metadata) => {
      attempts.push(`${metadata.phase}:${metadata.sourceId}`);
      if (metadata.sourceId === 'filtered') throw new Error('filtered local failed');
      return Buffer.from(metadata.sourceId).toString('base64');
    }, 3);

    assert.deepEqual(attempts, [
      'local-visual:gradient',
      'local-visual:filtered',
      'page-visual:slide-visuals',
    ]);
    assert.equal(result.blocking, false);
    assert.equal(result.fullPageFallback, null);
    assert.equal(result.layers.length, 1);
    assert.deepEqual(result.layers[0].sourceIds, ['gradient', 'filtered']);
    assert.deepEqual(
      result.layers[0].suppressedNativeVisualIds,
      ['gradient', 'filtered', 'filtered-image'],
    );
    assert.ok(result.diagnostics.some((item) => (
      item.code === 'local_raster_failed'
        && item.sourceId === 'filtered'
        && item.slideNumber === 4
    )));
    assert.ok(result.diagnostics.some((item) => (
      item.code === 'page_visual_fallback'
        && item.sourceId === 'slide-visuals'
        && item.slideNumber === 4
    )));
  });

  await t.test('all failed levels produce located blocking evidence instead of an empty success', async () => {
    const result = await renderRasterFallbackPlan({
      localRequests,
      pageVisualRequest,
      fullPageRequest,
    }, async (_html, _slideIndex, metadata) => {
      throw new Error(`${metadata.phase}:${metadata.sourceId} unavailable`);
    }, 3);

    assert.equal(result.blocking, true);
    assert.deepEqual(result.layers, []);
    assert.equal(result.fullPageFallback, null);
    const byCode = new Map(result.diagnostics.map((item) => [item.code, item]));
    assert.equal(byCode.get('local_raster_failed')?.sourceId, 'filtered');
    assert.equal(byCode.get('page_visual_raster_failed')?.sourceId, 'slide-visuals');
    assert.equal(byCode.get('full_page_raster_failed')?.sourceId, 'slide-4');
    assert.equal(byCode.get('full_page_raster_failed')?.severity, 'blocking');
    assert.equal(byCode.get('full_page_raster_failed')?.slideNumber, 4);
    for (const diagnostic of result.diagnostics) {
      assert.ok(diagnostic.reason, `${diagnostic.code} must retain its renderer failure`);
      assert.ok(diagnostic.sourceId, `${diagnostic.code} must retain its source location`);
      assert.ok(diagnostic.phase, `${diagnostic.code} must retain its fallback phase`);
    }
  });
});

test('builds a transparent page-visual request containing only all unsupported targets', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="one" style="background-image: linear-gradient(red, blue)"><p>One</p></div>
    <div data-pptx-source-id="two" style="filter: blur(2px)"><p>Two</p></div>
    <div data-pptx-source-id="native" style="background: red"><p>Native</p></div>
  `);
  const repair = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const localRequests = buildRasterFallbackRequests(doc, repair.diagnostics);

  const request = buildPageVisualFallbackRequest(doc, localRequests);

  assert.equal(request.phase, 'page-visual');
  assert.equal(request.kind, 'raster');
  assert.deepEqual(request.sourceIds.sort(), ['one', 'two']);
  assert.equal(request.html, undefined);
  assert.equal((request.buildHtml().match(/<[^>]+data-pptx-raster-target="1"/g) || []).length, 2);
  assert.match(request.buildHtml(), /background:\s*transparent\s*!important/i);
  assert.match(request.buildHtml(), /pptx-raster-hide-editable-text/);
});

test('full-page fallback serializes as the only slide object', async () => {
  const calls = [];
  const slide = {
    addText(value, options) { calls.push({ op: 'text', value, options }); },
    addImage(options) { calls.push({ op: 'image', options }); },
    addShape(type, options) { calls.push({ op: 'shape', type, options }); },
  };

  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [{
      type: 'p',
      text: 'Must not duplicate',
      sourceId: 'text',
      kind: 'native',
      zIndex: 1,
      position: { x: 1, y: 1, w: 2, h: 1 },
      style: { fontSize: 18, fontFace: 'Arial', color: '111111', align: 'left' },
    }],
    fallbackLayers: [],
    fullPageFallback: {
      sourceId: 'slide-3',
      kind: 'raster',
      phase: 'full-page',
      data: 'data:image/png;base64,full-page',
    },
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: { line: 'line', rect: 'rect', roundRect: 'roundRect' },
  });

  assert.deepEqual(calls.map((call) => call.op), ['image']);
  assert.deepEqual(
    (({ x, y, w, h }) => ({ x, y, w, h }))(calls[0].options),
    { x: 0, y: 0, w: 13.333, h: 7.5 },
  );
});

test('places an uncropped transparent local fallback canvas at full-slide geometry while retaining source bbox', async () => {
  const calls = [];
  const slide = {
    addImage(options) { calls.push(options); },
    addText() {},
    addShape() {},
  };
  const layer = {
    sourceId: 'filtered-card',
    kind: 'raster',
    phase: 'local-visual',
    canvas: 'full-page',
    zIndex: 2,
    paintOrder: 2,
    bbox: { x: 4, y: 2, w: 2, h: 1 },
    data: 'data:image/png;base64,transparent-page-canvas',
  };

  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [],
    fallbackLayers: [layer],
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: {},
  });

  assert.deepEqual(
    (({ x, y, w, h }) => ({ x, y, w, h }))(calls[0]),
    { x: 0, y: 0, w: 13.333, h: 7.5 },
  );
  assert.deepEqual(layer.bbox, { x: 4, y: 2, w: 2, h: 1 });
});

test('summarizes repairs local SVG PNG full-page fallback and blocking diagnostics by slide and source', () => {
  const summary = summarizePptxExportDiagnostics([
    {
      index: 0,
      slideData: {
        diagnostics: [
          { severity: 'repaired', code: 'manual_bullet_list', sourceId: 'list' },
          { severity: 'fallback', code: 'page_visual_fallback', sourceId: 'slide-visuals', phase: 'page-visual' },
        ],
        fallbackLayers: [
          { kind: 'svg-image', sourceId: 'curve', phase: 'local-svg' },
          { kind: 'raster', sourceId: 'gradient', phase: 'local-visual' },
          { kind: 'raster', sourceId: 'slide-visuals', phase: 'page-visual' },
        ],
      },
    },
    {
      index: 2,
      slideData: {
        diagnostics: [
          {
            severity: 'fallback', code: 'full_page_fallback', sourceId: 'slide-3',
            phase: 'full-page', reason: 'local and page visual failed',
          },
          {
            severity: 'blocking', code: 'pptx_serialization', sourceId: 'bad-shape',
            reason: 'serialization failed',
          },
        ],
        fallbackLayers: [],
        fullPageFallback: { kind: 'raster', sourceId: 'slide-3', phase: 'full-page' },
      },
    },
  ]);

  assert.deepEqual(summary.counts, {
    repaired: 1,
    svgImage: 1,
    localPng: 1,
    pageVisual: 1,
    fullPage: 1,
    blocking: 1,
  });
  assert.ok(summary.locations.some((item) => (
    item.slideNumber === 1 && item.sourceId === 'curve' && item.phase === 'local-svg'
  )));
  assert.ok(summary.locations.some((item) => (
    item.slideNumber === 3 && item.sourceId === 'slide-3' && item.phase === 'full-page'
  )));
  assert.ok(summary.locations.some((item) => (
    item.slideNumber === 3 && item.sourceId === 'bad-shape' && item.severity === 'blocking'
  )));
  assert.equal(summary.hasWarnings, true);
  assert.equal(summary.hasBlocking, true);
});

test('localized export diagnostic labels cover every summarized count and location', () => {
  const requiredKeys = [
    'exportDiagnosticsSummary',
    'exportDiagnosticsRepaired',
    'exportDiagnosticsSvg',
    'exportDiagnosticsLocalPng',
    'exportDiagnosticsPageVisual',
    'exportDiagnosticsFullPage',
    'exportDiagnosticsBlocking',
    'exportDiagnosticsLocation',
  ];
  for (const locale of ['en-US', 'zh-CN']) {
    requiredKeys.forEach((key) => assert.ok(STRINGS[locale][key], `${locale}:${key}`));
  }
});

test('localizes known diagnostics and safely redacts unknown low-level reasons', () => {
  assert.equal(
    formatLocalizedExportDiagnostic({ code: 'canvas_overflow' }, 'zh-CN').reason,
    '页面内容超出幻灯片边界，已切换视觉兜底。',
  );
  assert.equal(
    formatLocalizedExportDiagnostic({ code: 'canvas_overflow' }, 'en-US').reason,
    'Slide content exceeded the canvas; visual fallback was used.',
  );
  const unknown = formatLocalizedExportDiagnostic({
    code: 'vendor_failure',
    reason: 'Failed at /Users/alice/private/file.html\nhttps://secret.invalid <script>alert(1)</script>',
    sourceId: '../../secret/<img>',
  }, 'en-US');
  assert.equal(unknown.reason, 'Export encountered a protected internal error.');
  assert.doesNotMatch(JSON.stringify(unknown), /Users|https?:|script|[<>/]|\.\./i);
  assert.equal(unknown.sourceId, sanitizeDiagnosticSourceId('../../secret/<img>'));
  assert.ok(unknown.reason.length <= 120);
});

test('routes filter mask and foreignObject SVG visuals to local PNG without dropping editable text', () => {
  const doc = createDocument(`
    <svg data-pptx-source-id="unsafe-svg" viewBox="0 0 100 100">
      <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
      <path data-pptx-source-id="filtered-path" d="M0 0L90 90" filter="url(#blur)"/>
      <mask id="mask"><rect width="100" height="100" fill="white"/></mask>
      <foreignObject x="5" y="5" width="40" height="20"><div>Foreign visual</div></foreignObject>
      <text data-pptx-source-id="unsafe-label" x="10" y="90">Editable label</text>
    </svg>
    <svg data-pptx-source-id="unsafe-reference" viewBox="0 0 100 100">
      <use href="https://example.invalid/external.svg#shape"/>
    </svg>
  `);
  const repair = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);
  const diagnostics = [...repair.diagnostics, ...slideData.diagnostics];
  const request = buildRasterFallbackRequests(doc, diagnostics)
    .find((item) => item.sourceId === 'unsafe-svg');
  const unsafeReferenceRequest = buildRasterFallbackRequests(doc, diagnostics)
    .find((item) => item.sourceId === 'unsafe-reference');

  assert.ok(diagnostics.some((item) => item.code === 'complex_svg_raster'));
  assert.equal(request?.captureStrategy, 'visual-subtree');
  assert.ok(request?.suppressedNativeVisualIds.includes('unsafe-svg'));
  assert.ok(request?.suppressedNativeVisualIds.includes('filtered-path'));
  assert.equal(unsafeReferenceRequest?.captureStrategy, 'visual-subtree');
  assert.ok(slideData.elements.some((element) => (
    element.type === 'svg-text' && element.text === 'Editable label'
  )));
});

test('assigns decoration subtree and pseudo capture strategies with exact native suppression scopes', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="gradient" style="background-image:linear-gradient(red,blue)">
      <img data-pptx-source-id="gradient-child" src="data:image/png;base64,AA=="/>
      <p data-pptx-source-id="gradient-text">Gradient text</p>
    </div>
    <div data-pptx-source-id="filtered" style="filter:blur(2px)">
      <img data-pptx-source-id="filtered-child" src="data:image/png;base64,AA=="/>
      <p data-pptx-source-id="filtered-text">Filtered text</p>
    </div>
    <div data-pptx-source-id="pseudo"><p>Pseudo owner</p></div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const requests = buildRasterFallbackRequests(doc, [
    { severity: 'fallback', code: 'css_gradient', sourceId: 'gradient' },
    { severity: 'fallback', code: 'css_filter', sourceId: 'filtered' },
    { severity: 'fallback', code: 'generated_content', sourceId: 'pseudo' },
  ]);
  const bySource = new Map(requests.map((request) => [request.sourceId, request]));

  assert.equal(bySource.get('gradient').captureStrategy, 'self-decoration');
  assert.deepEqual(bySource.get('gradient').suppressedNativeVisualIds, ['gradient']);
  assert.equal(bySource.get('gradient').html, undefined);
  assert.match(bySource.get('gradient').buildHtml(), /data-pptx-capture-strategy="self-decoration"/);
  assert.match(bySource.get('gradient').buildHtml(), /self-decoration[^}]*>\s*\*/s);

  assert.equal(bySource.get('filtered').captureStrategy, 'visual-subtree');
  assert.ok(bySource.get('filtered').suppressedNativeVisualIds.includes('filtered-child'));
  assert.ok(bySource.get('filtered').suppressedNativeVisualIds.includes('filtered'));
  assert.match(bySource.get('filtered').buildHtml(), /data-pptx-capture-strategy="visual-subtree"/);

  assert.equal(bySource.get('pseudo').captureStrategy, 'pseudo-only');
  assert.deepEqual(bySource.get('pseudo').suppressedNativeVisualIds, []);
  assert.match(bySource.get('pseudo').buildHtml(), /data-pptx-capture-strategy="pseudo-only"/);
  assert.match(bySource.get('pseudo').buildHtml(), /background:\s*none\s*!important/);
});

test('suppresses duplicated native visuals for raster captures while retaining editable text and unrelated children', async () => {
  const calls = [];
  const slide = {
    addText(value) { calls.push({ op: 'text', value }); },
    addImage(options) { calls.push({ op: 'image', data: options.data, path: options.path }); },
    addShape(type) { calls.push({ op: 'shape', type }); },
  };
  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [
      {
        type: 'shape', sourceId: 'gradient', kind: 'native', zIndex: 1, paintOrder: 1,
        text: '', position: { x: 1, y: 1, w: 3, h: 2 },
        shape: { fill: 'FF0000', line: null, rectRadius: 0 },
      },
      {
        type: 'image', sourceId: 'gradient-child', kind: 'native', zIndex: 1, paintOrder: 2,
        src: 'data:image/png;base64,native-child', position: { x: 1, y: 1, w: 1, h: 1 },
      },
      {
        type: 'image', sourceId: 'filtered-child', kind: 'native', zIndex: 2, paintOrder: 4,
        src: 'data:image/png;base64,duplicate-filtered-child', position: { x: 4, y: 1, w: 1, h: 1 },
      },
      {
        type: 'p', sourceId: 'filtered-text', kind: 'native', zIndex: 2, paintOrder: 5,
        text: 'Editable filtered text', position: { x: 4, y: 1, w: 2, h: 1 },
        style: { fontSize: 16, fontFace: 'Arial', color: '111111', align: 'left' },
      },
    ],
    fallbackLayers: [
      {
        sourceId: 'gradient', kind: 'raster', phase: 'local-visual', zIndex: 1, paintOrder: 0,
        captureStrategy: 'self-decoration', suppressedNativeVisualIds: ['gradient'],
        canvas: 'full-page', bbox: { x: 1, y: 1, w: 3, h: 2 },
        data: 'data:image/png;base64,gradient-layer',
      },
      {
        sourceId: 'filtered', kind: 'raster', phase: 'local-visual', zIndex: 2, paintOrder: 3,
        captureStrategy: 'visual-subtree',
        suppressedNativeVisualIds: ['filtered', 'filtered-child', 'filtered-text'],
        canvas: 'full-page', bbox: { x: 4, y: 1, w: 3, h: 2 },
        data: 'data:image/png;base64,filter-layer',
      },
    ],
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: { rect: 'rect', roundRect: 'roundRect', line: 'line' },
  });

  assert.deepEqual(calls.map((call) => call.op), ['image', 'image', 'image', 'text']);
  assert.ok(calls.some((call) => call.data?.includes('gradient-layer')));
  assert.ok(calls.some((call) => call.data?.includes('native-child')));
  assert.ok(calls.some((call) => call.data?.includes('filter-layer')));
  assert.ok(!calls.some((call) => call.data?.includes('duplicate-filtered-child')));
  assert.equal(calls.filter((call) => call.value === 'Editable filtered text').length, 1);
});

test('preserves actual SVG DOM paint order between paths and native primitives', () => {
  const extractOrder = (markup) => {
    const doc = createDocument(markup);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    const slideData = extractSlideDataFromDocument(doc);
    return [...slideData.elements, ...(slideData.fallbackLayers || [])]
      .filter((item) => ['ordered-path', 'ordered-rect'].includes(item.sourceId))
      .sort((left, right) => left.paintOrder - right.paintOrder)
      .map((item) => item.sourceId);
  };

  assert.deepEqual(extractOrder(`
    <svg viewBox="0 0 100 100">
      <path data-pptx-source-id="ordered-path" d="M0 0L100 100"/>
      <rect data-pptx-source-id="ordered-rect" x="10" y="10" width="20" height="20"/>
    </svg>
  `), ['ordered-path', 'ordered-rect']);
  assert.deepEqual(extractOrder(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="ordered-rect" x="10" y="10" width="20" height="20"/>
      <path data-pptx-source-id="ordered-path" d="M0 0L100 100"/>
    </svg>
  `), ['ordered-rect', 'ordered-path']);
});

test('uses attribute and computed CSS SVG transforms and falls back when transform cannot be represented', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="attr-transform" x="10" y="10" width="20" height="10"
        transform="translate(5 10) scale(2)"/>
      <rect data-pptx-source-id="css-transform" x="40" y="40" width="20" height="10"
        style="transform:rotate(30deg);transform-origin:50px 45px"/>
      <rect data-pptx-source-id="unsafe-transform" x="70" y="70" width="20" height="10"
        style="transform:skewX(25deg)"/>
      <rect data-pptx-source-id="ctm-transform" x="10" y="10" width="10" height="10"
        transform="translate(1 1)"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  doc.querySelector('[data-pptx-source-id="ctm-transform"]').getCTM = () => ({
    a: 1, b: 0, c: 0, d: 1, e: 30, f: 10,
  });
  const slideData = extractSlideDataFromDocument(doc);
  const attr = slideData.elements.find((item) => item.sourceId === 'attr-transform');
  const css = slideData.elements.find((item) => item.sourceId === 'css-transform');
  const unsafeNative = slideData.elements.find((item) => item.sourceId === 'unsafe-transform');
  const unsafeFallback = slideData.fallbackLayers?.find((item) => item.sourceId === 'unsafe-transform');
  const ctm = slideData.elements.find((item) => item.sourceId === 'ctm-transform');

  assert.equal(attr?.kind, 'native');
  assert.ok(attr.bbox.w > 0 && attr.bbox.h > 0);
  assert.equal(css?.kind, 'native');
  assert.equal(Number(css?.shape?.rotate?.toFixed(1)), 30);
  assert.ok(css.bbox.w > css.position.w);
  assert.equal(unsafeNative, undefined);
  assert.equal(unsafeFallback?.kind, 'svg-image');
  const svgRect = doc.querySelector('svg').getBoundingClientRect();
  assert.equal(Number(ctm?.bbox?.x.toFixed(4)), Number(((svgRect.left + 40) / 96).toFixed(4)));
  assert.equal(Number(ctm?.bbox?.y.toFixed(4)), Number(((svgRect.top + 20) / 96).toFixed(4)));
});

test('attaches bbox sourceId zIndex and native kind metadata to every native object', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="box" style="background:rgb(1,2,3);z-index:2">
      <p data-pptx-source-id="copy">Editable copy</p>
      <img data-pptx-source-id="photo-meta" src="data:image/png;base64,AA=="/>
    </div>
    <svg viewBox="0 0 100 100"><line data-pptx-source-id="line-meta" x1="0" y1="0" x2="50" y2="50"/></svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const native = extractSlideDataFromDocument(doc).elements;

  assert.ok(native.length >= 3);
  native.forEach((element) => {
    assert.equal(element.kind, 'native');
    assert.ok(element.sourceId);
    assert.ok(Number.isFinite(element.zIndex));
    assert.ok(element.bbox);
    assert.ok(Number.isFinite(element.bbox.x));
    assert.ok(Number.isFinite(element.bbox.y));
    assert.ok(Number.isFinite(element.bbox.w));
    assert.ok(Number.isFinite(element.bbox.h));
  });
});

test('does not apply viewBox scaling twice to getCTM polygon coordinates', () => {
  const doc = createDocument(`
    <svg data-pptx-source-id="ctm-svg" viewBox="0 0 200 100">
      <polygon data-pptx-source-id="ctm-triangle" points="0,0 20,0 10,10"/>
      <polygon data-pptx-source-id="ctm-diamond" points="10,0 20,10 10,20 0,10"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  for (const id of ['ctm-triangle', 'ctm-diamond']) {
    doc.querySelector(`[data-pptx-source-id="${id}"]`).getCTM = () => ({
      a: 2, b: 0, c: 0, d: 3, e: 30, f: 15,
    });
  }
  const svgRect = doc.querySelector('svg').getBoundingClientRect();
  const slideData = extractSlideDataFromDocument(doc);
  const triangle = slideData.elements.find((item) => item.sourceId === 'ctm-triangle');
  const diamond = slideData.elements.find((item) => item.sourceId === 'ctm-diamond');

  assert.equal(Number(triangle.position.x.toFixed(4)), Number(((svgRect.left + 30) / 96).toFixed(4)));
  assert.equal(Number(triangle.position.w.toFixed(4)), Number((40 / 96).toFixed(4)));
  assert.equal(Number(diamond.position.y.toFixed(4)), Number(((svgRect.top + 15) / 96).toFixed(4)));
  assert.equal(Number(diamond.position.h.toFixed(4)), Number((60 / 96).toFixed(4)));
});

test('shares one full-DOM paint order domain across HTML native and raster fallback siblings', () => {
  const wrappers = Array.from({ length: 30 }, (_, index) => (
    `<div data-pptx-source-id="wrapper-${index}"><span></span></div>`
  )).join('');
  const doc = createDocument(`
    <div data-pptx-source-id="native-before" style="background:rgb(1,2,3)"></div>
    ${wrappers}
    <div data-pptx-source-id="gradient-middle"
      style="background-image:linear-gradient(red,blue)"></div>
    <div data-pptx-source-id="native-after" style="background:rgb(4,5,6)"></div>
  `);
  const repair = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);
  const raster = buildRasterFallbackRequests(doc, repair.diagnostics)
    .find((item) => item.sourceId === 'gradient-middle');
  const before = slideData.elements.find((item) => item.sourceId === 'native-before');
  const after = slideData.elements.find((item) => item.sourceId === 'native-after');

  assert.ok(before.paintOrder < raster.paintOrder);
  assert.ok(raster.paintOrder < after.paintOrder);
  assert.equal(before.subOrder, 0);
  assert.equal(raster.subOrder, 0);
  assert.equal(after.subOrder, 0);
});

test('orders decomposed objects by shared paintOrder and explicit subOrder in Stage 2', async () => {
  const calls = [];
  const slide = {
    addText() {},
    addImage(options) { calls.push(options.data); },
    addShape(_type, options) { calls.push(options.line?.color); },
  };
  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [
      {
        type: 'line', sourceId: 'poly', kind: 'native', zIndex: 0, paintOrder: 8, subOrder: 2,
        x1: 0, y1: 0, x2: 1, y2: 1, color: '222222', width: 1,
      },
      {
        type: 'line', sourceId: 'poly', kind: 'native', zIndex: 0, paintOrder: 8, subOrder: 1,
        x1: 0, y1: 0, x2: 1, y2: 1, color: '111111', width: 1,
      },
    ],
    fallbackLayers: [{
      sourceId: 'middle', kind: 'svg-image', zIndex: 0, paintOrder: 8, subOrder: 1.5,
      bbox: { x: 0, y: 0, w: 1, h: 1 }, data: 'data:image/svg+xml,mid',
    }],
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, { width: 1280, height: 720, errors: [] }, {
    addSlide: () => slide,
    ShapeType: { line: 'line', rect: 'rect', roundRect: 'roundRect' },
  });

  assert.deepEqual(calls, ['111111', 'data:image/svg+xml,mid', '222222']);
});

test('local SVG image preserves class and inherited styles plus ancestor transforms', () => {
  const doc = createDocument(`
    <svg data-pptx-source-id="styled-root" viewBox="0 0 300 150">
      <defs><linearGradient id="paint"><stop offset="0" stop-color="red"/></linearGradient></defs>
      <g data-pptx-source-id="styled-group" transform="translate(40 20)"
        style="fill:rgb(10,20,30);stroke:rgb(40,50,60);opacity:0.6">
        <path data-pptx-source-id="styled-path" class="accent"
          d="M0 0L80 0L40 60Z"/>
      </g>
    </svg>
  `, '.accent { fill: inherit; stroke-width: 5; }');
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const slideData = extractSlideDataFromDocument(doc);
  const layer = slideData.fallbackLayers.find((item) => item.sourceId === 'styled-path');
  const markup = decodeURIComponent(layer.data.replace('data:image/svg+xml,', ''));

  assert.match(markup, /viewBox="0 0 300 150"/);
  assert.match(markup, /transform="translate\(40 20\)"/);
  assert.match(markup, /fill:\s*rgb\(10,\s*20,\s*30\)/);
  assert.match(markup, /stroke:\s*rgb\(40,\s*50,\s*60\)/);
  assert.match(markup, /stroke-width:\s*5(?:px)?/);
  assert.match(markup, /opacity:\s*0\.6/);
  assert.equal(layer.bbox.w, 640 / 96);
  assert.equal(layer.bbox.h, 30 / 96);
});
