import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

import {
  buildSlideFromExtracted,
  createPptxDeck,
} from '../src/pptx-html-build.js';

const requireFromPptxGen = createRequire(import.meta.resolve('pptxgenjs'));
const JSZip = requireFromPptxGen('jszip');
const requireFromWebUi = createRequire(
  new URL('../../../../../../../../../web-ui/package.json', import.meta.url),
);
const { JSDOM, VirtualConsole } = requireFromWebUi('jsdom');

const PNG_1X1 = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';
const BODY_DIMENSIONS = { width: 1280, height: 720, errors: [] };

function textElement({
  text,
  sourceId,
  zIndex,
  paintOrder,
  x = 1,
  y = 1,
  w = 4,
  h = 0.6,
}) {
  return {
    type: 'p',
    text,
    sourceId,
    kind: 'native',
    zIndex,
    paintOrder,
    position: { x, y, w, h },
    style: {
      fontSize: 20,
      fontFace: 'Arial',
      color: '111111',
      align: 'left',
      lineSpacing: 24,
      margin: 0,
    },
  };
}

async function writeAndOpen(pptx) {
  const output = await pptx.write({ outputType: 'nodebuffer' });
  assert.ok(Buffer.isBuffer(output), 'PptxGenJS 4.0.1 must return a Node Buffer');
  assert.ok(output.length > 0, 'PPTX buffer must not be empty');
  return JSZip.loadAsync(output);
}

async function zipText(zip, path) {
  const entry = zip.file(path);
  assert.ok(entry, `${path} must exist in the PPTX`);
  return entry.async('string');
}

function topLevelSlideObjects(slideXml) {
  return [...slideXml.matchAll(/<p:(sp|pic)>[\s\S]*?<\/p:\1>/g)]
    .map((match) => ({ type: match[1], xml: match[0] }));
}

function pictureExtents(objectXml) {
  const match = objectXml.match(
    /<a:xfrm>[\s\S]*?<a:off x="(\d+)" y="(\d+)"\/>\s*<a:ext cx="(\d+)" cy="(\d+)"\/>/,
  );
  assert.ok(match, 'picture transform must contain offset and extent');
  return {
    x: Number(match[1]),
    y: Number(match[2]),
    cx: Number(match[3]),
    cy: Number(match[4]),
  };
}

function relatedMediaPaths(relsXml) {
  return [...relsXml.matchAll(/Type="[^"]*\/image" Target="\.\.\/media\/([^"]+)"/g)]
    .map((match) => `ppt/media/${match[1]}`);
}

async function withControllableExportDom(run) {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', {
    pretendToBeVisual: true,
    virtualConsole: new VirtualConsole(),
  });
  const { window } = dom;
  const { document } = window;
  const savedGlobals = new Map();
  const globals = {
    window,
    document,
    DOMParser: window.DOMParser,
    Node: window.Node,
    NodeFilter: window.NodeFilter,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  };
  Object.entries(globals).forEach(([key, value]) => {
    savedGlobals.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, {
      configurable: true,
      writable: true,
      value,
    });
  });

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
  const measuredRect = (element) => {
    if (element.classList?.contains('ppt-export-root')
      || element.classList?.contains('ppt-export-body')) {
      return rect(0, 0, 1280, 720);
    }
    const style = element.style || {};
    const order = Number(element.dataset?.layoutOrder || 0);
    return rect(
      parseFloat(style.left) || 40,
      parseFloat(style.top) || (40 + order * 70),
      parseFloat(style.width) || 480,
      parseFloat(style.height) || 48,
    );
  };
  const elementPrototype = window.HTMLElement.prototype;
  const svgPrototype = window.SVGElement.prototype;
  const originalHtmlRect = elementPrototype.getBoundingClientRect;
  const originalSvgRect = svgPrototype.getBoundingClientRect;
  elementPrototype.getBoundingClientRect = function getBoundingClientRect() {
    return measuredRect(this);
  };
  svgPrototype.getBoundingClientRect = function getBoundingClientRect() {
    return measuredRect(this);
  };
  const prototypeDescriptors = {
    offsetWidth: Object.getOwnPropertyDescriptor(elementPrototype, 'offsetWidth'),
    offsetHeight: Object.getOwnPropertyDescriptor(elementPrototype, 'offsetHeight'),
    scrollWidth: Object.getOwnPropertyDescriptor(elementPrototype, 'scrollWidth'),
    scrollHeight: Object.getOwnPropertyDescriptor(elementPrototype, 'scrollHeight'),
  };
  Object.defineProperties(elementPrototype, {
    offsetWidth: { configurable: true, get() { return measuredRect(this).width; } },
    offsetHeight: { configurable: true, get() { return measuredRect(this).height; } },
    scrollWidth: { configurable: true, get() { return measuredRect(this).width; } },
    scrollHeight: { configurable: true, get() { return measuredRect(this).height; } },
  });
  const originalCreateRange = document.createRange.bind(document);
  document.createRange = () => ({
    element: null,
    selectNodeContents(element) {
      this.element = element;
    },
    getBoundingClientRect() {
      return this.element ? measuredRect(this.element) : rect(0, 0, 0, 0);
    },
    detach() {},
  });

  try {
    return await run({ window, document });
  } finally {
    document.createRange = originalCreateRange;
    elementPrototype.getBoundingClientRect = originalHtmlRect;
    svgPrototype.getBoundingClientRect = originalSvgRect;
    Object.entries(prototypeDescriptors).forEach(([key, descriptor]) => {
      if (descriptor) Object.defineProperty(elementPrototype, key, descriptor);
      else delete elementPrototype[key];
    });
    for (const [key, descriptor] of savedGlobals) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
    window.close();
  }
}

