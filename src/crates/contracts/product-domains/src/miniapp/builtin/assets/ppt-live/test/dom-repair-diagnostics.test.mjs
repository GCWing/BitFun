import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

import {
  EditableExportError,
  validateEditableSlideScene,
} from '../src/editable-slide-scene.js';
import { normalizeDocumentToEditableScene } from '../src/editable-slide-normalize.js';
import { extractSlideDataFromDocument } from '../src/html2pptx-dom-core.js';
import { buildSlideFromScene, createPptxDeck } from '../src/pptx-html-build.js';
import { sanitizeSlideDocumentRoot } from '../src/sanitize-slide-html.js';
import {
  HTML_ALLOWED_TAGS,
  SVG_ALLOWED_TAGS,
  isAllowedSanitizedAttribute,
  sanitizeSlideDocument,
} from '../src/sanitize-slide-markup.js';
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

test('editable scene rejects fallback fields with located diagnostics', () => {
  for (const fallbackField of ['rasterBase64', 'renderRaster']) {
    const scene = {
      slideNumber: 3,
      width: 13.333,
      height: 7.5,
      nodes: [],
      [fallbackField]: null,
    };

    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => {
        assert.ok(error instanceof EditableExportError);
        assert.deepEqual(
          {
            slideNumber: error.diagnostic.slideNumber,
            sourceId: error.diagnostic.sourceId,
            code: error.diagnostic.code,
          },
          {
            slideNumber: 3,
            sourceId: 'slide-3',
            code: 'editable_scene_fallback_forbidden',
          },
        );
        return true;
      },
      fallbackField,
    );
  }
});

test('normalizes an allowed hr element into one native editable line', () => {
  const doc = createDocument(`
    <hr data-pptx-source-id="section-rule" style="
      position:absolute;left:96px;top:192px;width:384px;height:0;
      margin:0;border:0;border-top:2px dashed #123456">
  `);
  sanitizeSlideDocumentRoot(doc);
  doc.querySelector('hr').getBoundingClientRect = () => ({
    x: 96, y: 192, left: 96, top: 192, right: 480, bottom: 192,
    width: 384, height: 0,
  });

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 4,
    width: 1280 / 96,
    height: 720 / 96,
  });
  const rules = scene.nodes.filter((node) => node.sourceId === 'section-rule');

  assert.equal(rules.length, 1);
  assert.deepEqual(rules[0], {
    type: 'line',
    sourceId: 'section-rule',
    x1: 1,
    y1: 2,
    x2: 5,
    y2: 2,
    paintOrder: 1,
    subOrder: 0,
    zIndex: 0,
    style: {
      color: '123456',
      width: 1.5,
      dash: 'dash',
      dashType: 'dash',
    },
  });
});

test('blocks SVG defs geometry and local paint-server fills with structured diagnostics', () => {
  const cases = [
    {
      sourceId: 'hidden-def-shape',
      code: 'svg_defs_geometry_unsupported',
      markup: `<svg viewBox="0 0 100 100">
        <defs><rect data-pptx-source-id="hidden-def-shape" width="80" height="80" fill="#fff"/></defs>
      </svg>`,
    },
    {
      sourceId: 'gradient-shape',
      code: 'svg_paint_server_unsupported',
      markup: `<svg viewBox="0 0 100 100">
        <defs><linearGradient id="paint"><stop offset="0" stop-color="#fff"/></linearGradient></defs>
        <rect data-pptx-source-id="gradient-shape" width="80" height="80" fill="url(#paint)"/>
      </svg>`,
    },
    {
      sourceId: 'pattern-line',
      code: 'svg_paint_server_unsupported',
      markup: `<svg viewBox="0 0 100 100">
        <defs><pattern id="paint" width="4" height="4"></pattern></defs>
        <line data-pptx-source-id="pattern-line" x2="80" y2="80" stroke="url(#paint)"/>
      </svg>`,
    },
  ];

  for (const { sourceId, code, markup } of cases) {
    const doc = createDocument(markup);
    sanitizeSlideDocumentRoot(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 9,
        width: 1280 / 96,
        height: 720 / 96,
      }),
      (error) => error instanceof EditableExportError
        && error.slideNumber === 9
        && error.sourceId === sourceId
        && error.code === code,
      sourceId,
    );
  }
});

function createSilentDom(markup, options = {}) {
  return new JSDOM(markup, {
    ...options,
    virtualConsole: new VirtualConsole(),
  });
}

function decodeSvgLayerMarkup(data) {
  const raw = String(data || '');
  const comma = raw.indexOf(',');
  const header = raw.slice(0, comma);
  const payload = raw.slice(comma + 1);
  if (/;base64$/i.test(header)) return Buffer.from(payload, 'base64').toString('utf8');
  return decodeURIComponent(payload);
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

test('normalizes CSS border triangles in all four directions as editable native triangles', () => {
  const directions = [
    ['bottom', 0],
    ['left', 90],
    ['top', 180],
    ['right', 270],
  ];
  const doc = createDocument(directions.map(([side]) => `
    <div data-pptx-source-id="triangle-${side}" style="
      width:0;height:0;
      border-top:20px solid transparent;
      border-right:20px solid transparent;
      border-bottom:20px solid transparent;
      border-left:20px solid transparent;
      border-${side}-color:rgb(255,0,0)">
    </div>
  `).join(''));
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 2, width: 13.333, height: 7.5,
  });

  directions.forEach(([side, rotate]) => {
    const node = scene.nodes.find((item) => item.sourceId === `triangle-${side}`);
    assert.equal(node?.type, 'shape', side);
    assert.equal(node?.shapeType, 'triangle', side);
    assert.equal(node?.style.rotate, rotate, side);
  });
});

test('normalizes rounded SVG rects, anchored text, and dashed lines', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="rounded" x="5" y="5" width="40" height="20"
        rx="3.3333333333333335" ry="3.3333333333333335"/>
      <text data-pptx-source-id="anchored" x="50" y="50" text-anchor="middle">Centered</text>
      <line data-pptx-source-id="dashed" x1="0" y1="80" x2="100" y2="80"
        stroke="#123456" stroke-dasharray="6 3"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  assert.equal(scene.nodes.find((node) => node.sourceId === 'rounded')?.shapeType, 'roundRect');
  assert.equal(scene.nodes.find((node) => node.sourceId === 'anchored')?.style.align, 'center');
  assert.equal(scene.nodes.find((node) => node.sourceId === 'dashed')?.style.dash, 'dash');
});