test('DOM preparation main path exports editable OOXML plus local visual fallback', async () => {
  await withControllableExportDom(async () => {
    const [deckExportModule, slideExportModule, elementModelModule] = await Promise.all([
      import('../src/export-deck-browser.js'),
      import('../src/export-slide-browser.js'),
      import('../src/element-model-html.js'),
    ]);
    const { exportPptxPrepared } = deckExportModule;
    const { prepareSlidesForPptxExport } = slideExportModule;
    assert.equal(slideExportModule.buildElementSlideHtml, elementModelModule.buildElementSlideHtml);
    const html = `<!doctype html><html><head><style>
      .fixture-pseudo::before { content: "Generated star"; color: red; }
    </style></head><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="panel" data-layout-order="0"
        style="position:absolute;left:30px;top:30px;width:900px;height:600px;background-color:rgb(220,230,240);z-index:0"></div>
      <p data-pptx-source-id="manual-one" data-layout-order="1"
        style="position:absolute;left:80px;top:100px;width:420px;height:40px;z-index:1">• First prepared bullet</p>
      <p data-pptx-source-id="manual-two" data-layout-order="2"
        style="position:absolute;left:80px;top:150px;width:420px;height:40px;z-index:1">• Second <strong>prepared</strong> bullet</p>
      <p data-pptx-source-id="mixed-copy" data-layout-order="3"
        style="position:absolute;left:80px;top:230px;width:520px;height:50px;z-index:2">Editable <strong>mixed</strong> text</p>
      <div data-pptx-source-id="merge-copy" data-pptx-merge="true" data-layout-order="4"
        style="position:absolute;left:80px;top:290px;width:460px;height:80px;z-index:2">
        <p>Merged prepared first</p><p>Merged prepared second</p>
      </div>
      <table data-pptx-source-id="prepared-table" data-layout-order="5"
        style="position:absolute;left:80px;top:390px;width:420px;height:90px;z-index:2">
        <tr><th>Prepared header</th><td>Prepared cell</td></tr>
      </table>
      <svg data-pptx-source-id="basic-svg" data-layout-order="6" viewBox="0 0 100 100"
        style="position:absolute;left:520px;top:80px;width:100px;height:100px;z-index:2">
        <rect data-pptx-source-id="basic-svg-rect" x="5" y="5" width="30" height="20" fill="#00ff00"/>
        <text data-pptx-source-id="basic-svg-text" x="5" y="90">Prepared SVG text</text>
      </svg>
      <img data-pptx-source-id="native-photo" data-layout-order="4" alt="native"
        style="position:absolute;left:650px;top:100px;width:120px;height:120px;z-index:3" src="${PNG_1X1}" />
      <div data-pptx-source-id="gradient-card" data-layout-order="5"
        style="position:absolute;left:620px;top:300px;width:240px;height:140px;background-image:linear-gradient(red,blue);z-index:4"></div>
      <div data-pptx-source-id="filtered-card" data-layout-order="7"
        style="position:absolute;left:900px;top:300px;width:180px;height:120px;filter:blur(2px);z-index:4"></div>
      <svg data-pptx-source-id="complex-svg" data-layout-order="8" viewBox="0 0 100 100"
        style="position:absolute;left:920px;top:100px;width:120px;height:120px;z-index:4">
        <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
        <path data-pptx-source-id="complex-path" d="M0 0L90 90" filter="url(#blur)"/>
        <text data-pptx-source-id="complex-svg-text" x="5" y="90">Prepared complex SVG label</text>
      </svg>
      <div data-pptx-source-id="pseudo-card" class="fixture-pseudo" data-layout-order="9"
        style="position:absolute;left:900px;top:470px;width:180px;height:80px;z-index:4"></div>
      <p data-pptx-source-id="foreground-copy" data-layout-order="6"
        style="position:absolute;left:80px;top:520px;width:500px;height:50px;z-index:5">Foreground prepared text</p>
    </body></html>`;
    const deck = {
      title: 'Prepared DOM integration',
      slides: [{ id: 'slide-dom', title: 'Prepared DOM integration', html }],
    };
    const rasterPhases = [];

    const prepared = await prepareSlidesForPptxExport(deck.slides, {
      renderRaster: async (_rasterHtml, _slideIndex, metadata) => {
        rasterPhases.push(metadata.phase);
        return PNG_1X1.replace(/^data:image\/png;base64,/, '');
      },
    });

    assert.equal(prepared.length, 1);
    assert.ok(rasterPhases.length >= 3);
    assert.ok(rasterPhases.every((phase) => phase === 'local-visual'));
    assert.ok(prepared[0].slideData.elements.some((item) => item.type === 'list'));
    assert.ok(prepared[0].slideData.elements.some((item) => item.type === 'shape'));
    assert.ok(prepared[0].slideData.elements.some((item) => item.type === 'image'));
    assert.ok(prepared[0].slideData.fallbackLayers.some((item) => item.sourceId === 'gradient-card'));
    assert.equal(prepared[0].slideData.fullPageFallback, null);

    const exported = await exportPptxPrepared(deck, prepared);
    const pptxBuffer = Buffer.from(exported.base64, 'base64');
    assert.ok(Buffer.isBuffer(pptxBuffer) && pptxBuffer.length > 0);
    const zip = await JSZip.loadAsync(pptxBuffer);
    const [slideXml, relsXml] = await Promise.all([
      zipText(zip, 'ppt/slides/slide1.xml'),
      zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
    ]);
    const objects = topLevelSlideObjects(slideXml);

    for (const text of [
      'First prepared bullet',
      'Second ',
      'prepared',
      ' bullet',
      'Editable ',
      'mixed',
      ' text',
      'Merged prepared first',
      'Merged prepared second',
      'Prepared header',
      'Prepared cell',
      'Prepared SVG text',
      'Prepared complex SVG label',
      'Foreground prepared text',
    ]) {
      assert.match(slideXml, new RegExp(`<a:t>${text}</a:t>`), text);
    }
    assert.match(slideXml, /<a:buChar char="(?:•|&#x2022;)"\/>/);
    assert.match(slideXml, /<a:prstGeom prst="rect"/);
    assert.ok((slideXml.match(/<p:sp>/g) || []).length >= 4);
    assert.ok((slideXml.match(/<p:pic>/g) || []).length >= 4);
    assert.ok(relatedMediaPaths(relsXml).length >= 4);
    assert.ok(objects.some((item) => item.type === 'pic'));
    assert.ok(objects.findIndex((item) => item.xml.includes('Foreground prepared text'))
      > objects.map((item) => item.type).lastIndexOf('pic'));
    assert.ok(!(objects.length === 1 && objects[0].type === 'pic'));
  });
});