test('normalizes a DOM table as one native node with measured grid and complete cell formatting', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="quality-table"
      style="font-family:Arial;font-size:16px;border-collapse:collapse">
      <tr>
        <th colspan="2" class="styled-header">
          <span style="color:#ffffff">Head</span><strong style="color:#ffeeaa;font-weight:700">er</strong>
        </th>
        <th style="background:#223344">Metric</th>
      </tr>
      <tr>
        <td style="background:#ffffff">A</td>
        <td colspan="2" style="background:#f0f1f2">Wide value</td>
      </tr>
      <tr>
        <td rowspan="2" style="background:#ddeeff;vertical-align:bottom">Merged</td>
        <td style="background:#ffffff">B1</td>
        <td style="background:#ffffff">C1</td>
      </tr>
      <tr>
        <td style="background:#ffffff">B2</td>
        <td style="background:#ffffff">C2</td>
      </tr>
    </table>
  `, `
    .styled-header {
      background:#112233;
      padding:8px 12px 10px 14px;
      border-top:1px solid #101010;
      border-right:2px solid #202020;
      border-bottom:3px solid #303030;
      border-left:4px solid #404040;
      text-align:center;
      vertical-align:middle;
      font-weight:700;
    }
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  const rows = [...table.rows];
  const cells = rows.map((row) => [...row.cells]);
  table.getBoundingClientRect = () => rect(96, 120, 600, 204);
  rows.forEach((row, index) => {
    const heights = [60, 48, 48, 48];
    const top = 120 + heights.slice(0, index).reduce((sum, value) => sum + value, 0);
    row.getBoundingClientRect = () => rect(96, top, 600, heights[index]);
  });
  [
    [rect(96, 120, 360, 60), rect(456, 120, 240, 60)],
    [rect(96, 180, 120, 48), rect(216, 180, 480, 48)],
    [rect(96, 228, 120, 96), rect(216, 228, 240, 48), rect(456, 228, 240, 48)],
    [rect(216, 276, 240, 48), rect(456, 276, 240, 48)],
  ].forEach((rowRects, rowIndex) => {
    rowRects.forEach((cellRect, cellIndex) => {
      cells[rowIndex][cellIndex].getBoundingClientRect = () => cellRect;
    });
  });

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 3, width: 13.333, height: 7.5,
  });
  const tables = scene.nodes.filter((node) => node.type === 'table');
  assert.equal(tables.length, 1);
  const node = tables[0];
  assert.equal(node.sourceId, 'quality-table');
  assert.deepEqual(node.columnWidths, [1.25, 2.5, 2.5]);
  assert.deepEqual(node.rows.map((row) => row.height), [0.625, 0.5, 0.5, 0.5]);
  assert.equal(node.rows[0].cells[0].colspan, 2);
  assert.equal(node.rows[2].cells[0].rowspan, 2);
  assert.deepEqual(node.rows[0].cells[0].text.map((run) => run.text), ['Head', 'er']);
  assert.deepEqual(node.rows[0].cells[0].text.map((run) => run.options.color), ['FFFFFF', 'FFEEAA']);
  assert.equal(node.rows[0].cells[0].text[1].options.bold, true);
  assert.equal(
    node.rows[0].cells[0].style.fill,
    '112233',
    JSON.stringify(node.rows[0].cells[0]),
  );
  assert.deepEqual(node.rows[0].cells[0].style.border, {
    top: { color: '101010', width: 0.75 },
    right: { color: '202020', width: 1.5 },
    bottom: { color: '303030', width: 2.25 },
    left: { color: '404040', width: 3 },
  });
  assert.deepEqual(node.rows[0].cells[0].style.padding, [
    8 / 96, 12 / 96, 10 / 96, 14 / 96,
  ]);
  assert.equal(node.rows[0].cells[0].style.align, 'center');
  assert.equal(node.rows[0].cells[0].style.valign, 'mid');
  assert.equal(node.rows[0].cells[0].style.fontFamily, 'Arial');
  assert.equal(node.rows[0].cells[0].style.fontSize, 12);
  assert.equal(node.rows[0].cells[0].style.fontColor, 'FFFFFF');
  assert.equal(node.rows[0].cells[0].style.bold, true);
  assert.equal(scene.nodes.some((item) => (
    item.type === 'text' && /Header|Metric|Wide value|Merged|B[12]|C[12]/.test(
      Array.isArray(item.text) ? item.text.map((run) => run.text).join('') : item.text,
    )
  )), false);
});

test('blocks DOM tables whose measured spans cannot form a rectangular grid', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="broken-table">
      <tr><td>A</td><td>B</td><td>C</td></tr>
      <tr><td colspan="2">Only two columns</td></tr>
    </table>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  table.getBoundingClientRect = () => rect(96, 120, 600, 100);
  const rows = [...table.rows];
  rows[0].getBoundingClientRect = () => rect(96, 120, 600, 50);
  rows[1].getBoundingClientRect = () => rect(96, 170, 600, 50);
  [...rows[0].cells].forEach((cell, index) => {
    cell.getBoundingClientRect = () => rect(96 + index * 200, 120, 200, 50);
  });
  rows[1].cells[0].getBoundingClientRect = () => rect(96, 170, 400, 50);

  assert.throws(
    () => normalizeDocumentToEditableScene(doc, {
      slideNumber: 8, width: 13.333, height: 7.5,
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'table_grid_non_rectangular'
      && error.sourceId === 'broken-table',
  );
});

test('preserves transparent table fills and block-level cell text line breaks as styled runs', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="transparent-table">
      <tr><td style="background:transparent">
        <p><span style="color:#aa0000">A</span></p>
        <div><strong style="color:#00aa00;font-weight:700">B</strong><br><em>C</em></div>
      </td></tr>
    </table>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  const row = table.rows[0];
  const cell = row.cells[0];
  table.getBoundingClientRect = () => rect(96, 120, 240, 80);
  row.getBoundingClientRect = () => rect(96, 120, 240, 80);
  cell.getBoundingClientRect = () => rect(96, 120, 240, 80);

  const node = normalizeDocumentToEditableScene(doc, {
    slideNumber: 4, width: 13.333, height: 7.5,
  }).nodes.find((item) => item.type === 'table');

  assert.equal(node.rows[0].cells[0].style.fill, null);
  assert.equal(node.rows[0].cells[0].text.map((run) => run.text).join(''), 'A\nB\nC');
  assert.deepEqual(
    node.rows[0].cells[0].text.filter((run) => run.text !== '\n').map((run) => ({
      text: run.text,
      color: run.options.color,
      bold: run.options.bold,
      italic: run.options.italic,
    })),
    [
      { text: 'A', color: 'AA0000', bold: false, italic: false },
      { text: 'B', color: '00AA00', bold: true, italic: false },
      { text: 'C', color: '000000', bold: false, italic: true },
    ],
  );
});

test('preserves semantic inline spaces and separates both sides of block cell content', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="rich-space-table"><tr><td>
      <span style="color:#aa0000">Hello</span> <strong style="color:#00aa00;font-weight:700">world</strong>
      <div><em style="color:#0000aa">A</em></div>B
    </td></tr></table>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  const row = table.rows[0];
  const cell = row.cells[0];
  table.getBoundingClientRect = () => rect(96, 120, 320, 100);
  row.getBoundingClientRect = () => rect(96, 120, 320, 100);
  cell.getBoundingClientRect = () => rect(96, 120, 320, 100);

  const runs = normalizeDocumentToEditableScene(doc, {
    slideNumber: 4, width: 13.333, height: 7.5,
  }).nodes.find((item) => item.type === 'table').rows[0].cells[0].text;

  assert.equal(runs.map((run) => run.text).join(''), 'Hello world\nA\nB');
  assert.equal(runs[0].text, 'Hello');
  assert.equal(runs[0].options.color, 'AA0000');
  assert.equal(runs.find((run) => run.text === 'world').options.color, '00AA00');
  assert.equal(runs.find((run) => run.text === 'world').options.bold, true);
  assert.equal(runs.find((run) => run.text === 'A').options.color, '0000AA');
  assert.equal(runs[0].text.startsWith('\n'), false);
  assert.equal(runs.at(-1).text.endsWith('\n'), false);
});

test('resolves rowspan zero through the end of its own table row group', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="row-group-table">
      <thead><tr><th>Head A</th><th>Head B</th></tr></thead>
      <tbody>
        <tr><td rowspan="0">Body group</td><td>B1</td></tr>
        <tr><td>B2</td></tr>
        <tr><td>B3</td></tr>
      </tbody>
      <tfoot><tr><td>Foot A</td><td>Foot B</td></tr></tfoot>
    </table>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  const rows = [...table.rows];
  table.getBoundingClientRect = () => rect(96, 100, 400, 200);
  rows.forEach((row, rowIndex) => {
    row.getBoundingClientRect = () => rect(96, 100 + rowIndex * 40, 400, 40);
  });
  [
    [rect(96, 100, 200, 40), rect(296, 100, 200, 40)],
    [rect(96, 140, 200, 120), rect(296, 140, 200, 40)],
    [rect(296, 180, 200, 40)],
    [rect(296, 220, 200, 40)],
    [rect(96, 260, 200, 40), rect(296, 260, 200, 40)],
  ].forEach((rowRects, rowIndex) => {
    rowRects.forEach((cellRect, cellIndex) => {
      rows[rowIndex].cells[cellIndex].getBoundingClientRect = () => cellRect;
    });
  });

  const node = normalizeDocumentToEditableScene(doc, {
    slideNumber: 5, width: 13.333, height: 7.5,
  }).nodes.find((item) => item.type === 'table');

  assert.equal(node.rows[1].cells[0].rowspan, 3);
  assert.equal(node.rows[4].cells.length, 2);
});