test('prepare main path records security repairs and escalates unsafe geometry to page visual fallback', async () => {
  await withControllableExportDom(async () => {
    const { prepareSlidesForPptxExport } = await import('../src/export-slide-browser.js');
    const rendered = [];
    const html = `<!doctype html><html><body style="width:1280px;height:720px">
      <script>alert(1)</script>
      <div data-pptx-source-id="native-panel" style="position:absolute;left:20px;top:20px;width:200px;height:100px;background:#123456"></div>
      <img data-pptx-source-id="native-image" src="${PNG_1X1}" style="position:absolute;left:240px;top:20px;width:80px;height:80px">
      <p data-pptx-source-id="overflow-copy" style="position:absolute;left:1200px;top:680px;width:200px;height:80px;font-size:24px">Overflow</p>
    </body></html>`;
    const prepared = await prepareSlidesForPptxExport([{ id: 'unsafe', html }], {
      renderRaster: async (rasterHtml, _index, metadata) => {
        rendered.push({ rasterHtml, metadata });
        return PNG_1X1.replace(/^data:image\/png;base64,/, '');
      },
    });

    assert.ok(prepared[0].diagnostics.some((item) => (
      item.code === 'active_content_removed' && item.severity === 'repaired'
    )));
    assert.ok(prepared[0].diagnostics.some((item) => item.code === 'text_out_of_bounds'));
    assert.equal(rendered[0].metadata.phase, 'page-visual');
    assert.doesNotMatch(rendered[0].rasterHtml, /<script|alert\(1\)/i);
    assert.deepEqual(
      prepared[0].slideData.fallbackLayers[0].suppressedNativeVisualIds.sort(),
      ['native-image', 'native-panel'],
    );
  });
});