test('normalizes the production slide-03 quality table as one native table regression', () => {
  const doc = createDocument(`
    <table data-pptx-source-id="slide-03-quality-table"
      style="width:100%;border-collapse:collapse;font-size:11pt">
      <thead><tr>
        <th style="background:#1E293B;padding:8pt 12pt;border:1pt solid #1E293B;width:14%"><p style="color:#fff;font-weight:700;font-size:11pt;text-align:left">阶段</p></th>
        <th style="background:#1E293B;padding:8pt 12pt;border:1pt solid #1E293B;width:22%"><p style="color:#fff;font-weight:700;font-size:11pt;text-align:left">输入</p></th>
        <th style="background:#1E293B;padding:8pt 12pt;border:1pt solid #1E293B;width:36%"><p style="color:#fff;font-weight:700;font-size:11pt;text-align:left">处理动作</p></th>
        <th style="background:#1E293B;padding:8pt 12pt;border:1pt solid #1E293B;width:28%"><p style="color:#fff;font-weight:700;font-size:11pt;text-align:left">质量门槛</p></th>
      </tr></thead>
      <tbody>
        <tr><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb"><p style="font-weight:600;color:#0f766e;font-size:11pt">采集</p></td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">Kafka / MQTT</td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">拉取与推送双模式接入</td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">完整率 ≥ 99.5%</td></tr>
        <tr><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">清洗</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">原始数据流</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">字段校验、去重、PII 脱敏</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">脏数据率 ≤ 0.3%</td></tr>
        <tr><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">特征</td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">清洗后数据</td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">向量化、时序聚合、维度对齐</td><td style="background:#fff;padding:8pt 12pt;border:1pt solid #e5e7eb">特征覆盖率 ≥ 95%</td></tr>
        <tr><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">推理</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">特征向量</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">模型打分、阈值判定、结果序列化</td><td style="background:#FAFAF7;padding:8pt 12pt;border:1pt solid #e5e7eb">P99 延迟 ≤ 80ms</td></tr>
      </tbody>
    </table>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const rect = (left, top, width, height) => ({
    x: left, y: top, left, top, width, height,
    right: left + width, bottom: top + height,
  });
  const table = doc.querySelector('table');
  const rows = [...table.rows];
  const widths = [156.8, 246.4, 403.2, 313.6];
  table.getBoundingClientRect = () => rect(80, 300, 1120, 220);
  rows.forEach((row, rowIndex) => {
    row.getBoundingClientRect = () => rect(80, 300 + rowIndex * 44, 1120, 44);
    let left = 80;
    [...row.cells].forEach((cell, columnIndex) => {
      const cellLeft = left;
      cell.getBoundingClientRect = () => rect(cellLeft, 300 + rowIndex * 44, widths[columnIndex], 44);
      left += widths[columnIndex];
    });
  });

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 3, width: 13.333, height: 7.5,
  });
  const tables = scene.nodes.filter((node) => node.type === 'table');
  assert.equal(tables.length, 1);
  assert.deepEqual(
    tables[0].columnWidths.map((width) => Number(width.toFixed(6))),
    widths.map((width) => Number((width / 96).toFixed(6))),
  );
  assert.equal(tables[0].rows.length, 5);
  assert.equal(tables[0].rows.flatMap((row) => row.cells).length, 20);
  assert.equal(tables[0].rows[4].cells[3].text.map((run) => run.text).join(''), 'P99 延迟 ≤ 80ms');
  assert.equal(scene.nodes.some((node) => (
    node.type === 'text'
    && String(Array.isArray(node.text) ? node.text.map((run) => run.text).join('') : node.text)
      .includes('P99 延迟 ≤ 80ms')
  )), false);
});

test('samples every supported SVG path command into editable line nodes', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 200 120">
      <path data-pptx-source-id="all-commands" fill="none" stroke="#112233"
        d="M5 5 L20 5 H30 V20 C35 5 45 5 50 20 S65 35 70 20 Q80 5 90 20 T110 20 Z"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 4, width: 13.333, height: 7.5,
  });
  const pathNodes = scene.nodes.filter((node) => node.sourceId === 'all-commands');
  assert.ok(pathNodes.length > 20);
  assert.ok(pathNodes.every((node) => node.type === 'line'));
  assert.ok(pathNodes.every((node) => node.style.color === '112233'));
});

test('rejects absolute and relative SVG arc commands as unsupported paths', () => {
  for (const command of ['A', 'a']) {
    const sourceId = `arc-${command}`;
    const doc = createDocument(`
      <svg viewBox="0 0 100 100">
        <path data-pptx-source-id="${sourceId}" fill="none" stroke="#112233"
          d="M10 10 ${command} 20 20 0 0 1 50 50"/>
      </svg>
    `);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);

    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 4, width: 13.333, height: 7.5,
      }),
      (error) => error instanceof EditableExportError
        && error.code === 'svg_path_command_unsupported'
        && error.sourceId === sourceId,
      command,
    );
  }
});

test('rejects transforms on an SVG path and on its parent group', () => {
  const cases = [
    {
      sourceId: 'transformed-path',
      markup: '<path data-pptx-source-id="transformed-path" transform="translate(5 5)" fill="none" d="M0 0L20 20"/>',
    },
    {
      sourceId: 'transformed-parent',
      markup: '<g transform="translate(5 5)"><path data-pptx-source-id="transformed-parent" fill="none" d="M0 0L20 20"/></g>',
    },
  ];

  for (const { sourceId, markup } of cases) {
    const doc = createDocument(`<svg viewBox="0 0 100 100">${markup}</svg>`);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);

    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 4, width: 13.333, height: 7.5,
      }),
      (error) => error instanceof EditableExportError
        && error.code === 'svg_path_transform_unsupported'
        && error.sourceId === sourceId,
      sourceId,
    );
  }
});

test('rewrites linear gradients into ordered solid editable strips', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="gradient"
      style="background-image:linear-gradient(90deg, #ff0000 0%, #0000ff 100%)"></div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const strips = scene.nodes.filter((node) => node.sourceId === 'gradient');
  assert.ok(strips.length >= 8);
  assert.ok(strips.every((node) => node.type === 'shape' && node.shapeType === 'rect'));
  assert.ok(strips.every((node) => node.rewrite === 'css_gradient'));
  assert.notEqual(strips[0].style.fill, strips.at(-1).style.fill);
  assert.deepEqual(strips.map((node) => node.order), [...strips.keys()]);
});

test('preserves intentional images and emits no fallback or SVG-image nodes', () => {
  const doc = createDocument(`
      <img data-pptx-source-id="photo"
        src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="/>
    <svg viewBox="0 0 100 100">
      <path data-pptx-source-id="route-path" d="M0 0L100 100" fill="none" stroke="black"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  assert.equal(scene.nodes.find((node) => node.sourceId === 'photo')?.intent, 'user-image');
  assert.doesNotMatch(JSON.stringify(scene), /fallback|svg-image|raster|page-visual/);
  assert.equal(validateEditableSlideScene(scene), scene);
});

test('HTML intentional images require inline base64 raster data URLs', () => {
  const invalidSources = [
    'data:image/svg+xml;base64,PHN2Zy8+',
    'data:image/png,not-base64',
    'https://example.invalid/photo.png',
    'file:///tmp/photo.png',
    'blob:unsafe',
  ];
  invalidSources.forEach((src, index) => {
    const doc = createDocument(
      `<img data-pptx-source-id="unsafe-${index}" src="${src}">`,
    );
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 4, width: 13.333, height: 7.5,
      }),
      (error) => error instanceof EditableExportError
        && ['editable_scene_payload_invalid', 'external_resource'].includes(error.code)
        && error.sourceId === `unsafe-${index}`,
    );
  });
});