test('mounted analysis returns blocking evidence for an unreadable document', async () => {
  const { analyzeMountedSlideForPptx } = await import('../src/export-slide-browser.js');
  const result = analyzeMountedSlideForPptx(null, '');
  assert.equal(result.issues[0].code, 'unreadable_document');
  assert.equal(result.issues[0].severity, 'blocking');
});

test('interactive thumbnail and export preview entry points mount only sanitized HTML', async () => {
  await withControllableExportDom(async ({ document }) => {
    const render = await import('../src/render.js');
    const { buildHtmlDeck } = await import('../src/export-html.js');
    const unsafe = `<!doctype html><html><body>
      <p onclick="alert(1)">Safe text</p><img src="https://evil.invalid/a.png">
      <style>div{background:\\75\\72\\6c (javascript:alert(1))}</style>
    </body></html>`;

    const exportStage = render.buildExportPreviewStage(unsafe);
    document.body.append(exportStage);
    const exportHtml = exportStage.querySelector('iframe').srcdoc;

    const thumbContainer = document.createElement('div');
    thumbContainer.innerHTML = render.slideHtml({ id: 'thumb', html: unsafe });
    document.body.append(thumbContainer);
    render.hydrateHtmlSlideIframes(thumbContainer);
    const thumbHtml = thumbContainer.querySelector('iframe').srcdoc;

    const canvas = document.createElement('div');
    canvas.id = 'slideCanvas';
    document.body.append(canvas);
    render.renderSlideCanvas({
      title: 'Preview test',
      outline: ['Preview test'],
      brief: { topic: 'Preview test' },
      slides: [{ id: 'interactive', html: unsafe }],
      activeSlideId: 'interactive',
      selectedElementId: null,
      generation: { phase: 'idle' },
    }, {});
    const interactiveHtml = canvas.querySelector('[data-slide-stage]').shadowRoot.innerHTML;
    const standaloneDeckHtml = buildHtmlDeck({
      title: 'Sanitized deck',
      slides: [{ id: 'standalone', html: unsafe }],
    });

    for (const mounted of [exportHtml, thumbHtml, interactiveHtml, standaloneDeckHtml]) {
      assert.match(mounted, /Safe text/);
      assert.doesNotMatch(mounted, /onclick|evil\.invalid|javascript:|\\75\\72\\6c/i);
    }
  });
});

test('real PPTX keeps editable objects and local fallback in OOXML paint order', async () => {
  const pptx = createPptxDeck({ title: 'OOXML regression' });
  const result = await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [
      {
        type: 'shape',
        text: '',
        sourceId: 'panel',
        kind: 'native',
        zIndex: 0,
        paintOrder: 0,
        position: { x: 0.5, y: 0.5, w: 5, h: 2.5 },
        shape: { fill: 'DDEEFF', line: { color: '224466', width: 1 }, rectRadius: 0 },
      },
      textElement({
        text: [{ text: 'Editable ', options: {} }, { text: 'title', options: { bold: true } }],
        sourceId: 'title',
        zIndex: 1,
        paintOrder: 1,
      }),
      {
        type: 'list',
        sourceId: 'list',
        kind: 'native',
        zIndex: 2,
        paintOrder: 2,
        position: { x: 1, y: 2, w: 4, h: 1.2 },
        items: [
          { text: 'First bullet', options: { bullet: { indent: 18 }, breakLine: true } },
          { text: 'Second bullet', options: { bullet: { indent: 18 } } },
        ],
        style: {
          fontSize: 18,
          fontFace: 'Arial',
          color: '222222',
          align: 'left',
          lineSpacing: 22,
          paraSpaceBefore: 0,
          paraSpaceAfter: 0,
          margin: 0,
        },
      },
      {
        type: 'image',
        src: PNG_1X1,
        sourceId: 'native-image',
        kind: 'native',
        zIndex: 3,
        paintOrder: 3,
        position: { x: 6, y: 1, w: 1, h: 1 },
      },
      textElement({
        text: 'Foreground label',
        sourceId: 'foreground',
        zIndex: 5,
        paintOrder: 5,
        x: 8,
        y: 1,
      }),
    ],
    fallbackLayers: [{
      sourceId: 'gradient-card',
      kind: 'raster',
      phase: 'local-visual',
      zIndex: 4,
      paintOrder: 4,
      bbox: { x: 7, y: 3, w: 2, h: 1 },
      data: PNG_1X1,
      diagnostics: [{ severity: 'fallback', code: 'css_gradient', sourceId: 'gradient-card' }],
    }],
    placeholders: [],
    diagnostics: [{ severity: 'fallback', code: 'css_gradient', sourceId: 'gradient-card' }],
    errors: [],
  }, BODY_DIMENSIONS, pptx);

  assert.equal(result.diagnostics[0].sourceId, 'gradient-card');
  const zip = await writeAndOpen(pptx);
  const [presentationXml, slideXml, relsXml] = await Promise.all([
    zipText(zip, 'ppt/presentation.xml'),
    zipText(zip, 'ppt/slides/slide1.xml'),
    zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
  ]);

  assert.match(presentationXml, /<p:sldSz cx="12192000" cy="6858000"/);
  assert.match(slideXml, /<a:t>Editable <\/a:t>/);
  assert.match(slideXml, /<a:t>title<\/a:t>/);
  assert.match(slideXml, /<a:t>First bullet<\/a:t>/);
  assert.match(slideXml, /<a:t>Second bullet<\/a:t>/);
  assert.match(slideXml, /<a:buChar char="(?:•|&#x2022;)"\/>/);
  assert.match(slideXml, /<a:prstGeom prst="rect"/);

  const objects = topLevelSlideObjects(slideXml);
  assert.deepEqual(objects.map(({ type, xml }) => {
    if (type === 'pic') return 'pic';
    if (xml.includes('Editable ')) return 'editable-title';
    if (xml.includes('First bullet')) return 'editable-list';
    if (xml.includes('Foreground label')) return 'foreground-text';
    return 'basic-shape';
  }), [
    'basic-shape',
    'editable-title',
    'editable-list',
    'pic',
    'pic',
    'foreground-text',
  ]);
  assert.equal((slideXml.match(/<p:sp>/g) || []).length, 4);
  assert.equal((slideXml.match(/<p:pic>/g) || []).length, 2);
  assert.equal((relsXml.match(/relationships\/image"/g) || []).length, 2);
  const mediaPaths = relatedMediaPaths(relsXml);
  assert.equal(mediaPaths.length, 2);
  mediaPaths.forEach((path) => assert.ok(zip.file(path), `${path} must exist`));

  const pictureSizes = objects.filter((object) => object.type === 'pic').map((object) => pictureExtents(object.xml));
  assert.ok(pictureSizes.every(({ cx, cy }) => cx < 12192000 && cy < 6858000));
  assert.ok(!(
    objects.length === 1
    && objects[0].type === 'pic'
  ), 'a normal local fallback must not collapse the slide into one full-page image');
});

test('real PPTX page visual suppresses native visuals but preserves editable text in paint order', async () => {
  const pptx = createPptxDeck({ title: 'Page visual suppression' });
  await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [
      {
        type: 'shape', sourceId: 'panel', kind: 'native', zIndex: 0, paintOrder: 0,
        position: { x: 0.5, y: 0.5, w: 3, h: 2 },
        shape: { fill: '123456', line: null, rectRadius: 0 },
      },
      {
        type: 'image', sourceId: 'photo', kind: 'native', zIndex: 1, paintOrder: 1,
        src: PNG_1X1, position: { x: 4, y: 1, w: 1, h: 1 },
      },
      textElement({
        text: 'Editable text survives',
        sourceId: 'copy',
        zIndex: 3,
        paintOrder: 3,
      }),
    ],
    fallbackLayers: [{
      sourceId: 'slide-visuals',
      kind: 'raster',
      phase: 'page-visual',
      canvas: 'full-page',
      zIndex: 0,
      paintOrder: 0,
      suppressedNativeVisualIds: ['panel', 'photo'],
      bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
      data: PNG_1X1,
    }],
    placeholders: [],
    diagnostics: [],
    errors: [],
  }, BODY_DIMENSIONS, pptx);

  const zip = await writeAndOpen(pptx);
  const [slideXml, relsXml] = await Promise.all([
    zipText(zip, 'ppt/slides/slide1.xml'),
    zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
  ]);
  const objects = topLevelSlideObjects(slideXml);
  assert.deepEqual(objects.map((object) => (
    object.type === 'pic' ? 'page-visual' : 'editable-text'
  )), ['page-visual', 'editable-text']);
  assert.match(slideXml, /Editable text survives/);
  assert.doesNotMatch(slideXml, /123456/);
  assert.equal((relsXml.match(/relationships\/image"/g) || []).length, 1);
});

test('real PPTX full-page final fallback is one located picture and nothing else', async () => {
  const pptx = createPptxDeck({ title: 'Full-page fallback' });
  const diagnostic = {
    severity: 'fallback',
    code: 'full_page_fallback',
    sourceId: 'slide-2',
    slideNumber: 2,
    phase: 'full-page',
    reason: 'local and page visual rendering failed',
  };
  const result = await buildSlideFromExtracted({
    background: { type: 'color', value: 'FFFFFF' },
    elements: [textElement({
      text: 'Must not survive final fallback',
      sourceId: 'hidden-text',
      zIndex: 1,
      paintOrder: 1,
    })],
    fallbackLayers: [{
      sourceId: 'local-layer',
      kind: 'raster',
      zIndex: 0,
      paintOrder: 0,
      bbox: { x: 1, y: 1, w: 2, h: 2 },
      data: PNG_1X1,
    }],
    fullPageFallback: {
      sourceId: 'slide-2',
      kind: 'raster',
      phase: 'full-page',
      data: PNG_1X1,
    },
    placeholders: [],
    diagnostics: [diagnostic],
    errors: [],
  }, BODY_DIMENSIONS, pptx);

  assert.deepEqual(result.diagnostics, [diagnostic]);
  const zip = await writeAndOpen(pptx);
  const [presentationXml, slideXml, relsXml] = await Promise.all([
    zipText(zip, 'ppt/presentation.xml'),
    zipText(zip, 'ppt/slides/slide1.xml'),
    zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
  ]);
  const objects = topLevelSlideObjects(slideXml);

  assert.equal(objects.length, 1);
  assert.equal(objects[0].type, 'pic');
  assert.doesNotMatch(slideXml, /Must not survive final fallback/);
  assert.equal((slideXml.match(/<p:sp>/g) || []).length, 0);
  assert.equal((slideXml.match(/<p:pic>/g) || []).length, 1);
  assert.equal((relsXml.match(/relationships\/image"/g) || []).length, 1);
  const mediaPaths = relatedMediaPaths(relsXml);
  assert.equal(mediaPaths.length, 1);
  assert.ok(zip.file(mediaPaths[0]), `${mediaPaths[0]} must exist`);

  const slideSize = presentationXml.match(/<p:sldSz cx="(\d+)" cy="(\d+)"/);
  assert.ok(slideSize);
  const extent = pictureExtents(objects[0].xml);
  assert.deepEqual({ x: extent.x, y: extent.y }, { x: 0, y: 0 });
  assert.ok(Math.abs(extent.cx - Number(slideSize[1])) < 1000);
  assert.equal(extent.cy, Number(slideSize[2]));
  assert.equal(result.diagnostics[0].sourceId, 'slide-2');
  assert.equal(result.diagnostics[0].slideNumber, 2);
  assert.equal(result.diagnostics[0].phase, 'full-page');
});