test('blocks unsupported visuals with located editable export diagnostics', () => {
  const fixtures = [
    ['filtered', '<div data-pptx-source-id="filtered" style="filter:blur(2px)"></div>', 'css_filter'],
    ['masked', '<svg><mask id="m"></mask><rect data-pptx-source-id="masked" mask="url(#m)"/></svg>', 'svg_mask'],
    ['external', '<svg><image data-pptx-source-id="external" href="https://example.invalid/a.png"/></svg>', 'external_resource'],
    ['animated', '<svg><rect data-pptx-source-id="animated"><animate attributeName="x"/></rect></svg>', 'animation_unsupported'],
    ['generated', '<div data-pptx-source-id="generated" class="pseudo"></div>', 'generated_content',
      '.pseudo::before { content:"x"; }'],
  ];

  fixtures.forEach(([sourceId, markup, code, css = '']) => {
    const doc = createDocument(markup, css);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 6, width: 13.333, height: 7.5,
      }),
      (error) => error instanceof EditableExportError
        && error.code === code
        && error.slideNumber === 6
        && error.sourceId === sourceId,
      sourceId,
    );
  });
});

test('blocks closed filled SVG paths instead of converting them to pictures', () => {
  const doc = createDocument(`
    <svg><path data-pptx-source-id="filled-path" d="M0 0L20 0L10 20Z" fill="#ff0000"/></svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  assert.throws(
    () => normalizeDocumentToEditableScene(doc, {
      slideNumber: 3, width: 13.333, height: 7.5,
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'svg_path_fill_unsupported'
      && error.sourceId === 'filled-path',
  );
});

test('maps four SVG triangle orientations and CSS border triangles with exact geometry', () => {
  const doc = createDocument(`
    <svg data-pptx-source-id="triangles" viewBox="0 0 100 100">
      <polygon data-pptx-source-id="up" points="10,20 20,0 30,20" fill="#f00"/>
      <polygon data-pptx-source-id="right" points="32,5 40,10 32,15" fill="#0f0"/>
      <polygon data-pptx-source-id="down" points="50,0 70,0 60,20" fill="#00f"/>
      <polygon data-pptx-source-id="left" points="90,5 82,10 90,15" fill="#ff0"/>
    </svg>
    ${['bottom', 'left', 'top', 'right'].map((side) => `
      <div data-pptx-source-id="css-${side}" style="
        width:0;height:0;border:0 solid transparent;
        border-${side}-width:30px;
        border-${side}-color:#123456;
        ${side === 'top' || side === 'bottom'
          ? 'border-left-width:20px;border-right-width:20px'
          : 'border-top-width:20px;border-bottom-width:20px'}"></div>
    `).join('')}
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const svg = doc.querySelector('svg');
  svg.getBoundingClientRect = () => ({
    x: 96, y: 48, left: 96, top: 48, right: 288, bottom: 240, width: 192, height: 192,
  });
  ['bottom', 'left', 'top', 'right'].forEach((side, index) => {
    const width = side === 'left' || side === 'right' ? 30 : 40;
    const height = side === 'top' || side === 'bottom' ? 30 : 40;
    doc.querySelector(`[data-pptx-source-id="css-${side}"]`).getBoundingClientRect = () => ({
      x: 320, y: 80 + index * 50, left: 320, top: 80 + index * 50,
      right: 320 + width, bottom: 80 + index * 50 + height, width, height,
    });
  });

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 3, width: 13.333, height: 7.5,
  });
  const expectedSvg = {
    up: [0, 0.4, 0.4],
    right: [90, 0.16, 0.2],
    down: [180, 0.4, 0.4],
    left: [270, 0.16, 0.2],
  };
  Object.entries(expectedSvg).forEach(([sourceId, [rotate, w, h]]) => {
    const node = scene.nodes.find((item) => item.sourceId === sourceId);
    assert.equal(node?.style.rotate, rotate, sourceId);
    assert.equal(Number(node?.w.toFixed(6)), Number(w.toFixed(6)), `${sourceId}:w`);
    assert.equal(Number(node?.h.toFixed(6)), Number(h.toFixed(6)), `${sourceId}:h`);
  });
  const expectedCssRotation = { bottom: 0, left: 90, top: 180, right: 270 };
  Object.entries(expectedCssRotation).forEach(([side, rotate], index) => {
    const node = scene.nodes.find((item) => item.sourceId === `css-${side}`);
    assert.equal(node?.style.rotate, rotate, side);
    assert.equal(node?.x, 320 / 96, `${side}:x`);
    assert.equal(node?.y, (80 + index * 50) / 96, `${side}:y`);
    assert.equal(node?.w, (side === 'left' || side === 'right' ? 30 : 40) / 96, `${side}:w`);
    assert.equal(node?.h, (side === 'top' || side === 'bottom' ? 30 : 40) / 96, `${side}:h`);
  });
});

test('blocks extractor diagnostics that have no editable rewrite', () => {
  const filledPolygon = createDocument(`
    <svg viewBox="0 0 100 100">
      <polygon data-pptx-source-id="filled-freeform"
        points="5,5 40,5 45,30 20,45 5,25" fill="#abcdef"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(filledPolygon);
  installMeasurableLayout(filledPolygon);
  assert.throws(
    () => normalizeDocumentToEditableScene(filledPolygon, {
      slideNumber: 2, width: 13.333, height: 7.5,
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'svg_polygon_unsupported'
      && error.sourceId === 'filled-freeform',
  );

  const arbitraryFallback = createDocument(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="skewed" x="5" y="5" width="20" height="10"
        style="transform:skewX(20deg)"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(arbitraryFallback);
  installMeasurableLayout(arbitraryFallback);
  assert.throws(
    () => normalizeDocumentToEditableScene(arbitraryFallback, {
      slideNumber: 5, width: 13.333, height: 7.5,
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'svg_transform_unsupported'
      && error.sourceId === 'skewed',
  );
});

test('projects arbitrary gradient angles, interpolates stops, and paints before child text', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="gradient-180"
      style="z-index:7;background-image:linear-gradient(180deg,#ff0000 0%,#00ff00 25%,#0000ff 100%)">
      <p data-pptx-source-id="gradient-copy">Text above gradient</p>
    </div>
    <div data-pptx-source-id="gradient-diagonal"
      style="background-image:linear-gradient(to top right,#000000,#ffffff)"></div>
    <div data-pptx-source-id="gradient-0"
      style="background-image:linear-gradient(0deg,#ff0000,#0000ff)"></div>
    <div data-pptx-source-id="gradient-90"
      style="background-image:linear-gradient(to right,#ff0000,#0000ff)"></div>
    <div data-pptx-source-id="gradient-270"
      style="background-image:linear-gradient(to left,#ff0000,#0000ff)"></div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const vertical = scene.nodes.filter((node) => node.sourceId === 'gradient-180');
  const diagonal = scene.nodes.filter((node) => node.sourceId === 'gradient-diagonal');
  const copy = scene.nodes.find((node) => node.sourceId === 'gradient-copy');
  assert.equal(vertical[0].style.fill, 'DF2000');
  assert.equal(vertical[4].style.fill, '00F40B');
  assert.equal(vertical.at(-1).style.fill, '000BF4');
  assert.ok(vertical.every((node) => node.zIndex === copy.zIndex));
  assert.ok(vertical.every((node) => node.paintOrder < copy.paintOrder
    || (node.paintOrder === copy.paintOrder && node.subOrder < copy.subOrder)));
  assert.ok(diagonal.length > 16, 'diagonal gradients require a projected editable cell grid');
  const distinctDiagonal = new Set(diagonal.map((node) => node.style.fill));
  assert.ok(distinctDiagonal.size > 16);
  assert.ok(diagonal.every((node) => Number.isFinite(node.paintOrder) && Number.isFinite(node.subOrder)));
  const endpoints = (sourceId) => {
    const cells = scene.nodes.filter((node) => node.sourceId === sourceId);
    return [cells[0].style.fill, cells.at(-1).style.fill];
  };
  assert.deepEqual(endpoints('gradient-0'), ['0800F7', 'F70008']);
  assert.deepEqual(endpoints('gradient-90'), ['F70008', '0800F7']);
  assert.deepEqual(endpoints('gradient-270'), ['0800F7', 'F70008']);
});

test('supports CSS gradient angle units and fail-closes unsupported stops and colors', () => {
  const supported = createDocument(`
    <div data-pptx-source-id="turn" style="background:linear-gradient(.5turn,#f00,#00f)"></div>
    <div data-pptx-source-id="rad" style="background:linear-gradient(3.141592653589793rad,#f00,#00f)"></div>
    <div data-pptx-source-id="grad" style="background:linear-gradient(200grad,#f00,#00f)"></div>
  `);
  sanitizeSlideDocumentRoot(supported);
  installMeasurableLayout(supported);
  const scene = normalizeDocumentToEditableScene(supported, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const endpoints = (sourceId) => scene.nodes
    .filter((node) => node.sourceId === sourceId)
    .map((node) => node.style.fill);
  assert.deepEqual(endpoints('turn'), endpoints('rad'));
  assert.deepEqual(endpoints('rad'), endpoints('grad'));

  const rejected = [
    ['px-stop', 'linear-gradient(90deg,#f00 10px,#00f 100%)'],
    ['em-stop', 'linear-gradient(90deg,#f00 1em,#00f 100%)'],
    ['double-stop', 'linear-gradient(90deg,#f00 10% 20%,#00f)'],
    ['color-hint', 'linear-gradient(90deg,#f00 0%,40%,#00f 100%)'],
    ['unsupported-color', 'linear-gradient(90deg,rebeccapurple,#00f)'],
  ];
  rejected.forEach(([sourceId, background]) => {
    const doc = createDocument(
      `<div data-pptx-source-id="${sourceId}" style="background:${background}"></div>`,
    );
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 8, width: 13.333, height: 7.5,
      }),
      (error) => error.code === 'css_gradient_unsupported' && error.sourceId === sourceId,
      sourceId,
    );
  });
});

test('interpolates gradient alpha into editable cell transparency and rejects unsupported alpha', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="alpha-gradient"
      style="background:linear-gradient(90deg,rgba(255,0,0,0),rgba(0,0,255,1))"></div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const cells = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  }).nodes.filter((node) => node.sourceId === 'alpha-gradient');
  assert.equal(cells[0].style.transparency, 96.875);
  assert.equal(cells.at(-1).style.transparency, 3.125);
  assert.ok(cells.every((cell) => cell.style.fill === '0000FF'));
  assert.ok(cells.every((cell, index) => (
    index === 0 || cell.style.transparency < cells[index - 1].style.transparency
  )));

  const unsupported = createDocument(`
    <div data-pptx-source-id="unsupported-alpha"
      style="--gradient:linear-gradient(90deg,rgba(255,0,0,2turn),#0000ff);
        background:var(--gradient)"></div>
  `);
  sanitizeSlideDocumentRoot(unsupported);
  installMeasurableLayout(unsupported);
  assert.throws(
    () => normalizeDocumentToEditableScene(unsupported, {
      slideNumber: 1, width: 13.333, height: 7.5,
    }),
    (error) => error.code === 'css_gradient_unsupported'
      && error.sourceId === 'unsupported-alpha',
  );
});

test('preserves direct text owned by a gradient element above its background cells', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="gradient-with-copy"
      style="background:linear-gradient(90deg,#111,#999)">Direct gradient copy</div>
  `);
  installMeasurableLayout(doc);
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const nodes = scene.nodes.filter((node) => node.sourceId === 'gradient-with-copy');
  const text = nodes.find((node) => node.type === 'text');
  const cells = nodes.filter((node) => node.type === 'shape');
  assert.equal(text?.text, 'Direct gradient copy');
  assert.ok(cells.length > 0);
  assert.ok(cells.every((cell) => cell.paintOrder < text.paintOrder
    || (cell.paintOrder === text.paintOrder && cell.subOrder < text.subOrder)));
});

test('blocks every drawable filled path including open and multi-subpath forms', () => {
  const cases = [
    ['open-default-fill', 'M0 0L20 20', null],
    ['open-explicit-fill', 'M0 0L20 20', '#ff0000'],
    ['multi-subpath-fill', 'M0 0L20 0Z M30 0L40 10', '#00ff00'],
  ];
  cases.forEach(([sourceId, d, fill]) => {
    const doc = createDocument(`
      <svg><path data-pptx-source-id="${sourceId}" d="${d}"
        ${fill ? `fill="${fill}"` : ''} stroke="#000"/></svg>
    `);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 7, width: 13.333, height: 7.5,
      }),
      (error) => error instanceof EditableExportError
        && error.code === 'svg_path_fill_unsupported'
        && error.sourceId === sourceId,
      sourceId,
    );
  });
});

test('positions SVG text frames from middle and end anchor coordinates', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <text data-pptx-source-id="middle-anchor" x="50" y="40"
        text-anchor="middle">Middle</text>
      <text data-pptx-source-id="end-anchor" x="80" y="70"
        text-anchor="end">End</text>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const middle = scene.nodes.find((node) => node.sourceId === 'middle-anchor');
  const end = scene.nodes.find((node) => node.sourceId === 'end-anchor');
  const anchorX = (40 + 305 + 50 * 0.3) / 96;
  const endAnchorX = (40 + 305 + 80 * 0.3) / 96;
  assert.equal(Number((middle.x + middle.w / 2).toFixed(4)), Number(anchorX.toFixed(4)));
  assert.equal(Number((end.x + end.w).toFixed(4)), Number(endAnchorX.toFixed(4)));
});

test('uses one preserveAspectRatio mapping for SVG shapes paths polygons and text', () => {
  const doc = createDocument(`
    <svg data-pptx-source-id="meet-svg" viewBox="0 0 200 100">
      <rect data-pptx-source-id="meet-rect" x="0" y="0" width="20" height="20"/>
      <path data-pptx-source-id="meet-path" d="M0 0L20 0" fill="none" stroke="#000"/>
      <polygon data-pptx-source-id="meet-triangle" points="90,20 100,0 110,20"/>
      <text data-pptx-source-id="meet-text" x="0" y="20">A</text>
    </svg>
    <svg data-pptx-source-id="none-svg" viewBox="0 0 200 100" preserveAspectRatio="none">
      <rect data-pptx-source-id="none-rect" x="0" y="0" width="200" height="100"/>
    </svg>
    <svg data-pptx-source-id="slice-svg" viewBox="0 0 200 100"
      preserveAspectRatio="xMaxYMin slice">
      <rect data-pptx-source-id="slice-rect" x="100" y="0" width="100" height="100"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  [...doc.querySelectorAll('svg')].forEach((svg, index) => {
    svg.getBoundingClientRect = () => ({
      x: 100, y: 50 + index * 220, left: 100, top: 50 + index * 220,
      right: 300, bottom: 250 + index * 220, width: 200, height: 200,
    });
  });
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const meetRect = scene.nodes.find((node) => node.sourceId === 'meet-rect');
  const meetPath = scene.nodes.find((node) => node.sourceId === 'meet-path');
  const meetTriangle = scene.nodes.find((node) => node.sourceId === 'meet-triangle');
  const meetText = scene.nodes.find((node) => node.sourceId === 'meet-text');
  assert.equal(meetRect.y, 100 / 96);
  assert.equal(meetPath.y1, 100 / 96);
  assert.equal(meetTriangle.y, 100 / 96);
  assert.equal(meetText.y, (100 + 20 - 16) / 96);
  const noneRect = scene.nodes.find((node) => node.sourceId === 'none-rect');
  assert.equal(noneRect.w, 200 / 96);
  assert.equal(noneRect.h, 200 / 96);
  const sliceRect = scene.nodes.find((node) => node.sourceId === 'slice-rect');
  assert.equal(sliceRect.x, 100 / 96);
  assert.equal(sliceRect.w, 200 / 96);
});

test('blocks CSS masks and recursively finds generated content in grouping rules', () => {
  const masked = createDocument(`
    <div data-pptx-source-id="css-mask" style="mask-image:linear-gradient(black,transparent)"></div>
  `);
  sanitizeSlideDocumentRoot(masked);
  installMeasurableLayout(masked);
  assert.throws(
    () => normalizeDocumentToEditableScene(masked, {
      slideNumber: 2, width: 13.333, height: 7.5,
    }),
    (error) => error.code === 'css_mask' && error.sourceId === 'css-mask',
  );

  const generated = createDocument(
    '<div data-pptx-source-id="nested-generated" class="nested"></div>',
    '@media screen { @supports (display:grid) { .nested::after { content:"generated"; } } }',
  );
  sanitizeSlideDocumentRoot(generated);
  installMeasurableLayout(generated);
  assert.throws(
    () => normalizeDocumentToEditableScene(generated, {
      slideNumber: 2, width: 13.333, height: 7.5,
    }),
    (error) => error.code === 'generated_content' && error.sourceId === 'nested-generated',
  );
});

test('parses relative repeated path parameters, preserves S/T controls, adapts curves, and classifies dashes', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 200 120">
      <path data-pptx-source-id="relative-repeat" fill="none" stroke="#111"
        d="m5 5 10 0 0 10 h10 10 v10 10"/>
      <path data-pptx-source-id="smooth-curves" fill="none" stroke="#222"
        d="M0 80 C10 0 20 0 30 80 S50 160 60 80 S80 0 90 80
           Q100 20 110 80 T130 80 T150 80"/>
      <path data-pptx-source-id="flat-curve" fill="none" stroke="#333"
        d="M0 100 C20 100 40 100 60 100"/>
      <line data-pptx-source-id="dash" x1="0" y1="10" x2="40" y2="10"
        stroke="#000" stroke-dasharray="8 4"/>
      <line data-pptx-source-id="dot" x1="0" y1="20" x2="40" y2="20"
        stroke="#000" stroke-dasharray="1 4"/>
      <line data-pptx-source-id="dash-dot" x1="0" y1="30" x2="40" y2="30"
        stroke="#000" stroke-dasharray="8 3 1 3"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  assert.equal(scene.nodes.filter((node) => node.sourceId === 'relative-repeat').length, 6);
  assert.ok(scene.nodes.filter((node) => node.sourceId === 'smooth-curves').length
    > scene.nodes.filter((node) => node.sourceId === 'flat-curve').length);
  assert.equal(scene.nodes.find((node) => node.sourceId === 'dash').style.dash, 'dash');
  assert.equal(scene.nodes.find((node) => node.sourceId === 'dot').style.dash, 'dot');
  assert.equal(scene.nodes.find((node) => node.sourceId === 'dash-dot').style.dash, 'dashDot');
});

test('samples reflected S and T controls at their exact midpoint coordinates', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 200 120">
      <path data-pptx-source-id="reflected-s" fill="none" stroke="#000"
        d="M0 50 C10 0 20 0 30 50 S50 100 60 50"/>
      <path data-pptx-source-id="reflected-t" fill="none" stroke="#000"
        d="M70 50 Q80 0 90 50 T110 50"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const svg = doc.querySelector('svg');
  svg.getBoundingClientRect = () => ({
    x: 0, y: 0, left: 0, top: 0, right: 200, bottom: 120, width: 200, height: 120,
  });
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const hasEndpoint = (sourceId, x, y) => scene.nodes.some((node) => (
    node.sourceId === sourceId
    && Math.abs(node.x2 * 96 - x) < 1e-6
    && Math.abs(node.y2 * 96 - y) < 1e-6
  ));
  assert.equal(hasEndpoint('reflected-s', 45, 87.5), true);
  assert.equal(hasEndpoint('reflected-t', 100, 75), true);
});

test('preserves distinct editable roundRect radii and rejects unequal or out-of-range values', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="radius-eight" x="5" y="5" width="80" height="50"
        rx="8" ry="8" fill="#123456"/>
      <rect data-pptx-source-id="radius-four" x="5" y="60" width="80" height="50"
        rx="4" ry="4" fill="#654321"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  doc.querySelector('svg').getBoundingClientRect = () => ({
    x: 0, y: 0, left: 0, top: 0, right: 100, bottom: 100, width: 100, height: 100,
  });
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const radiusEight = scene.nodes.find((item) => item.sourceId === 'radius-eight');
  const radiusFour = scene.nodes.find((item) => item.sourceId === 'radius-four');
  assert.equal(radiusEight.shapeType, 'roundRect');
  assert.equal(radiusFour.shapeType, 'roundRect');
  assert.equal(radiusEight.style.radius, 8 / 96);
  assert.equal(radiusFour.style.radius, 4 / 96);
  assert.notEqual(radiusEight.style.radius, radiusFour.style.radius);

  const unsupported = [
    ['unequal-radius', '12', '4'],
    ['out-of-range-radius', '26', '26'],
  ];
  unsupported.forEach(([sourceId, rx, ry]) => {
    const invalid = createDocument(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="${sourceId}" x="5" y="5" width="80" height="50"
        rx="${rx}" ry="${ry}" fill="#123456"/>
    </svg>
    `);
    sanitizeSlideDocumentRoot(invalid);
    installMeasurableLayout(invalid);
    assert.throws(
      () => normalizeDocumentToEditableScene(invalid, {
        slideNumber: 1, width: 13.333, height: 7.5,
      }),
      (error) => error.code === 'svg_round_rect_radius_unsupported'
        && error.sourceId === sourceId,
    );
  });
});

test('normalizes the slide-01 roundRect probe without blocking', () => {
  const slide01Probe = createDocument(`
    <svg viewBox="0 0 1280 720">
      <rect data-pptx-source-id="slide-01-card" x="120" y="90" width="80" height="50"
        rx="8" ry="8" fill="#17233c" stroke="#3f67ff"/>
      <text data-pptx-source-id="slide-01-label" x="160" y="122"
        text-anchor="middle">核心模块</text>
    </svg>
  `);
  sanitizeSlideDocumentRoot(slide01Probe);
  installMeasurableLayout(slide01Probe);
  const scene = normalizeDocumentToEditableScene(slide01Probe, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const card = scene.nodes.find((node) => node.sourceId === 'slide-01-card');
  assert.equal(card.shapeType, 'roundRect');
  assert.ok(card.style.radius > 0);
  assert.ok(scene.nodes.some((node) => (
    node.sourceId === 'slide-01-label' && node.type === 'text'
  )));
});

test('blocks nonuniform preserveAspectRatio none for roundRect but permits uniform scaling', () => {
  const createRoundRectDocument = () => {
    const doc = createDocument(`
      <svg viewBox="0 0 100 100" preserveAspectRatio="none">
        <rect data-pptx-source-id="none-roundrect" x="5" y="5" width="80" height="50"
          rx="8" ry="8" fill="#123456"/>
      </svg>
    `);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    return doc;
  };

  const nonuniform = createRoundRectDocument();
  nonuniform.querySelector('svg').getBoundingClientRect = () => ({
    x: 0, y: 0, left: 0, top: 0, right: 200, bottom: 100, width: 200, height: 100,
  });
  assert.throws(
    () => normalizeDocumentToEditableScene(nonuniform, {
      slideNumber: 1, width: 13.333, height: 7.5,
    }),
    (error) => error.code === 'svg_round_rect_radius_unsupported'
      && error.sourceId === 'none-roundrect',
  );

  const uniform = createRoundRectDocument();
  uniform.querySelector('svg').getBoundingClientRect = () => ({
    x: 0, y: 0, left: 0, top: 0, right: 200, bottom: 200, width: 200, height: 200,
  });
  const node = normalizeDocumentToEditableScene(uniform, {
    slideNumber: 1, width: 13.333, height: 7.5,
  }).nodes.find((item) => item.sourceId === 'none-roundrect');
  assert.equal(node.style.radius, 16 / 96);
});

test('rejects asymmetric preset polygons instead of silently standardizing them', () => {
  for (const [sourceId, points] of [
    ['off-center-triangle', '0,0 20,0 3,20'],
    ['asymmetric-diamond', '10,0 20,10 8,22 0,10'],
  ]) {
    const doc = createDocument(`
      <svg viewBox="0 0 100 100">
        <polygon data-pptx-source-id="${sourceId}" points="${points}" fill="#123456"/>
      </svg>
    `);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    assert.throws(
      () => normalizeDocumentToEditableScene(doc, {
        slideNumber: 1, width: 13.333, height: 7.5,
      }),
      (error) => error.code === 'svg_polygon_unsupported' && error.sourceId === sourceId,
      sourceId,
    );
  }
});

test('measures SVG text with geometry APIs and CJK-aware fallback widths', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <text data-pptx-source-id="measured" x="50" y="30" text-anchor="middle">Measured</text>
      <text data-pptx-source-id="cjk" x="50" y="60" text-anchor="middle">中A文B</text>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const svg = doc.querySelector('svg');
  svg.getBoundingClientRect = () => ({
    x: 0, y: 0, left: 0, top: 0, right: 100, bottom: 100, width: 100, height: 100,
  });
  doc.querySelector('[data-pptx-source-id="measured"]').getBBox = () => ({
    x: 35, y: 14, width: 30, height: 16,
  });
  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 1, width: 13.333, height: 7.5,
  });
  const measured = scene.nodes.find((node) => node.sourceId === 'measured');
  const cjk = scene.nodes.find((node) => node.sourceId === 'cjk');
  assert.equal(measured.x, 35 / 96);
  assert.equal(measured.w, 30 / 96);
  assert.equal(cjk.w, (16 * (1 + 0.6 + 1 + 0.6)) / 96);
  assert.ok(Math.abs(cjk.x + cjk.w / 2 - 50 / 96) < 1e-12);
});

test('normalizes the three-slide production visual matrix with the slide-03 arrow pointing right', () => {
  const slides = [
    `<svg viewBox="0 0 50 20"><polygon data-pptx-source-id="slide-01-arrow"
      points="20,15 25,5 30,15" fill="#fff"/></svg>`,
    `<div data-pptx-source-id="slide-02-gradient"
      style="background:linear-gradient(135deg,#111,#555)"></div>`,
    `<svg viewBox="0 0 50 20"><polygon data-pptx-source-id="slide-03-arrow"
      points="32,5 40,10 32,15" fill="#fff"/></svg>`,
  ];
  const scenes = slides.map((markup, index) => {
    const doc = createDocument(markup);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    return normalizeDocumentToEditableScene(doc, {
      slideNumber: index + 1, width: 13.333, height: 7.5,
    });
  });
  assert.equal(scenes[0].nodes.find((node) => node.sourceId === 'slide-01-arrow').style.rotate, 0);
  assert.equal(scenes[2].nodes.find((node) => node.sourceId === 'slide-03-arrow').style.rotate, 90);
  scenes.forEach((scene) => assert.equal(validateEditableSlideScene(scene), scene));
});

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

test('keeps dense list body text authored as li > p wrappers', async () => {
  // Stress-test decks (and the element-model path) wrap every bullet in <p>.
  // sanitize unwraps <li><p> into <ul><p>; empty list payloads previously
  // failed scene validation and degrade deleted the whole UL (all body copy).
  const doc = createDocument(`
    <div class="quad" style="position:absolute;left:40px;top:125px;width:340px;height:200px;background:#1C1C1C;padding:12px">
      <h3 style="color:#7f1d1d">高影响 · 高紧迫</h3>
      <ul>
        <li><p>支付链路抖动必须在 Q1 前根治。</p></li>
        <li><p>多语种架构改造可提升可用性。</p></li>
        <li><p>风控模型升级涉及监管报备。</p></li>
      </ul>
    </div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const slideData = extractSlideDataFromDocument(doc);
  const list = slideData.elements.find((element) => element.type === 'list');
  assert.ok(list, 'post-sanitize ul>p lists must still extract as a list element');
  const listText = list.items.map((run) => run.text).join('');
  assert.match(listText, /支付链路抖动/);
  assert.match(listText, /多语种架构/);
  assert.match(listText, /风控模型升级/);

  const scene = normalizeDocumentToEditableScene(doc, {
    slideNumber: 6,
    width: 1280 / 96,
    height: 720 / 96,
  });
  const sceneText = scene.nodes
    .filter((node) => node.type === 'text')
    .map((node) => (Array.isArray(node.text) ? node.text.map((run) => run.text).join('') : node.text))
    .join('\n');
  assert.match(sceneText, /支付链路抖动/);
  assert.match(sceneText, /多语种架构/);

  const pptx = createPptxDeck({ title: 'li-p-list' });
  await buildSlideFromScene(scene, pptx);
  const base64 = String(await pptx.write({ outputType: 'base64' })).replace(/^data:.*;base64,/, '');
  const requireFromPptxGen = createRequire(import.meta.resolve('pptxgenjs'));
  const JSZip = requireFromPptxGen('jszip');
  const zip = await JSZip.loadAsync(base64, { base64: true });
  const xml = await zip.file('ppt/slides/slide1.xml').async('string');
  assert.match(xml, /支付链路抖动/);
  assert.match(xml, /多语种架构/);
  assert.match(xml, /风控模型升级/);
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

test('mixed unsupported visuals report rewrite or blocking diagnostics only', () => {
  const doc = createDocument(`
    <div id="gradient" style="background-image: linear-gradient(red, blue)">Gradient</div>
    <div id="filter" style="filter: blur(2px)">Filtered</div>
    <svg id="complex-svg"><defs><filter id="blur"></filter></defs><path d="M0 0 L10 10"></path></svg>
    <div id="pseudo" class="with-pseudo">Pseudo</div>
  `, '.with-pseudo::before { content: "prefix"; }');

  const { diagnostics } = sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const extracted = extractSlideDataFromDocument(doc);

  diagnostics.forEach((diagnostic) => {
    assert.notEqual(diagnostic.severity, 'fallback');
  });
  for (const code of ['css_filter']) {
    assert.equal(
      extracted.diagnostics.find((item) => item.code === code)?.severity,
      'blocking',
      code,
    );
  }
  assert.doesNotMatch(JSON.stringify(extracted), /fallbackLayers|svg-image|rasterBase64/);
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
  const scene = {
    slideNumber: 1,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'text',
      text: 'Serializable text',
      x: 1, y: 1, w: 4, h: 1,
      style: { fontSize: 20, fontFace: 'Arial', color: '111111', align: 'left' },
      sourceId: 'source-text',
    }],
  };

  await assert.rejects(
    buildSlideFromScene(scene, { addSlide: () => targetSlide, ShapeType: {} }),
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
  assert.equal(svgElements.find((element) => element.svgType === 'rect')?.shape?.fill, 'ff0000');
  assert.equal(svgElements.find((element) => element.svgType === 'rect')?.shape?.line?.color, '000000');
  assert.equal(svgElements.find((element) => element.svgType === 'circle')?.shape?.fill, '00ff00');
  assert.equal(svgElements.find((element) => element.type === 'line')?.color, '0000ff');
  assert.equal(svgElements.find((element) => element.text === 'SVG label')?.type, 'svg-text');
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

  assert.equal(route.length, 2);
  assert.ok(route.every((element) => element.type === 'line' && element.kind === 'native'));
  assert.equal(triangle?.svgType, 'triangle');
  assert.equal(triangle?.kind, 'native');
  assert.equal(diamond?.svgType, 'diamond');
  assert.equal(freeform.length, 5);
  assert.ok(freeform.every((element) => element.type === 'line' && element.kind === 'native'));
  assert.equal(Object.hasOwn(slideData, 'fallbackLayers'), false);
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

test('simulates the WebKit border-box regression for semantic CSS triangles', () => {
  const cases = [
    { side: 'Bottom', rotate: 0, width: 84, height: 74, cross: ['Left', 'Right'] },
    { side: 'Left', rotate: 90, width: 16, height: 18, cross: ['Top', 'Bottom'] },
    { side: 'Top', rotate: 180, width: 44, height: 28, cross: ['Left', 'Right'] },
    { side: 'Right', rotate: 270, width: 18, height: 20, cross: ['Top', 'Bottom'] },
  ];
  const markup = cases.map(({ side, cross }, index) => `
    <div data-pptx-source-id="webkit-${side.toLowerCase()}" style="
      position:absolute;left:${80 + index * 120}px;top:80px;
      width:0;height:0;box-sizing:border-box;
      border-${cross[0].toLowerCase()}:${index + 9}px solid transparent;
      border-${cross[1].toLowerCase()}:${index + 9}px solid transparent;
      border-${side.toLowerCase()}:${index + 16}px solid rgb(15,118,110)">
    </div>
  `).join('');
  const doc = createDocument(`${markup}
    <div data-pptx-source-id="ordinary-partial" style="
      width:120px;height:40px;
      border-top:2px solid transparent;
      border-bottom:2px solid transparent;
      border-left:4px solid rgb(30,41,59)">Visible content</div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);

  const nativeGetComputedStyle = doc.defaultView.getComputedStyle.bind(doc.defaultView);
  const webkitSizes = new Map(cases.map(({ side, width, height }) => (
    [`webkit-${side.toLowerCase()}`, { width, height }]
  )));
  doc.defaultView.getComputedStyle = (element, pseudoElement) => {
    const computed = nativeGetComputedStyle(element, pseudoElement);
    const size = webkitSizes.get(element.dataset?.pptxSourceId);
    if (!size) return computed;
    return new Proxy(computed, {
      get(target, property) {
        if (property === 'width') return `${size.width}px`;
        if (property === 'height') return `${size.height}px`;
        if (property === 'boxSizing') return 'border-box';
        return Reflect.get(target, property, target);
      },
    });
  };
  cases.forEach(({ side, width, height }, index) => {
    const element = doc.querySelector(`[data-pptx-source-id="webkit-${side.toLowerCase()}"]`);
    element.getBoundingClientRect = () => ({
      x: 80 + index * 120,
      y: 80,
      left: 80 + index * 120,
      top: 80,
      right: 80 + index * 120 + width,
      bottom: 80 + height,
      width,
      height,
    });
  });

  const slideData = extractSlideDataFromDocument(doc);
  cases.forEach(({ side, rotate, width, height }) => {
    const sourceId = `webkit-${side.toLowerCase()}`;
    const nodes = slideData.elements.filter((element) => element.sourceId === sourceId);
    assert.equal(nodes.length, 1, `${side} must not also emit border lines`);
    assert.equal(nodes[0].svgType, 'triangle', side);
    assert.equal(nodes[0].kind, 'native', side);
    assert.equal(nodes[0].shape.rotate, rotate, side);
    assert.equal(nodes[0].position.w, width / 96, `${side} bbox width`);
    assert.equal(nodes[0].position.h, height / 96, `${side} bbox height`);
  });
  const ordinaryNodes = slideData.elements.filter((element) => (
    element.sourceId === 'ordinary-partial'
  ));
  assert.equal(ordinaryNodes.some((element) => element.svgType === 'triangle'), false);
  assert.equal(ordinaryNodes.filter((element) => element.type === 'line').length, 3);
});

test('localized export diagnostic labels cover strict summarized counts and locations', () => {
  const requiredKeys = [
    'exportDiagnosticsSummary',
    'exportDiagnosticsRepaired',
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
    '页面内容超出可编辑幻灯片边界。',
  );
  assert.equal(
    formatLocalizedExportDiagnostic({ code: 'canvas_overflow' }, 'en-US').reason,
    'Slide content exceeds the editable canvas.',
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

test('preserves actual SVG DOM paint order between paths and native primitives', () => {
  const extractOrder = (markup) => {
    const doc = createDocument(markup);
    sanitizeSlideDocumentRoot(doc);
    installMeasurableLayout(doc);
    const slideData = extractSlideDataFromDocument(doc);
    return slideData.elements
      .filter((item) => item.sourceId === 'ordered-rect' || item.sourceId.startsWith('ordered-path'))
      .sort((left, right) => left.paintOrder - right.paintOrder)
      .map((item) => item.sourceId.startsWith('ordered-path') ? 'ordered-path' : item.sourceId)
      .filter((sourceId, index, items) => sourceId !== items[index - 1]);
  };

  assert.deepEqual(extractOrder(`
    <svg viewBox="0 0 100 100">
      <line data-pptx-source-id="ordered-path" x1="0" y1="0" x2="100" y2="100" stroke="black"/>
      <rect data-pptx-source-id="ordered-rect" x="10" y="10" width="20" height="20"/>
    </svg>
  `), ['ordered-path', 'ordered-rect']);
  assert.deepEqual(extractOrder(`
    <svg viewBox="0 0 100 100">
      <rect data-pptx-source-id="ordered-rect" x="10" y="10" width="20" height="20"/>
      <line data-pptx-source-id="ordered-path" x1="0" y1="0" x2="100" y2="100" stroke="black"/>
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
  const unsafeDiagnostic = slideData.diagnostics.find((item) => (
    item.sourceId === 'unsafe-transform' && item.severity === 'blocking'
  ));
  const ctm = slideData.elements.find((item) => item.sourceId === 'ctm-transform');

  assert.equal(attr?.kind, 'native');
  assert.ok(attr.bbox.w > 0 && attr.bbox.h > 0);
  assert.equal(css?.kind, 'native');
  assert.equal(Number(css?.shape?.rotate?.toFixed(1)), 30);
  assert.ok(css.bbox.w > css.position.w);
  assert.equal(unsafeNative, undefined);
  assert.equal(unsafeDiagnostic?.code, 'svg_transform_unsupported');
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

test('extractor emits no visual layer or image fallback fields', () => {
  const doc = createDocument(`
    <svg viewBox="0 0 100 100">
      <path data-pptx-source-id="route" d="M0 0L100 100" fill="none" stroke="#123456"/>
    </svg>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  const extracted = extractSlideDataFromDocument(doc);

  assert.equal(Object.hasOwn(extracted, 'fallbackLayers'), false);
  assert.doesNotMatch(JSON.stringify(extracted), /svg-image|raster|page-visual|full-page/);
  assert.equal(
    extracted.diagnostics.find((item) => item.code === 'svg_path_rewrite')?.severity,
    'rewrite',
  );
});

test('normalizer blocks unsupported visuals without invoking a page renderer', () => {
  const doc = createDocument(`
    <div data-pptx-source-id="filtered" style="filter:blur(2px)">Filtered</div>
  `);
  sanitizeSlideDocumentRoot(doc);
  installMeasurableLayout(doc);
  let renderPageCalls = 0;

  assert.throws(
    () => normalizeDocumentToEditableScene(doc, {
      slideNumber: 8,
      width: 13.333,
      height: 7.5,
      renderPage: () => { renderPageCalls += 1; },
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'css_filter'
      && error.slideNumber === 8
      && error.sourceId === 'filtered',
  );
  assert.equal(renderPageCalls, 0);
});

test('scene serializer rejects legacy visual metadata before adding slide objects', async () => {
  let objectCalls = 0;
  const scene = {
    slideNumber: 2,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'shape',
      shapeType: 'rect',
      sourceId: 'generated-panel',
      x: 1,
      y: 1,
      w: 2,
      h: 1,
      style: { fill: '112233' },
    }],
    fallbackLayers: [],
  };
  await assert.rejects(
    buildSlideFromScene(scene, {
      addSlide: () => ({
        addShape() { objectCalls += 1; },
      }),
      ShapeType: { rect: 'rect' },
    }),
    (error) => error instanceof EditableExportError
      && error.code === 'editable_scene_fallback_forbidden',
  );
  assert.equal(objectCalls, 0);
});

test('export summaries count rewrites degradations and blocking evidence', () => {
  const summary = summarizePptxExportDiagnostics([{
    slideNumber: 3,
    nodes: [{
      type: 'shape',
      sourceId: 'gradient-strip',
      rewrite: 'css_gradient',
    }],
  }], [{
    slideNumber: 3,
    sourceId: 'shadow-card',
    severity: 'degrade',
    code: 'box_shadow_removed',
  }]);
  assert.deepEqual(summary.counts, { rewritten: 1, blocking: 0, degraded: 1 });
  assert.equal(summary.hasWarnings, true);
  assert.equal(summary.locations[0].severity, 'rewrite');
  assert.equal(summary.locations[1].severity, 'degrade');
  assert.equal(summary.locations[1].code, 'box_shadow_removed');
  assert.equal(Object.hasOwn(summary.counts, 'localPng'), false);
  assert.equal(Object.hasOwn(summary.counts, 'fullPage'), false);
});
