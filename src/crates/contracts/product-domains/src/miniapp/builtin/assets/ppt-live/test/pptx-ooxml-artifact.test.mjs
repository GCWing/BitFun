import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import test from 'node:test';

import {
  EditableExportError,
  validateEditableSlideScene,
} from '../src/editable-slide-scene.js';
import {
  buildSlideFromScene,
  createPptxDeck,
} from '../src/pptx-html-build.js';

const requireFromPptxGen = createRequire(import.meta.resolve('pptxgenjs'));
const JSZip = requireFromPptxGen('jszip');
const requireFromWebUi = createRequire(
  new URL('../../../../../../../../../web-ui/package.json', import.meta.url),
);
const { JSDOM, VirtualConsole } = requireFromWebUi('jsdom');

const PNG_1X1 = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';
const PRODUCTION_FIXTURES = [
  {
    name: 'editable-slide-01.html',
    sha256: 'a6c3462cfb4ec4da8df0bf4b8e462c729a484057dee71aad3be1baa48e189758',
  },
  {
    name: 'editable-slide-02.html',
    sha256: '95d59e268502ba9bdd4c4b008075d45ee95aa27ca209aabc07aaa38fb74a8d6e',
  },
  {
    name: 'editable-slide-03.html',
    sha256: 'f48d20a6e8826993b0374a26903abc2ef8c893ddacb4d1b29a66cc40d48361e4',
  },
];

function fixtureSourceId(html, selector, occurrence = 0) {
  const fixtureDom = new JSDOM(html);
  try {
    const { body } = fixtureDom.window.document;
    const target = body.querySelectorAll(selector)[occurrence];
    assert.ok(target, `fixture selector ${selector}[${occurrence}] must exist`);
    const elements = [body, ...body.querySelectorAll('*')];
    return `pptx-source-${elements.indexOf(target) + 1}`;
  } finally {
    fixtureDom.window.close();
  }
}

function sceneNodesBySource(scene, sourceId, type = null) {
  return scene.nodes.filter((node) => (
    node.sourceId === sourceId && (type == null || node.type === type)
  ));
}

function assertNear(actual, expected, message, tolerance = 1e-6) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${message}: expected ${expected}, received ${actual}`,
  );
}

function assertLineEndpoints(line, expected, message) {
  assertNear(line.x1, expected.x1, `${message}.x1`);
  assertNear(line.y1, expected.y1, `${message}.y1`);
  assertNear(line.x2, expected.x2, `${message}.x2`);
  assertNear(line.y2, expected.y2, `${message}.y2`);
}

function svgPointMapper({
  leftPt, topPt, widthPt, heightPt, viewWidth, viewHeight,
}) {
  const leftPx = leftPt * 4 / 3;
  const topPx = topPt * 4 / 3;
  const widthPx = widthPt * 4 / 3;
  const heightPx = heightPt * 4 / 3;
  const scale = Math.min(widthPx / viewWidth, heightPx / viewHeight);
  const offsetX = (widthPx - viewWidth * scale) / 2;
  const offsetY = (heightPx - viewHeight * scale) / 2;
  return ({ x, y }) => ({
    x: (leftPx + offsetX + x * scale) / 96,
    y: (topPx + offsetY + y * scale) / 96,
  });
}

function editableSceneWith(node) {
  return {
    slideNumber: 2,
    width: 13.333,
    height: 7.5,
    nodes: [node],
  };
}

function editableTableCell(text, overrides = {}) {
  return {
    text,
    style: {
      fill: 'FFFFFF',
      border: { color: '222222', width: 1 },
      align: 'left',
    },
    ...overrides,
  };
}

test('editable scene accepts complete payloads for every native node kind', () => {
  const scene = {
    slideNumber: 2,
    width: 13.333,
    height: 7.5,
    nodes: [
      {
        type: 'text', sourceId: 'title', x: 1, y: 1, w: 4, h: 0.6,
        text: 'Editable title', style: { fontSize: 20 },
      },
      {
        type: 'shape', sourceId: 'panel', x: 0.5, y: 0.5, w: 5, h: 2.5,
        shapeType: 'rect', style: { fill: 'DDEEFF' },
      },
      {
        type: 'line', sourceId: 'route', x1: 1, y1: 1, x2: 2, y2: 1,
        style: { color: '224466', width: 1 },
      },
      {
        type: 'table', sourceId: 'table', x: 1, y: 3, w: 5, h: 2,
        columnWidths: [2, 3],
        rows: [
          { height: 1, cells: [editableTableCell('Header', { colspan: 2 })] },
          { height: 1, cells: [editableTableCell('Label'), editableTableCell('Value')] },
        ],
      },
      {
        type: 'image', sourceId: 'photo-src', x: 7, y: 1, w: 1, h: 1,
        intent: 'user-image', src: PNG_1X1,
      },
      {
        type: 'image', sourceId: 'photo-data', x: 8, y: 1, w: 1, h: 1,
        intent: 'user-image', data: PNG_1X1,
      },
    ],
  };

  assert.equal(validateEditableSlideScene(scene), scene);
});

test('editable scene accepts the locked native shape type whitelist', () => {
  const shapeTypes = [
    'rect', 'roundRect', 'ellipse', 'triangle', 'diamond',
    'rightArrow', 'leftArrow', 'upArrow', 'downArrow',
    'chevron', 'parallelogram', 'trapezoid', 'hexagon',
  ];
  const scene = {
    slideNumber: 2,
    width: 13.333,
    height: 7.5,
    nodes: shapeTypes.map((shapeType, index) => ({
      type: 'shape',
      sourceId: `shape-${index}`,
      x: 1,
      y: 1,
      w: 2,
      h: 1,
      shapeType,
      style: {},
    })),
  };

  assert.equal(validateEditableSlideScene(scene), scene);
});

test('editable scene rejects unknown native shape types', () => {
  assert.throws(
    () => validateEditableSlideScene(editableSceneWith({
      type: 'shape',
      sourceId: 'unsupported-star',
      x: 1,
      y: 1,
      w: 2,
      h: 1,
      shapeType: 'star5',
      style: {},
    })),
    (error) => {
      assert.equal(error.code, 'editable_scene_payload_invalid');
      assert.equal(error.sourceId, 'unsupported-star');
      return true;
    },
  );
});

test('editable scene rejects unknown node kinds with located diagnostics', () => {
  assert.throws(
    () => validateEditableSlideScene({
      slideNumber: 4,
      width: 13.333,
      height: 7.5,
      nodes: [{
        type: 'raster',
        sourceId: 'generated-chart',
        x: 1,
        y: 1,
        w: 4,
        h: 3,
      }],
    }),
    (error) => {
      assert.ok(error instanceof EditableExportError);
      assert.equal(error.diagnostic.slideNumber, 4);
      assert.equal(error.diagnostic.sourceId, 'generated-chart');
      assert.equal(error.diagnostic.code, 'editable_scene_node_type_unsupported');
      assert.equal(error.slideNumber, error.diagnostic.slideNumber);
      assert.equal(error.sourceId, error.diagnostic.sourceId);
      assert.equal(error.code, error.diagnostic.code);
      return true;
    },
  );
});

test('editable scene recursively rejects nested fallback metadata with node ownership', () => {
  const nestedNodes = [
    {
      type: 'shape',
      sourceId: 'nested-shape',
      x: 1,
      y: 1,
      w: 2,
      h: 1,
      shapeType: 'rect',
      style: { effects: [{ kind: 'raster' }] },
    },
    {
      type: 'table',
      sourceId: 'nested-table',
      x: 1,
      y: 1,
      w: 4,
      h: 2,
      columnWidths: [4],
      rows: [{
        cells: [editableTableCell('Blocked', {
          style: {
            fill: 'FFFFFF',
            border: { color: '222222', width: 1 },
            align: 'left',
            layers: [{ [['full', 'Page', 'Fallback'].join('')]: null }],
          },
        })],
      }],
    },
    {
      type: 'image',
      sourceId: 'nested-image',
      x: 1,
      y: 1,
      w: 2,
      h: 1,
      intent: 'user-image',
      data: 'data:image/png;base64,AA==',
      metadata: [{ kind: 'svg-image' }],
    },
  ];

  for (const node of nestedNodes) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith(node)),
      (error) => {
        assert.equal(error.code, 'editable_scene_fallback_forbidden');
        assert.equal(error.sourceId, node.sourceId);
        return true;
      },
      node.sourceId,
    );
  }

  assert.throws(
    () => validateEditableSlideScene({
      slideNumber: 6,
      width: 13.333,
      height: 7.5,
      nodes: [],
      metadata: [{ fallbackLayers: [] }],
    }),
    (error) => {
      assert.equal(error.code, 'editable_scene_fallback_forbidden');
      assert.equal(error.sourceId, 'slide-6');
      return true;
    },
  );
});

test('editable scene rejects node-level fallback and raster metadata', () => {
  const cases = [
    {
      sourceId: 'node-fallback-layers',
      node: {
        type: 'shape', sourceId: 'node-fallback-layers', x: 1, y: 1, w: 2, h: 2,
        shapeType: 'rect', style: {}, fallbackLayers: [],
      },
    },
    {
      sourceId: 'node-capture',
      node: {
        type: 'shape', sourceId: 'node-capture', x: 1, y: 1, w: 2, h: 2,
        shapeType: 'rect', style: {}, captureStrategy: 'visual-subtree',
      },
    },
    {
      sourceId: 'node-phase',
      node: {
        type: 'shape', sourceId: 'node-phase', x: 1, y: 1, w: 2, h: 2,
        shapeType: 'rect', style: {}, phase: 'local-visual',
      },
    },
    {
      sourceId: 'node-canvas',
      node: {
        type: 'shape', sourceId: 'node-canvas', x: 1, y: 1, w: 2, h: 2,
        shapeType: 'rect', style: {}, canvas: 'full-page',
      },
    },
    {
      sourceId: 'node-data',
      node: {
        type: 'shape', sourceId: 'node-data', x: 1, y: 1, w: 2, h: 2,
        shapeType: 'rect', style: {}, data: 'data:image/png;base64,AA==',
      },
    },
  ];

  for (const { sourceId, node } of cases) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith(node)),
      (error) => {
        assert.equal(error.code, 'editable_scene_fallback_forbidden');
        assert.equal(error.sourceId, sourceId);
        return true;
      },
      sourceId,
    );
  }
});

test('editable scene rejects an image disguised as a raster fallback', () => {
  assert.throws(
    () => validateEditableSlideScene(editableSceneWith({
      type: 'image',
      sourceId: 'raster-image',
      x: 1,
      y: 1,
      w: 2,
      h: 2,
      intent: 'user-image',
      data: 'data:image/png;base64,AA==',
      kind: 'raster',
    })),
    (error) => {
      assert.equal(error.code, 'editable_scene_fallback_forbidden');
      assert.equal(error.sourceId, 'raster-image');
      return true;
    },
  );
});

test('editable scene rejects non-finite, zero-length, and negative geometry', () => {
  const invalidNodes = [
    {
      type: 'line', sourceId: 'zero-line', x1: 1, y1: 1, x2: 1, y2: 1,
      style: { color: '111111' },
    },
    {
      type: 'line', sourceId: 'infinite-line', x1: 1, y1: 1, x2: Number.POSITIVE_INFINITY, y2: 2,
      style: { color: '111111' },
    },
    {
      type: 'shape',
      sourceId: 'negative-width',
      x: 1,
      y: 1,
      w: -1,
      h: 2,
      shapeType: 'rect',
      style: {},
    },
    {
      type: 'text', sourceId: 'negative-height', x: 1, y: 1, w: 1, h: -1,
      text: 'Invalid', style: {},
    },
  ];

  for (const node of invalidNodes) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith(node)),
      (error) => {
        assert.equal(error.code, 'editable_scene_geometry_invalid');
        assert.equal(error.sourceId, node.sourceId);
        return true;
      },
      node.sourceId,
    );
  }
});

test('editable scene rejects missing and empty node source ids', () => {
  for (const sourceId of [undefined, '', '   ']) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith({
        type: 'text',
        ...(sourceId === undefined ? {} : { sourceId }),
        x: 1,
        y: 1,
        w: 2,
        h: 1,
        text: 'Located text',
        style: {},
      })),
      (error) => {
        assert.equal(error.code, 'editable_scene_source_id_invalid');
        assert.ok(error.sourceId);
        return true;
      },
    );
  }
});

test('editable scene rejects missing required payload for every node kind', () => {
  const invalidNodes = [
    { type: 'text', sourceId: 'text-payload', x: 1, y: 1, w: 2, h: 1, style: {} },
    {
      type: 'text', sourceId: 'text-style', x: 1, y: 1, w: 2, h: 1,
      text: 'Missing style',
    },
    {
      type: 'shape', sourceId: 'shape-payload', x: 1, y: 1, w: 2, h: 1,
      style: {},
    },
    {
      type: 'shape', sourceId: 'shape-style', x: 1, y: 1, w: 2, h: 1,
      shapeType: 'rect',
    },
    {
      type: 'line', sourceId: 'line-payload', x1: 1, y1: 1, x2: 2, y2: 2,
    },
    {
      type: 'table', sourceId: 'table-payload', x: 1, y: 1, w: 2, h: 1,
      columnWidths: [2], rows: [{ cells: [] }],
    },
    {
      type: 'image', sourceId: 'image-payload', x: 1, y: 1, w: 2, h: 1,
      src: 'assets/photo.png',
    },
    {
      type: 'image', sourceId: 'image-source', x: 1, y: 1, w: 2, h: 1,
      intent: 'user-image',
    },
  ];

  for (const node of invalidNodes) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith(node)),
      (error) => {
        assert.equal(error.code, 'editable_scene_payload_invalid');
        assert.equal(error.sourceId, node.sourceId);
        return true;
      },
      node.sourceId,
    );
  }
});

test('editable scene rejects incomplete table column, cell style, and span contracts', () => {
  const validCell = () => editableTableCell('Value');
  const invalidTables = [
    {
      sourceId: 'table-missing-widths',
      rows: [{ cells: [validCell()] }],
    },
    {
      sourceId: 'table-invalid-width',
      columnWidths: [0],
      rows: [{ cells: [validCell()] }],
    },
    {
      sourceId: 'table-column-mismatch',
      columnWidths: [1, 1],
      rows: [{ cells: [validCell()] }],
    },
    {
      sourceId: 'table-missing-style',
      columnWidths: [1],
      rows: [{ cells: [{ text: 'Value' }] }],
    },
    {
      sourceId: 'table-invalid-fill',
      columnWidths: [1],
      rows: [{ cells: [editableTableCell('Value', {
        style: {
          fill: 'white',
          border: { color: '222222', width: 1 },
          align: 'left',
        },
      })] }],
    },
    {
      sourceId: 'table-invalid-border',
      columnWidths: [1],
      rows: [{ cells: [editableTableCell('Value', {
        style: {
          fill: 'FFFFFF',
          border: { color: '222222', width: -1 },
          align: 'left',
        },
      })] }],
    },
    {
      sourceId: 'table-invalid-align',
      columnWidths: [1],
      rows: [{ cells: [editableTableCell('Value', {
        style: {
          fill: 'FFFFFF',
          border: { color: '222222', width: 1 },
          align: 'justify',
        },
      })] }],
    },
    {
      sourceId: 'table-invalid-rowspan',
      columnWidths: [1],
      rows: [{ cells: [editableTableCell('Value', { rowspan: 0 })] }],
    },
    {
      sourceId: 'table-invalid-colspan',
      columnWidths: [1],
      rows: [{ cells: [editableTableCell('Value', { colspan: 1.5 })] }],
    },
  ];

  for (const { sourceId, ...payload } of invalidTables) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith({
        type: 'table',
        sourceId,
        x: 1,
        y: 1,
        w: 4,
        h: 2,
        ...payload,
      })),
      (error) => {
        assert.equal(error.code, 'editable_scene_payload_invalid');
        assert.equal(error.sourceId, sourceId);
        return true;
      },
      sourceId,
    );
  }
});

test('editable scene accepts transparent table fill and rejects width or height sum mismatches', () => {
  const transparent = editableSceneWith({
    type: 'table',
    sourceId: 'transparent-table',
    x: 1,
    y: 1,
    w: 2,
    h: 1,
    columnWidths: [2],
    rows: [{
      height: 1,
      cells: [editableTableCell('Transparent', {
        style: {
          fill: null,
          border: { color: '222222', width: 1 },
          align: 'left',
        },
      })],
    }],
  });
  assert.equal(validateEditableSlideScene(transparent), transparent);

  const mismatches = [
    {
      sourceId: 'table-width-sum',
      w: 2.2,
      h: 1,
      columnWidths: [1, 1],
      rows: [{
        height: 1,
        cells: [editableTableCell('A'), editableTableCell('B')],
      }],
    },
    {
      sourceId: 'table-height-sum',
      w: 2,
      h: 1.2,
      columnWidths: [1, 1],
      rows: [{
        height: 1,
        cells: [editableTableCell('A'), editableTableCell('B')],
      }],
    },
  ];
  for (const payload of mismatches) {
    assert.throws(
      () => validateEditableSlideScene(editableSceneWith({
        type: 'table',
        x: 1,
        y: 1,
        ...payload,
      })),
      (error) => error.code === 'editable_scene_payload_invalid'
        && error.sourceId === payload.sourceId,
      payload.sourceId,
    );
  }
});

test('serializes an editable table scene node as native a:tbl OOXML without pictures', async () => {
  const pptx = createPptxDeck({ title: 'Native table' });
  const cellStyle = (fill, overrides = {}) => ({
    fill,
    border: {
      top: { color: '111111', width: 0.75 },
      right: { color: '222222', width: 1.5 },
      bottom: { color: '333333', width: 2.25 },
      left: { color: '444444', width: 3 },
    },
    align: 'left',
    valign: 'mid',
    fontFamily: 'Arial',
    fontSize: 12,
    fontColor: '556677',
    bold: false,
    padding: [0.05, 0.1, 0.15, 0.2],
    ...overrides,
  });
  const scene = {
    slideNumber: 3,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'table',
      sourceId: 'native-table',
      x: 1,
      y: 1.5,
      w: 6,
      h: 2,
      columnWidths: [1.25, 2.25, 2.5],
      rows: [
        {
          height: 0.6,
          cells: [{
            text: [
              { text: 'Native ', options: { color: 'FFFFFF' } },
              { text: 'header', options: { bold: true, color: 'FFEEAA' } },
            ],
            colspan: 2,
            style: cellStyle('112233', { align: 'center', bold: true }),
          }, {
            text: 'Metric',
            style: cellStyle('223344', { fontColor: 'FFFFFF', bold: true }),
          }],
        },
        {
          height: 0.7,
          cells: [{
            text: 'Merged',
            rowspan: 2,
            style: cellStyle('DDEEFF'),
          }, {
            text: 'B1',
            style: cellStyle('FFFFFF'),
          }, {
            text: 'C1',
            style: cellStyle('FFFFFF'),
          }],
        },
        {
          height: 0.7,
          cells: [{
            text: 'B2',
            style: cellStyle(null),
          }, {
            text: 'C2',
            style: cellStyle('F5F5F5'),
          }],
        },
      ],
    }],
  };

  await buildSlideFromScene(scene, pptx);
  const zip = await writeAndOpen(pptx);
  const [slideXml, relsXml] = await Promise.all([
    zipText(zip, 'ppt/slides/slide1.xml'),
    zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
  ]);

  assert.match(slideXml, /<a:tbl>/);
  assert.match(slideXml, /gridSpan="2"/);
  assert.match(slideXml, /rowSpan="2"/);
  assert.match(slideXml, /<a:t>Native <\/a:t>/);
  assert.match(slideXml, /<a:t>header<\/a:t>/);
  assert.match(slideXml, /<a:rPr[^>]*b="1"/);
  assert.match(slideXml, /<a:solidFill><a:srgbClr val="112233"\/><\/a:solidFill>/);
  assert.deepEqual(
    [...slideXml.matchAll(/<a:gridCol w="(\d+)"\/>/g)].map((match) => Number(match[1])),
    [1143000, 2057400, 2286000],
  );
  assert.deepEqual(
    [...slideXml.matchAll(/<a:tr h="(\d+)">/g)].map((match) => Number(match[1])),
    [548640, 640080, 640080],
  );
  const tableCells = [...slideXml.matchAll(/<a:tc(?:\s[^>]*)?>[\s\S]*?<\/a:tc>/g)]
    .map((match) => match[0]);
  const headerCellXml = tableCells.find((cellXml) => cellXml.includes('<a:t>Native </a:t>'));
  assert.ok(headerCellXml);
  // OOXML ST_TextAnchoringType uses ctr (not mid); post-process rewrites mid→ctr.
  assert.match(headerCellXml, /<a:tcPr[^>]*marL="182880"[^>]*marR="91440"[^>]*marT="45720"[^>]*marB="137160"[^>]*anchor="ctr"/);
  assert.match(headerCellXml, /<a:lnL w="38100"[\s\S]*?<a:srgbClr val="444444"/);
  assert.match(headerCellXml, /<a:lnR w="19050"[\s\S]*?<a:srgbClr val="222222"/);
  assert.match(headerCellXml, /<a:lnT w="9525"[\s\S]*?<a:srgbClr val="111111"/);
  assert.match(headerCellXml, /<a:lnB w="28575"[\s\S]*?<a:srgbClr val="333333"/);
  assert.match(headerCellXml, /<a:pPr[^>]*algn="ctr"/);
  assert.match(headerCellXml, /<a:rPr[^>]*sz="1200"[^>]*b="1"/);
  assert.match(headerCellXml, /<a:latin typeface="Arial"/);
  const headerRuns = [...headerCellXml.matchAll(/<a:r>[\s\S]*?<\/a:r>/g)]
    .map((match) => match[0]);
  const nativeRunXml = headerRuns.find((runXml) => runXml.includes('<a:t>Native </a:t>'));
  const emphasizedRunXml = headerRuns.find((runXml) => runXml.includes('<a:t>header</a:t>'));
  assert.match(nativeRunXml, /<a:solidFill><a:srgbClr val="FFFFFF"\/><\/a:solidFill>/);
  assert.match(emphasizedRunXml, /<a:solidFill><a:srgbClr val="FFEEAA"\/><\/a:solidFill>/);
  const defaultColorCellXml = tableCells.find((cellXml) => cellXml.includes('<a:t>B1</a:t>'));
  assert.ok(defaultColorCellXml);
  assert.match(defaultColorCellXml, /<a:rPr[^>]*>[\s\S]*?<a:solidFill><a:srgbClr val="556677"\/><\/a:solidFill>/);
  const transparentCellXml = tableCells.find((cellXml) => cellXml.includes('<a:t>B2</a:t>'));
  assert.ok(transparentCellXml);
  assert.match(
    transparentCellXml,
    /<a:noFill\/>|<a:solidFill><a:srgbClr val="[0-9A-F]{6}"><a:alpha val="0"\/><\/a:srgbClr><\/a:solidFill>/,
  );
  assert.doesNotMatch(slideXml, /<p:pic>/);
  assert.doesNotMatch(relsXml, /relationships\/image"/);
});

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

async function withControllableExportDom(
  run,
  { realisticLayout = false, webkitBorderBoxRegressionSimulation = false } = {},
) {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', {
    pretendToBeVisual: true,
    virtualConsole: new VirtualConsole(),
    url: 'https://ppt-live.test/',
  });
  const { window } = dom;
  const { document } = window;
  if (webkitBorderBoxRegressionSimulation) {
    const nativeGetComputedStyle = window.getComputedStyle.bind(window);
    const borderPixels = (value) => {
      const raw = String(value || '').trim();
      const amount = parseFloat(raw) || 0;
      return raw.endsWith('pt') ? amount * 4 / 3 : amount;
    };
    window.getComputedStyle = (element, pseudoElement) => {
      const computed = nativeGetComputedStyle(element, pseudoElement);
      const borderWidth = ['Left', 'Right']
        .reduce((sum, side) => sum + borderPixels(computed[`border${side}Width`]), 0);
      const borderHeight = ['Top', 'Bottom']
        .reduce((sum, side) => sum + borderPixels(computed[`border${side}Width`]), 0);
      const isZeroContentBorderBox = element.tagName === 'DIV'
        && element.style.width !== ''
        && element.style.height !== ''
        && parseFloat(element.style.width) === 0
        && parseFloat(element.style.height) === 0
        && borderWidth > 0
        && borderHeight > 0;
      if (!isZeroContentBorderBox) return computed;
      return new Proxy(computed, {
        get(target, property) {
          if (property === 'width') return `${borderWidth}px`;
          if (property === 'height') return `${borderHeight}px`;
          if (property === 'boxSizing') return 'border-box';
          return Reflect.get(target, property, target);
        },
      });
    };
  }
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
    localStorage: window.localStorage,
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
  const simpleMeasuredRect = (element) => {
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
  const cssPixels = (value, relativeTo = 0) => {
    const raw = String(value || '').trim();
    if (!raw || raw === 'auto' || raw === 'none') return 0;
    const amount = Number.parseFloat(raw);
    if (!Number.isFinite(amount)) return 0;
    if (raw.endsWith('pt')) return amount * 4 / 3;
    if (raw.endsWith('%')) return relativeTo * amount / 100;
    return amount;
  };
  const realisticMeasuredRect = (element) => {
    if (element.classList?.contains('ppt-export-root')
      || element.classList?.contains('ppt-export-body')) {
      return rect(0, 0, 1280, 720);
    }
    const parent = element.parentElement;
    const parentRect = parent ? realisticMeasuredRect(parent) : rect(0, 0, 1280, 720);
    const style = element.style || {};
    const authoredStyle = element.dataset?.pptxAuthoredStyle || '';
    const authoredValue = (property) => {
      const match = authoredStyle.match(new RegExp(`(?:^|;)\\s*${property}\\s*:\\s*([^;]+)`, 'i'));
      return match?.[1]?.trim() || '';
    };
    const layoutValue = (property, cssProperty = property) => (
      style[property] || authoredValue(cssProperty)
    );
    const computed = window.getComputedStyle(element);
    const tag = element.tagName;
    const table = element.closest?.('table');
    if (tag === 'TABLE') {
      const rowCount = Math.max(1, element.rows.length);
      const width = cssPixels(layoutValue('width'), parentRect.width) || parentRect.width;
      return rect(parentRect.left, parentRect.top, width, rowCount * 40);
    }
    if (tag === 'THEAD' || tag === 'TBODY' || tag === 'TFOOT') {
      const rowsBefore = [...table.rows].findIndex((row) => row.parentElement === element);
      return rect(parentRect.left, realisticMeasuredRect(table).top + Math.max(0, rowsBefore) * 40,
        realisticMeasuredRect(table).width, element.rows.length * 40);
    }
    if (tag === 'TR') {
      const tableRect = realisticMeasuredRect(table);
      return rect(tableRect.left, tableRect.top + [...table.rows].indexOf(element) * 40,
        tableRect.width, 40);
    }
    if (tag === 'TD' || tag === 'TH') {
      const tableRect = realisticMeasuredRect(table);
      const row = element.parentElement;
      const cells = [...row.cells];
      const widths = cells.map((cell) => cssPixels(cell.style.width, tableRect.width));
      const declared = widths.reduce((sum, width) => sum + width, 0);
      const fallback = (tableRect.width - declared) / Math.max(1, widths.filter((width) => !width).length);
      const resolved = widths.map((width) => width || fallback);
      const cellIndex = cells.indexOf(element);
      const left = tableRect.left + resolved.slice(0, cellIndex).reduce((sum, width) => sum + width, 0);
      return rect(left, realisticMeasuredRect(row).top, resolved[cellIndex], 40);
    }
    const isAbsolute = layoutValue('position') === 'absolute' || computed.position === 'absolute';
    const containing = isAbsolute
      ? (element.closest?.('.ppt-export-body') ? rect(0, 0, 1280, 720) : parentRect)
      : parentRect;
    const leftValue = layoutValue('left') || computed.left;
    const topValue = layoutValue('top') || computed.top;
    const widthValue = layoutValue('width') || element.getAttribute?.('width') || computed.width;
    const heightValue = layoutValue('height') || element.getAttribute?.('height') || computed.height;
    const rightValue = layoutValue('right');
    const left = containing.left + cssPixels(leftValue, containing.width);
    const top = containing.top + cssPixels(topValue, containing.height);
    let width = cssPixels(widthValue, containing.width);
    let height = cssPixels(heightValue, containing.height);
    if (!width && rightValue) {
      width = containing.width - cssPixels(leftValue, containing.width)
        - cssPixels(rightValue, containing.width);
    }
    const borderWidth = ['Left', 'Right']
      .reduce((sum, side) => sum + cssPixels(computed[`border${side}Width`]), 0);
    const borderHeight = ['Top', 'Bottom']
      .reduce((sum, side) => sum + cssPixels(computed[`border${side}Width`]), 0);
    width = width || (tag === 'SVG' ? parentRect.width : Math.min(parentRect.width, 480));
    height = height || (/^(P|H[1-6]|LI)$/.test(tag) ? 28 : 48);
    if (cssPixels(widthValue) === 0 && borderWidth > 0) width = borderWidth;
    if (cssPixels(heightValue) === 0 && borderHeight > 0) height = borderHeight;
    return rect(left, top, width, height);
  };
  const measuredRect = realisticLayout ? realisticMeasuredRect : simpleMeasuredRect;
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

    // Sandbox iframes are populated via about:blank + document.write (srcdoc
    // renders blank in Tauri WebKit), so wait for the mounted document.
    const readMountedFrameHtml = async (frame) => {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        const mountedDoc = frame.contentDocument;
        const markup = mountedDoc?.documentElement?.outerHTML || '';
        if (/Safe text/.test(markup)) return markup;
        await new Promise((resolve) => { setTimeout(resolve, 10); });
      }
      throw new Error('sandbox iframe did not mount its document');
    };

    const exportStage = render.buildExportPreviewStage(unsafe);
    document.body.append(exportStage);
    const exportHtml = await readMountedFrameHtml(exportStage.querySelector('iframe'));

    const thumbContainer = document.createElement('div');
    thumbContainer.innerHTML = render.slideHtml({ id: 'thumb', html: unsafe });
    document.body.append(thumbContainer);
    render.hydrateHtmlSlideIframes(thumbContainer);
    const thumbHtml = await readMountedFrameHtml(thumbContainer.querySelector('iframe'));

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

test('UI and export host modules import with the current editable export API', async () => {
  await withControllableExportDom(async () => {
    const host = await import('../src/export-deck-host.js');
    assert.equal(typeof host.exportEditablePptx, 'function');
    await import('../ui.js');
    await new Promise((resolve) => { setTimeout(resolve, 100); });
  });
});

test('zero CSS border-radius remains a plain editable rectangle', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="plain-rectangle"
        style="position:absolute;left:96px;top:96px;width:192px;height:96px;
          border-radius:0px;background:#123456"></div>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'plain-rectangle-slide', html }]);
    const rectangle = scenes[0].nodes.find((node) => node.sourceId === 'plain-rectangle');

    assert.equal(rectangle.shapeType, 'rect');
    assert.equal(Object.hasOwn(rectangle.style, 'radius'), false);
  });
});

test('overflow-hidden decorative bleed bands do not block editable export', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0;overflow:hidden;position:relative">
      <div data-pptx-source-id="bleed-band" style="
        position:absolute;left:-140px;top:430px;width:1700px;height:130px;
        background:#1e3a8a;transform:rotate(-8deg)"></div>
      <h1 data-pptx-source-id="cover-title" style="
        position:absolute;left:80px;top:220px;width:800px;height:80px;
        margin:0;font-size:48px;color:#111">Bleed Cover</h1>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'bleed-cover', html }]);
    const band = scenes[0].nodes.find((node) => node.sourceId === 'bleed-band');
    const title = scenes[0].nodes.find((node) => node.sourceId === 'cover-title');
    assert.ok(band, 'bleed band should export as an editable shape');
    assert.equal(band.type, 'shape');
    assert.ok(band.x < 0, 'bleed band may start off-slide');
    assert.equal(band.style.rotate, 352); // -8deg normalized to [0,360)
    assert.ok(title, 'cover title should remain editable text');
    assert.equal(title.type, 'text');
    assert.ok(title.x >= 0 && title.y >= 0);
  }, { realisticLayout: true });
});

test('editable scene allows negative shape origins for bleed but rejects negative text origins', () => {
  const bleed = editableSceneWith({
    type: 'shape', sourceId: 'bleed-shape',
    x: -1.5, y: 2, w: 17, h: 1.3, shapeType: 'rect',
    style: { fill: '1E3A8A', rotate: 352 },
  });
  assert.equal(validateEditableSlideScene(bleed), bleed);

  assert.throws(
    () => validateEditableSlideScene(editableSceneWith({
      type: 'text', sourceId: 'offslide-text',
      x: -1, y: 1, w: 2, h: 1, text: 'Nope', style: {},
    })),
    (error) => error.code === 'editable_scene_geometry_invalid'
      && error.sourceId === 'offslide-text',
  );
});

test('hard CSS ring box-shadow rewrites to a concentric editable shape', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="pptx-source-12" style="
        position:absolute;left:571px;top:40px;width:17px;height:17px;
        background:#1e3a8a;border-radius:50%;
        box-shadow:0 0 0 1px rgba(0,0,0,0.1)"></div>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'ring-dot-slide', html }]);
    const ringNodes = scenes[0].nodes.filter((node) => (
      node.sourceId === 'pptx-source-12' && node.rewrite === 'css_box_shadow_ring'
    ));
    const mainNodes = scenes[0].nodes.filter((node) => (
      node.sourceId === 'pptx-source-12' && !node.rewrite
    ));
    assert.equal(ringNodes.length, 1);
    assert.equal(mainNodes.length, 1);
    assert.equal(ringNodes[0].style.fill, '000000');
    assert.equal(ringNodes[0].style.transparency, 90);
    assert.ok(ringNodes[0].w > mainNodes[0].w);
    assert.ok(ringNodes[0].h > mainNodes[0].h);
    assert.equal(Object.hasOwn(mainNodes[0].style, 'shadow'), false);
  });
});

for (const paintCase of [
  {
    name: 'inline SVG fill paint server',
    sourceId: 'inline-fill-server',
    body: `<svg viewBox="0 0 100 100">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#fff"/></linearGradient></defs>
      <rect data-pptx-source-id="inline-fill-server" width="80" height="80"
        style="fill:url(#g)"/>
    </svg>`,
  },
  {
    name: 'inline SVG stroke paint server',
    sourceId: 'inline-stroke-server',
    body: `<svg viewBox="0 0 100 100">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#fff"/></linearGradient></defs>
      <line data-pptx-source-id="inline-stroke-server" x2="80" y2="80"
        style="stroke:url(#g)"/>
    </svg>`,
  },
  {
    name: 'stylesheet SVG computed paint server',
    sourceId: 'stylesheet-paint-server',
    body: `<style>.paint-server-target { fill: url(#g); }</style>
      <svg viewBox="0 0 100 100">
        <defs><linearGradient id="g"><stop offset="0" stop-color="#fff"/></linearGradient></defs>
        <rect class="paint-server-target" data-pptx-source-id="stylesheet-paint-server"
          width="80" height="80"/>
      </svg>`,
  },
]) {
  test(`degrades ${paintCase.name} instead of aborting the export`, async () => {
    await withControllableExportDom(async () => {
      const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        ${paintCase.body}
      </body></html>`;
      const degradations = [];
      const scenes = await prepareEditableSlides([{ id: paintCase.sourceId, html }], {
        onDegrade: (record) => degradations.push(record),
      });
      // The export must succeed: paint servers are either rewritten to solid
      // paint or already neutralized by the sanitizer.
      assert.equal(scenes.length, 1);
      assert.equal(scenes[0].slideNumber, 1);
    });
  });
}

for (const unsupportedCase of [
  {
    name: 'canvas',
    sourceId: 'visible-canvas',
    markup: '<canvas data-pptx-source-id="visible-canvas" width="320" height="180"></canvas>',
  },
  {
    name: 'video',
    sourceId: 'visible-video',
    markup: '<video data-pptx-source-id="visible-video" controls src="movie.mp4"></video>',
  },
  {
    name: 'audio',
    sourceId: 'visible-audio',
    markup: '<audio data-pptx-source-id="visible-audio" controls src="sound.mp3"></audio>',
  },
  {
    name: 'iframe',
    sourceId: 'visible-iframe',
    markup: '<iframe data-pptx-source-id="visible-iframe" srcdoc="<p>Visible</p>"></iframe>',
  },
  {
    name: 'object',
    sourceId: 'visible-object',
    markup: '<object data-pptx-source-id="visible-object" data="diagram.svg"></object>',
  },
  {
    name: 'embed',
    sourceId: 'visible-embed',
    markup: '<embed data-pptx-source-id="visible-embed" src="diagram.svg">',
  },
]) {
  test(`removes visible unsupported ${unsupportedCase.name} and still exports`, async () => {
    await withControllableExportDom(async () => {
      const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        ${unsupportedCase.markup}
      </body></html>`;
      const degradations = [];
      const scenes = await prepareEditableSlides([{ id: unsupportedCase.sourceId, html }], {
        onDegrade: (record) => degradations.push(record),
      });
      assert.equal(scenes.length, 1);
      // The sanitizer removed the unsupported element; the export reports it.
      assert.ok(
        degradations.some((record) => record.code === 'active_content_removed'),
        `expected an active_content_removed degradation, got ${JSON.stringify(degradations)}`,
      );
    });
  });
}

test('removes active script but ignores non-visual document metadata during export', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const scripted = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <p data-pptx-source-id="kept-copy">Visible copy</p>
      <script data-pptx-source-id="active-script">document.body.textContent = 'painted';</script>
    </body></html>`;
    const degradations = [];
    const scenes = await prepareEditableSlides([{ id: 'active', html: scripted }], {
      onDegrade: (record) => degradations.push(record),
    });
    assert.equal(scenes.length, 1);
    assert.ok(
      scenes[0].nodes.some((node) => node.type === 'text' && JSON.stringify(node.text).includes('Visible copy')),
      'script removal must not destroy the rest of the slide',
    );
    assert.ok(degradations.some((record) => record.code === 'active_content_removed'));

    const metadata = `<!doctype html><html><head>
      <base href="https://example.invalid/"><meta name="description" content="metadata">
    </head><body style="width:1280px;height:720px;margin:0">
      <p data-pptx-source-id="metadata-copy">Visible copy</p>
    </body></html>`;
    const metadataScenes = await prepareEditableSlides([{ id: 'metadata', html: metadata }]);
    assert.equal(metadataScenes.length, 1);
  });
});

test('degrades escaped and external SVG paint-server CSS without matching comments or strings', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const degraded = [
      {
        sourceId: 'escaped-inline-paint',
        body: `<svg><rect data-pptx-source-id="escaped-inline-paint"
          style="fill:u\\72l(#g)" width="20" height="20"/></svg>`,
      },
      {
        sourceId: 'external-inline-paint',
        body: `<svg><line data-pptx-source-id="external-inline-paint"
          style="stroke:url(https://example.invalid/paint.svg#g)" x2="20" y2="20"/></svg>`,
      },
      {
        sourceId: 'comment-split-stylesheet-paint',
        body: `<style>.paint-token { stroke: u/**/rl(#g); }</style>
          <svg><line class="paint-token" data-pptx-source-id="comment-split-stylesheet-paint"
            x2="20" y2="20"/></svg>`,
      },
      {
        sourceId: 'inherited-computed-paint',
        body: `<style>
            svg.paint-scope { --paint: u\\72l(#g); }
            .computed-paint { fill: var(--paint); }
          </style>
          <svg class="paint-scope"><rect class="computed-paint"
            data-pptx-source-id="inherited-computed-paint" width="20" height="20"/></svg>`,
      },
    ];
    for (const paint of degraded) {
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        ${paint.body}
      </body></html>`;
      const scenes = await prepareEditableSlides([{ id: paint.sourceId, html }]);
      assert.equal(scenes.length, 1, paint.sourceId);
    }

    const safeHtml = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <style>
        /* .safe-paint { fill:url(#unused); } */
        .safe-paint { fill:#123456; --note:"stroke:url(#unused)"; }
      </style>
      <svg><rect class="safe-paint" data-pptx-source-id="safe-paint"
        width="20" height="20"/></svg>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'safe-paint', html: safeHtml }]);
    assert.equal(scenes.length, 1);
  });
});

test('degrades an SVG paint server selected by higher selector specificity', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <style>#target { fill:url(#g); } .shape { fill:#fff; }</style>
      <svg><rect id="target" class="shape" data-pptx-source-id="specific-paint"
        width="20" height="20"/></svg>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'specific-paint', html }]);
    assert.equal(scenes.length, 1);
  });
});

test('degrades an SVG paint server selected by important over higher specificity', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <style>.shape { fill:url(#g) !important; } #target { fill:#fff; }</style>
      <svg><rect id="target" class="shape" data-pptx-source-id="important-paint"
        width="20" height="20"/></svg>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'important-paint', html }]);
    assert.equal(scenes.length, 1);
  });
});

test('does not block SVG paint servers overridden by the winning cascade declaration', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const styles = [
      '.shape { fill:url(#g); } #target { fill:#fff; }',
      '#target { fill:url(#g); } .shape { fill:#fff !important; }',
    ];
    for (const [index, css] of styles.entries()) {
      const sourceId = `overridden-paint-${index + 1}`;
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        <style>${css}</style>
        <svg><rect id="target" class="shape" data-pptx-source-id="${sourceId}"
          width="20" height="20"/></svg>
      </body></html>`;
      const scenes = await prepareEditableSlides([{ id: sourceId, html }]);
      assert.equal(scenes.length, 1, sourceId);
    }
  });
});

test('strips non-none text-shadow and keeps the text editable', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    for (const [body, expectDegradation] of [
      [`<p data-pptx-source-id="inline-text-shadow"
        style="text-shadow:1px 1px 2px #000">Shadowed</p>`, true],
      // jsdom does not match shadow-root scoped stylesheets in getComputedStyle,
      // so the stylesheet case only asserts the export succeeds and keeps text;
      // a live WebView resolves the rule and records the same degradation.
      [`<style>.shadowed-copy { text-shadow:1px 1px #000; }</style>
        <p class="shadowed-copy" data-pptx-source-id="stylesheet-text-shadow">Shadowed</p>`, false],
    ]) {
      const sourceId = body.match(/data-pptx-source-id="([^"]+)"/)[1];
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        ${body}
      </body></html>`;
      const degradations = [];
      const scenes = await prepareEditableSlides([{ id: sourceId, html }], {
        onDegrade: (record) => degradations.push(record),
      });
      assert.equal(scenes.length, 1, sourceId);
      const textNode = scenes[0].nodes.find((node) => node.sourceId === sourceId);
      assert.ok(textNode, `${sourceId} should remain editable text`);
      if (expectDegradation) {
        assert.ok(
          degradations.some((record) => record.code === 'text_shadow_removed'),
          `expected text_shadow_removed degradation for ${sourceId}, got ${JSON.stringify(degradations)}`,
        );
      }
    }
  });
});

test('preserves a supported CSS outer box-shadow as native scene and OOXML shadow', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="shadow-card" style="
        position:absolute;left:96px;top:96px;width:240px;height:120px;
        background:#ddeeff;box-shadow:4px 6px 8px rgba(17,34,51,.4)"></div>
    </body></html>`;
    const deck = { title: 'Native shadow', slides: [{ id: 'shadow', html }] };
    const scenes = await prepareEditableSlides(deck.slides);
    const card = scenes[0].nodes.find((node) => node.sourceId === 'shadow-card');

    assert.deepEqual(card.style.shadow, {
      type: 'outer',
      angle: 56,
      blur: 6,
      color: '112233',
      offset: Math.sqrt(52) * 0.75,
      opacity: 0.4,
    });

    const exported = await exportEditablePptx(deck, scenes);
    const zip = await JSZip.loadAsync(Buffer.from(exported.base64, 'base64'));
    const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
    assert.match(slideXml, /<a:outerShdw\b/);
    assert.match(slideXml, /<a:srgbClr val="112233">[\s\S]*?<a:alpha val="40000"\/>/);
  });
});

test('maps the first usable layer of a multi-layer box-shadow to a native outer shadow', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="layered-shadow" style="
        position:absolute;left:96px;top:96px;width:240px;height:120px;
        background:#ddeeff;box-shadow:0 4px 6px rgba(0,0,0,.1), 0 10px 20px rgba(0,0,0,.15)"></div>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'layered', html }]);
    const card = scenes[0].nodes.find((node) => node.sourceId === 'layered-shadow');

    assert.equal(card.style.shadow.type, 'outer');
    assert.equal(card.style.shadow.angle, 90);
    assert.equal(card.style.shadow.blur, 4.5);
    assert.equal(card.style.shadow.color, '000000');
  });
});

test('negative or soft spread box-shadow approximates to a native outer shadow', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="spread-shadow" style="
        position:absolute;left:96px;top:96px;width:240px;height:120px;
        background:#ddeeff;box-shadow:0 20px 25px -5px rgba(0,0,0,.1)"></div>
    </body></html>`;
    const scenes = await prepareEditableSlides([{ id: 'spread', html }]);
    const card = scenes[0].nodes.find((node) => node.sourceId === 'spread-shadow');

    assert.equal(card.style.shadow.type, 'outer');
    assert.equal(card.style.shadow.blur, 18.75);
    assert.equal(card.style.shadow.offset, 15);
  });
});

test('inset-only box-shadow is stripped with a recorded degradation instead of aborting', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="inset-shadow" style="
        position:absolute;left:96px;top:96px;width:240px;height:120px;
        background:#ddeeff;box-shadow:inset 0 2px 4px rgba(0,0,0,.2)"></div>
    </body></html>`;
    const degradations = [];
    const scenes = await prepareEditableSlides([{ id: 'inset', html }], {
      onDegrade: (record) => degradations.push(record),
    });
    const card = scenes[0].nodes.find((node) => node.sourceId === 'inset-shadow');

    assert.ok(card, 'element stays editable without its shadow');
    assert.equal(Object.hasOwn(card.style, 'shadow'), false);
    assert.ok(
      degradations.some((record) => (
        record.code === 'box_shadow_removed' && record.sourceId === 'inset-shadow'
      )),
      `expected box_shadow_removed degradation, got ${JSON.stringify(degradations)}`,
    );
  });
});

test('element-model export skips elements that cannot be represented as editable objects', async () => {
  const slide = {
    title: 'Deck with a bad element',
    elements: [
      { id: 'ok-text', type: 'text', x: 10, y: 10, w: 40, h: 10, text: 'Keep me', style: {} },
      {
        id: 'bad-video', type: 'video', x: 10, y: 30, w: 40, h: 20,
        src: 'data:video/mp4;base64,AA==', style: {},
      },
    ],
  };
  const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
  const degradations = [];
  const scenes = await prepareEditableSlides([slide], {
    onDegrade: (record) => degradations.push(record),
  });

  assert.equal(scenes.length, 1);
  assert.ok(
    scenes[0].nodes.some((node) => node.type === 'text' && JSON.stringify(node.text).includes('Keep me')),
    'supported elements must survive the removal of a broken sibling',
  );
  assert.ok(
    degradations.some((record) => record.code === 'element_removed' && record.sourceId === 'bad-video'),
    `expected element_removed degradation, got ${JSON.stringify(degradations)}`,
  );
});

test('simplified scene builder always produces a valid editable scene', async () => {
  const { buildSimplifiedEditableScene } = await import('../src/export-degrade.js');
  const scene = buildSimplifiedEditableScene({
    slide: {
      title: '降级页标题',
      elements: [
        { type: 'text', text: '第一行内容' },
        { type: 'list', items: ['要点一', '要点二'] },
      ],
    },
    slideNumber: 2,
    width: 13.333,
    height: 7.5,
  });

  assert.equal(scene.slideNumber, 2);
  assert.equal(scene.nodes[0].type, 'shape');
  const texts = scene.nodes.filter((node) => node.type === 'text');
  assert.ok(texts.length >= 3);
  assert.ok(JSON.stringify(texts[0].text).includes('降级页标题'));
});

test('maps asymmetric text and merged-text padding as top right bottom left through OOXML', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <p data-pptx-source-id="padded-text"
        style="position:absolute;left:96px;top:96px;width:384px;height:96px;
          padding:10px 20px 30px 40px">Asymmetric text</p>
      <div data-pptx-source-id="padded-merge" data-pptx-merge="true"
        style="position:absolute;left:96px;top:240px;width:384px;height:120px;
          padding:11px 21px 31px 41px">
        <p>Merged text</p>
      </div>
    </body></html>`;
    const deck = { title: 'Asymmetric padding', slides: [{ id: 'padding', html }] };
    const scenes = await prepareEditableSlides(deck.slides);
    const textNode = scenes[0].nodes.find((node) => node.sourceId === 'padded-text');
    const mergeNode = scenes[0].nodes.find((node) => node.sourceId === 'padded-merge');

    assert.deepEqual(textNode.style.margin, [10 / 96, 20 / 96, 30 / 96, 40 / 96]);
    assert.deepEqual(mergeNode.style.margin, [11 / 96, 21 / 96, 31 / 96, 41 / 96]);

    const exported = await exportEditablePptx(deck, scenes);
    const zip = await JSZip.loadAsync(Buffer.from(exported.base64, 'base64'));
    const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
    const textShape = [...slideXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)]
      .map((match) => match[0])
      .find((xml) => xml.includes('Asymmetric text'));
    const mergeShape = [...slideXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)]
      .map((match) => match[0])
      .find((xml) => xml.includes('Merged text'));

    assert.match(textShape, /<a:bodyPr[^>]*lIns="381000"[^>]*tIns="95250"[^>]*rIns="190500"[^>]*bIns="285750"/);
    assert.match(mergeShape, /<a:bodyPr[^>]*lIns="390525"[^>]*tIns="104775"[^>]*rIns="200025"[^>]*bIns="295275"/);
  });
});

test('collapsed table borders tolerate subpixel outer-edge differences', async () => {
  await withControllableExportDom(async ({ window }) => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const prototype = window.HTMLElement.prototype;
    const measuredRect = prototype.getBoundingClientRect;
    prototype.getBoundingClientRect = function getBoundingClientRect() {
      const rect = measuredRect.call(this);
      if (this.tagName === 'TABLE') {
        return {
          ...rect,
          top: rect.top - 0.6,
          bottom: rect.bottom + 0.6,
          height: rect.height + 1.2,
          toJSON() {
            return {
              left: rect.left,
              top: rect.top - 0.6,
              width: rect.width,
              height: rect.height + 1.2,
            };
          },
        };
      }
      if (!['TD', 'TH'].includes(this.tagName) || this.cellIndex !== this.parentElement.cells.length - 1) {
        return rect;
      }
      return {
        ...rect,
        width: rect.width - 1.2,
        right: rect.right - 1.2,
        toJSON() {
          return {
            left: rect.left,
            top: rect.top,
            width: rect.width - 1.2,
            height: rect.height,
          };
        },
      };
    };
    try {
      const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
        <table data-pptx-source-id="collapsed-table"
          style="position:absolute;left:80px;top:120px;width:800px;height:160px;
            border-collapse:collapse">
          <tr>
            <th style="width:50%;border:2px solid #111;background:#eee">A</th>
            <th style="width:50%;border:2px solid #111;background:#eee">B</th>
          </tr>
          <tr>
            <td style="border:2px solid #111">C</td>
            <td style="border:2px solid #111">D</td>
          </tr>
        </table>
      </body></html>`;
      const scenes = await prepareEditableSlides([{ id: 'collapsed-table-slide', html }]);
      const table = scenes[0].nodes.find((node) => node.sourceId === 'collapsed-table');

      assert.equal(table.type, 'table');
      assert.equal(table.columnWidths.length, 2);
      assert.equal(table.rows.length, 2);
    } finally {
      prototype.getBoundingClientRect = measuredRect;
    }
  }, { realisticLayout: true });
});

test('CSS border-radius preserves exact editable roundRect OOXML adjustment', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="css-rounded"
        style="position:absolute;left:96px;top:96px;width:192px;height:96px;
          border-radius:4px;background:#123456"></div>
    </body></html>`;
    const deck = { title: 'CSS radius', slides: [{ id: 'css-radius', html }] };
    const scenes = await prepareEditableSlides(deck.slides);
    const rounded = scenes[0].nodes.find((node) => node.sourceId === 'css-rounded');
    assert.equal(rounded.shapeType, 'roundRect');
    assert.equal(rounded.style.radius, 4 / 96);

    const exported = await exportEditablePptx(deck, scenes);
    const zip = await JSZip.loadAsync(Buffer.from(exported.base64, 'base64'));
    const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
    assert.match(
      slideXml,
      /<a:prstGeom prst="roundRect">[\s\S]*?<a:gd name="adj" fmla="val 4167"\/>/,
    );
    assert.doesNotMatch(slideXml, /fmla="val 16667"/);
  });
});

test('HTML preparation and export never call raster or page renderers', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    let rasterCalls = 0;
    let pageCalls = 0;
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="panel"
        style="position:absolute;left:40px;top:40px;width:400px;height:200px;background:#ddeeff"></div>
      <p data-pptx-source-id="title"
        style="position:absolute;left:80px;top:90px;width:300px;height:50px">Editable title</p>
      <svg viewBox="0 0 100 100"
        style="position:absolute;left:500px;top:80px;width:200px;height:200px">
        <path data-pptx-source-id="route" d="M0 0L100 100" fill="none" stroke="#123456"/>
      </svg>
    </body></html>`;
    const deck = { title: 'Strict HTML', slides: [{ id: 'strict', html }] };
    const scenes = await prepareEditableSlides(deck.slides, {
      renderRaster: () => { rasterCalls += 1; },
      renderPage: () => { pageCalls += 1; },
    });
    const result = await exportEditablePptx(deck, scenes);
    const zip = await JSZip.loadAsync(Buffer.from(result.base64, 'base64'));
    const [slideXml, relsXml] = await Promise.all([
      zipText(zip, 'ppt/slides/slide1.xml'),
      zipText(zip, 'ppt/slides/_rels/slide1.xml.rels'),
    ]);

    assert.equal(rasterCalls, 0);
    assert.equal(pageCalls, 0);
    assert.match(slideXml, /Editable title/);
    assert.doesNotMatch(slideXml, /<p:pic>/);
    assert.doesNotMatch(relsXml, /relationships\/image"/);
    assert.equal(zip.file(/^ppt\/media\//).length, 0);
  });
});

// This Node/JSDOM case simulates WebKit border-box measurements only. Release
// acceptance still requires exporting these fixtures in the real Tauri WebKit
// runtime and inspecting the resulting editable-only OOXML artifact.
test('production fixtures lock OOXML under a WebKit border-box regression simulation', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    const fixtureBuffers = await Promise.all(PRODUCTION_FIXTURES.map((fixture) => (
      readFile(new URL(`./fixtures/${fixture.name}`, import.meta.url))
    )));
    fixtureBuffers.forEach((buffer, index) => {
      assert.equal(
        createHash('sha256').update(buffer).digest('hex'),
        PRODUCTION_FIXTURES[index].sha256,
        `${PRODUCTION_FIXTURES[index].name} must remain byte-for-byte complete`,
      );
    });
    const fixtureHtml = fixtureBuffers.map((buffer) => buffer.toString('utf8'));
    const deck = {
      title: 'Production editable acceptance',
      slides: fixtureHtml.map((html, index) => ({ id: `production-${index + 1}`, html })),
    };
    const progress = [];
    let rasterCalls = 0;
    let pageCalls = 0;
    const scenes = await prepareEditableSlides(deck.slides, {
      onSlideProgress: (pageNumber) => progress.push(pageNumber),
      renderRaster: () => { rasterCalls += 1; },
      renderPage: () => { pageCalls += 1; },
    });

    assert.deepEqual(progress, [1, 2, 3]);
    assert.equal(rasterCalls, 0);
    assert.equal(pageCalls, 0);
    assert.deepEqual(scenes.map((scene) => scene.slideNumber), [1, 2, 3]);
    scenes.forEach((scene, index) => {
      assert.equal(scene.nodes.some((node) => node.type === 'image'), false, `slide ${index + 1}`);
    });

    const [first, second, third] = scenes;
    const firstShapes = first.nodes.filter((node) => node.type === 'shape');
    const firstLines = first.nodes.filter((node) => node.type === 'line');
    const firstText = first.nodes.filter((node) => node.type === 'text');
    assert.deepEqual(
      { shapes: firstShapes.length, lines: firstLines.length, text: firstText.length },
      { shapes: 15, lines: 18, text: 13 },
    );
    const firstMap = svgPointMapper({
      leftPt: 500, topPt: 115, widthPt: 400, heightPt: 235, viewWidth: 400, viewHeight: 260,
    });
    const cssUpId = fixtureSourceId(fixtureHtml[0], 'div[style*="border-bottom: 56pt"]');
    const cssRightId = fixtureSourceId(fixtureHtml[0], 'div[style*="border-left: 12pt"]');
    const svgTriangleId = fixtureSourceId(fixtureHtml[0], 'svg polygon');
    assert.equal(new Set([cssUpId, cssRightId, svgTriangleId]).size, 3);
    const cssUp = sceneNodesBySource(first, cssUpId, 'shape');
    const cssRight = sceneNodesBySource(first, cssRightId, 'shape');
    const svgTriangle = sceneNodesBySource(first, svgTriangleId, 'shape');
    assert.equal(cssUp.length, 1);
    assert.equal(cssRight.length, 1);
    assert.equal(svgTriangle.length, 1);
    assert.equal(sceneNodesBySource(first, cssUpId).length, 1,
      'CSS up triangle must not also emit partial-border lines');
    assert.equal(sceneNodesBySource(first, cssRightId).length, 1,
      'CSS right triangle must not also emit partial-border lines');
    assert.deepEqual(
      [cssUp[0].shapeType, cssUp[0].style.rotate, cssUp[0].style.fill],
      ['triangle', 0, '1E293B'],
    );
    assertNear(cssUp[0].x, 290 / 72, 'CSS up triangle x');
    assertNear(cssUp[0].y, 195 / 72, 'CSS up triangle y');
    assertNear(cssUp[0].w, 64 / 72, 'CSS up triangle width');
    assertNear(cssUp[0].h, 56 / 72, 'CSS up triangle height');
    assert.deepEqual(
      [cssRight[0].shapeType, cssRight[0].style.rotate, cssRight[0].style.fill],
      ['triangle', 90, '787774'],
    );
    assertNear(cssRight[0].x, 408 / 72, 'CSS right arrow x');
    assertNear(cssRight[0].y, 286 / 72, 'CSS right arrow y');
    assertNear(cssRight[0].w, 12 / 72, 'CSS right arrow width');
    assertNear(cssRight[0].h, 14 / 72, 'CSS right arrow height');
    const svgTriangleStart = firstMap({ x: 240, y: 82 });
    const svgTriangleEnd = firstMap({ x: 320, y: 138 });
    assert.deepEqual(
      [svgTriangle[0].shapeType, svgTriangle[0].style.rotate, svgTriangle[0].style.fill],
      ['triangle', 0, '1E293B'],
    );
    assertNear(svgTriangle[0].x, svgTriangleStart.x, 'SVG triangle x');
    assertNear(svgTriangle[0].y, svgTriangleStart.y, 'SVG triangle y');
    assertNear(svgTriangle[0].w, svgTriangleEnd.x - svgTriangleStart.x, 'SVG triangle width');
    assertNear(svgTriangle[0].h, svgTriangleEnd.y - svgTriangleStart.y, 'SVG triangle height');

    const curveId = fixtureSourceId(fixtureHtml[0], 'svg path[d*=" Q "]');
    const curveLines = sceneNodesBySource(first, curveId, 'line');
    assert.equal(curveLines.length, 16);
    const curveStart = firstMap({ x: 20, y: 200 });
    const curveQMid = firstMap({ x: 115, y: 185 });
    const curveJoin = firstMap({ x: 200, y: 200 });
    const curveTMid = firstMap({ x: 282.5, y: 215 });
    const curveEnd = firstMap({ x: 370, y: 200 });
    assertLineEndpoints(curveLines[0], {
      x1: curveStart.x, y1: curveStart.y,
      x2: curveLines[0].x2, y2: curveLines[0].y2,
    }, 'Q/T first segment');
    assertNear(curveLines[3].x2, curveQMid.x, 'Q midpoint x');
    assertNear(curveLines[3].y2, curveQMid.y, 'Q midpoint y');
    assertNear(curveLines[7].x2, curveJoin.x, 'Q/T join x');
    assertNear(curveLines[7].y2, curveJoin.y, 'Q/T join y');
    assertNear(curveLines[11].x2, curveTMid.x, 'T midpoint x');
    assertNear(curveLines[11].y2, curveTMid.y, 'T midpoint y');
    assertNear(curveLines.at(-1).x2, curveEnd.x, 'Q/T end x');
    assertNear(curveLines.at(-1).y2, curveEnd.y, 'Q/T end y');
    assert.ok(curveLines.every((node) => node.style.color === '0F766E'));
    const dashedId = fixtureSourceId(fixtureHtml[0], 'svg path[stroke-dasharray]');
    assert.notEqual(dashedId, curveId);
    const dashedLines = sceneNodesBySource(first, dashedId, 'line');
    assert.equal(dashedLines.length, 1);
    assert.equal(dashedLines[0].style.dashType, 'dash');
    const dashStart = firstMap({ x: 20, y: 230 });
    const dashEnd = firstMap({ x: 370, y: 230 });
    assertLineEndpoints(dashedLines[0], {
      x1: dashStart.x, y1: dashStart.y, x2: dashEnd.x, y2: dashEnd.y,
    }, 'independent dashed path');
    const firstRoundRects = firstShapes.filter((node) => node.shapeType === 'roundRect');
    assert.equal(firstRoundRects.length, 4);
    const roundRectSpecs = [
      {
        name: 'CSS rounded card',
        sourceId: fixtureSourceId(
          fixtureHtml[0],
          'div[style*="width: 170pt"][style*="border-radius: 10pt"]',
        ),
        geometry: { x: 250 / 72, y: 115 / 72, w: 170 / 72, h: 56 / 72 },
        radius: 10 / 72,
      },
      {
        name: 'CSS SVG panel',
        sourceId: fixtureSourceId(
          fixtureHtml[0],
          'div[style*="width: 400pt"][style*="height: 300pt"]',
        ),
        geometry: { x: 500 / 72, y: 115 / 72, w: 400 / 72, h: 300 / 72 },
        radius: 8 / 72,
      },
      {
        name: 'SVG rx8',
        sourceId: fixtureSourceId(fixtureHtml[0], 'svg rect[rx="8"]'),
        geometry: null,
        radius: 0.1004273504,
      },
      {
        name: 'SVG rx4',
        sourceId: fixtureSourceId(fixtureHtml[0], 'svg rect[rx="4"]'),
        geometry: null,
        radius: 0.0502136752,
      },
    ];
    assert.deepEqual(
      firstRoundRects.map((node) => node.sourceId),
      roundRectSpecs.map((spec) => spec.sourceId),
    );
    roundRectSpecs.forEach((spec) => {
      const nodes = sceneNodesBySource(first, spec.sourceId, 'shape');
      assert.equal(nodes.length, 1, spec.name);
      assert.equal(nodes[0].shapeType, 'roundRect', spec.name);
      assertNear(nodes[0].style.radius, spec.radius, `${spec.name} radius`, 1e-4);
      if (!spec.geometry) return;
      for (const property of ['x', 'y', 'w', 'h']) {
        assertNear(nodes[0][property], spec.geometry[property], `${spec.name} ${property}`, 1e-4);
      }
    });
    assert.match(JSON.stringify(firstText.map((node) => node.text)), /PPT 形状与元素测试矩阵/);
    assert.match(JSON.stringify(firstText.map((node) => node.text)), /曲线 · 虚线/);

    const secondShapes = second.nodes.filter((node) => node.type === 'shape');
    const secondLines = second.nodes.filter((node) => node.type === 'line');
    const secondText = second.nodes.filter((node) => node.type === 'text');
    assert.deepEqual(
      { shapes: secondShapes.length, lines: secondLines.length, text: secondText.length },
      // Dense <li><p> lists (unwrapped to ul>p) now keep their body text runs
      // instead of being deleted by empty-list degrade. Fixture text count rose.
      { shapes: 8, lines: 10, text: 30 },
    );
    const secondMap = svgPointMapper({
      leftPt: 600, topPt: 108, widthPt: 300, heightPt: 215, viewWidth: 300, viewHeight: 240,
    });
    const gridSources = Array.from({ length: 4 }, (_, index) => (
      fixtureSourceId(fixtureHtml[1], 'svg line[stroke-dasharray]', index)
    ));
    const gridLines = gridSources.flatMap((sourceId) => sceneNodesBySource(second, sourceId, 'line'));
    assert.equal(gridLines.length, 4);
    [60, 100, 140, 180].forEach((y, index) => {
      const start = secondMap({ x: 30, y });
      const end = secondMap({ x: 290, y });
      assert.equal(gridLines[index].style.dashType, 'dash');
      assert.equal(gridLines[index].style.color, 'E5E7EB');
      assertLineEndpoints(gridLines[index], {
        x1: start.x, y1: start.y, x2: end.x, y2: end.y,
      }, `grid line ${index + 1}`);
    });
    const axisSources = Array.from({ length: 2 }, (_, index) => (
      fixtureSourceId(fixtureHtml[1], 'svg line:not([stroke-dasharray])', index)
    ));
    const axes = axisSources.flatMap((sourceId) => sceneNodesBySource(second, sourceId, 'line'));
    assert.equal(axes.length, 2);
    const verticalAxisStart = secondMap({ x: 30, y: 20 });
    const verticalAxisEnd = secondMap({ x: 30, y: 200 });
    const horizontalAxisStart = secondMap({ x: 30, y: 200 });
    const horizontalAxisEnd = secondMap({ x: 290, y: 200 });
    assertLineEndpoints(axes[0], {
      x1: verticalAxisStart.x, y1: verticalAxisStart.y,
      x2: verticalAxisEnd.x, y2: verticalAxisEnd.y,
    }, 'vertical axis');
    assertLineEndpoints(axes[1], {
      x1: horizontalAxisStart.x, y1: horizontalAxisStart.y,
      x2: horizontalAxisEnd.x, y2: horizontalAxisEnd.y,
    }, 'horizontal axis');
    axes.forEach((axis) => {
      assert.equal(axis.style.color, '787774');
      assert.equal(axis.style.dashType, undefined);
    });
    const barSpecs = [
      [45, 53, 28, 147, '1E293B'],
      [85, 64, 28, 136, '1E293B'],
      [125, 48, 28, 152, '0F766E'],
      [165, 75, 28, 125, '1E293B'],
      [205, 59, 28, 141, '1E293B'],
      [245, 69, 28, 131, '1E293B'],
    ];
    barSpecs.forEach(([x, y, w, h, color], index) => {
      const sourceId = fixtureSourceId(fixtureHtml[1], 'svg rect', index);
      const bars = sceneNodesBySource(second, sourceId, 'shape');
      assert.equal(bars.length, 1, `bar ${index + 1}`);
      const start = secondMap({ x, y });
      const end = secondMap({ x: x + w, y: y + h });
      assert.equal(bars[0].style.fill, color);
      assertNear(bars[0].x, start.x, `bar ${index + 1} x`);
      assertNear(bars[0].y, start.y, `bar ${index + 1} y`);
      assertNear(bars[0].w, end.x - start.x, `bar ${index + 1} width`);
      assertNear(bars[0].h, end.y - start.y, `bar ${index + 1} height`);
    });
    const chartTextSpecs = [
      [59, 218, '推理', '787774'], [99, 218, '管道', '787774'],
      [139, 218, '安全', '787774'], [179, 218, '监控', '787774'],
      [219, 218, '网关', '787774'], [259, 218, '存储', '787774'],
      [59, 48, '92', '1E293B'], [99, 59, '85', '1E293B'],
      [139, 43, '95', '0F766E'], [179, 70, '78', '1E293B'],
      [219, 54, '88', '1E293B'], [259, 64, '82', '1E293B'],
    ];
    chartTextSpecs.forEach(([x, y, text, color], index) => {
      const sourceId = fixtureSourceId(fixtureHtml[1], 'svg text', index);
      const labels = sceneNodesBySource(second, sourceId, 'text');
      assert.equal(labels.length, 1, `chart text ${index + 1}`);
      assert.equal(labels[0].text, text);
      assert.equal(labels[0].style.align, 'center');
      assert.equal(labels[0].style.color?.toUpperCase(), color);
      const anchor = secondMap({ x, y });
      assertNear(labels[0].x + labels[0].w / 2, anchor.x, `chart text ${text} anchor x`);
      assert.ok(labels[0].y < anchor.y, `chart text ${text} baseline must follow its box`);
    });

    const thirdShapes = third.nodes.filter((node) => node.type === 'shape');
    const thirdLines = third.nodes.filter((node) => node.type === 'line');
    const thirdText = third.nodes.filter((node) => node.type === 'text');
    const thirdTables = third.nodes.filter((node) => node.type === 'table');
    assert.deepEqual(
      {
        shapes: thirdShapes.length,
        lines: thirdLines.length,
        text: thirdText.length,
        tables: thirdTables.length,
      },
      { shapes: 10, lines: 4, text: 16, tables: 1 },
    );
    [205, 375, 545, 715].forEach((leftPt, index) => {
      const lineId = fixtureSourceId(fixtureHtml[2], 'body > svg line', index);
      const triangleId = fixtureSourceId(fixtureHtml[2], 'body > svg polygon', index);
      const lines = sceneNodesBySource(third, lineId, 'line');
      const triangles = sceneNodesBySource(third, triangleId, 'shape');
      assert.equal(lines.length, 1, `flow arrow ${index + 1} line`);
      assert.equal(triangles.length, 1, `flow arrow ${index + 1} triangle`);
      const arrowMap = svgPointMapper({
        leftPt, topPt: 135.5, widthPt: 40, heightPt: 20, viewWidth: 40, viewHeight: 20,
      });
      const lineStart = arrowMap({ x: 2, y: 10 });
      const join = arrowMap({ x: 32, y: 10 });
      const triangleTopLeft = arrowMap({ x: 32, y: 5 });
      const triangleBottomRight = arrowMap({ x: 40, y: 15 });
      assertLineEndpoints(lines[0], {
        x1: lineStart.x, y1: lineStart.y, x2: join.x, y2: join.y,
      }, `flow arrow ${index + 1} line`);
      assert.equal(triangles[0].shapeType, 'triangle');
      assert.equal(triangles[0].style.rotate, 90);
      assert.equal(triangles[0].style.fill, '787774');
      assertNear(triangles[0].x, triangleTopLeft.x, `flow arrow ${index + 1} triangle x`);
      assertNear(triangles[0].y, triangleTopLeft.y, `flow arrow ${index + 1} triangle y`);
      assertNear(triangles[0].w, triangleBottomRight.x - triangleTopLeft.x,
        `flow arrow ${index + 1} triangle width`);
      assertNear(triangles[0].h, triangleBottomRight.y - triangleTopLeft.y,
        `flow arrow ${index + 1} triangle height`);
      assertNear(lines[0].x2, triangles[0].x, `flow arrow ${index + 1} geometry join`);
      assertNear(lines[0].y2, triangles[0].y + triangles[0].h / 2,
        `flow arrow ${index + 1} center join`);
    });
    assert.equal(thirdTables.length, 1);
    assert.equal(thirdTables[0].rows.length, 5);
    assert.equal(thirdTables[0].columnWidths.length, 4);
    assert.deepEqual(thirdTables[0].columnWidths, Array(4).fill(35 / 12));
    const tableCells = thirdTables[0].rows.flatMap((row) => row.cells);
    assert.equal(tableCells.length, 20);
    assert.deepEqual(
      tableCells.slice(0, 4).map((cell) => ({
        text: cell.text.map((run) => run.text).join(''),
        fill: cell.style.fill,
        fontColor: cell.style.fontColor,
        bold: cell.style.bold,
      })),
      [
        { text: '阶段', fill: '1E293B', fontColor: 'FFFFFF', bold: true },
        { text: '输入', fill: '1E293B', fontColor: 'FFFFFF', bold: true },
        { text: '处理动作', fill: '1E293B', fontColor: 'FFFFFF', bold: true },
        { text: '质量门槛', fill: '1E293B', fontColor: 'FFFFFF', bold: true },
      ],
    );
    assert.equal(tableCells[4].text.map((run) => run.text).join(''), '采集');
    assert.equal(tableCells[4].style.fontColor, '0F766E');
    assert.equal(tableCells[10].text.map((run) => run.text).join(''), '字段校验、去重、PII 脱敏');
    assert.equal(tableCells[19].text.map((run) => run.text).join(''), 'P99 延迟 ≤ 80ms');
    thirdTables[0].rows.forEach((row, rowIndex) => {
      const expectedFill = ['1E293B', 'FFFFFF', 'FAFAF7', 'FFFFFF', 'FAFAF7'][rowIndex];
      const expectedBorder = rowIndex === 0 ? '1E293B' : 'E5E7EB';
      row.cells.forEach((cell) => {
        assert.equal(cell.style.fill, expectedFill);
        assert.deepEqual(cell.style.padding, [8 / 72, 12 / 72, 8 / 72, 12 / 72]);
        for (const side of ['top', 'right', 'bottom', 'left']) {
          assert.deepEqual(cell.style.border[side], { color: expectedBorder, width: 1 });
        }
      });
    });
    thirdTables[0].rows.forEach((row) => assertNear(row.height, 40 / 96, 'table row height'));

    const exported = await exportEditablePptx(deck, scenes);
    assert.ok(exported.base64.length > 1000);
    const zip = await JSZip.loadAsync(Buffer.from(exported.base64, 'base64'));
    const slideXml = await Promise.all([1, 2, 3].map((page) => (
      zipText(zip, `ppt/slides/slide${page}.xml`)
    )));
    const relsXml = await Promise.all([1, 2, 3].map((page) => (
      zipText(zip, `ppt/slides/_rels/slide${page}.xml.rels`)
    )));
    slideXml.forEach((xml, index) => {
      const shapeCount = (xml.match(/<p:sp>/g) || []).length;
      const textCount = (xml.match(/<a:t>/g) || []).length;
      const tableCount = (xml.match(/<a:tbl>/g) || []).length;
      const scene = scenes[index];
      const expectedShapes = scene.nodes.filter((node) => node.type !== 'table').length;
      const expectedTextRuns = scene.nodes.reduce((count, node) => {
        if (node.type === 'text') return count + (Array.isArray(node.text) ? node.text.length : 1);
        if (node.type === 'table') {
          return count + node.rows.flatMap((row) => row.cells)
            .reduce((sum, cell) => sum + (Array.isArray(cell.text) ? cell.text.length : 1), 0);
        }
        return count;
      }, 0);
      assert.equal(shapeCount, expectedShapes, `slide ${index + 1} exact OOXML p:sp count`);
      assert.equal(textCount, expectedTextRuns, `slide ${index + 1} exact OOXML a:t count`);
      assert.equal(tableCount, index === 2 ? 1 : 0, `slide ${index + 1} OOXML table count`);
      assert.doesNotMatch(xml, /<p:pic>/);
      assert.doesNotMatch(relsXml[index], /relationships\/image"/);
    });
    assert.equal(zip.file(/^ppt\/media\//).length, 0);

    assert.equal((slideXml[0].match(/<a:prstGeom prst="triangle">/g) || []).length, 3);
    assert.equal((slideXml[0].match(/<a:xfrm rot="5400000"/g) || []).length, 1);
    assert.equal((slideXml[0].match(/<a:prstDash val="dash"\/>/g) || []).length, 1);
    const firstAdjustments = [...slideXml[0].matchAll(
      /<a:prstGeom prst="roundRect">[\s\S]*?<a:gd name="adj" fmla="val (\d+)"\/>/g,
    )].map((match) => Number(match[1]));
    assert.deepEqual(firstAdjustments, [17857, 2667, 16000, 8000]);
    assert.match(slideXml[0], /PPT 形状与元素测试矩阵/);
    assert.match(slideXml[0], /矩形 · 圆角 · 描边 · 圆 · 多边形 · 直线 · 曲线 · 虚线/);

    assert.equal((slideXml[1].match(/<a:prstDash val="dash"\/>/g) || []).length, 4);
    assert.match(slideXml[1], /<a:srgbClr val="0F766E"\/>/);
    assert.equal((slideXml[1].match(/<a:pPr[^>]*algn="ctr"/g) || []).length, 12);
    assert.match(slideXml[1], /<a:t>推理<\/a:t>/);
    assert.match(slideXml[1], /<a:t>95<\/a:t>/);

    assert.equal((slideXml[2].match(/<a:prstGeom prst="triangle">/g) || []).length, 4);
    assert.equal((slideXml[2].match(/<a:xfrm rot="5400000"/g) || []).length, 4);
    assert.match(slideXml[2], /<a:tbl>/);
    assert.match(slideXml[2], /<a:t>阶段<\/a:t>/);
    assert.match(slideXml[2], /<a:t>字段校验、去重、PII 脱敏<\/a:t>/);
    assert.match(slideXml[2], /<a:t>P99 延迟 ≤ 80ms<\/a:t>/);
    // PptxGenJS otherwise emits a duplicate cNvPr id for the first table, which
    // forces Microsoft PowerPoint into "needs repair" and corrupts table CJK.
    slideXml.forEach((xml, index) => {
      const objectIds = [...xml.matchAll(/<p:cNvPr id="(\d+)"/g)].map((match) => match[1]);
      assert.equal(
        new Set(objectIds).size,
        objectIds.length,
        `slide ${index + 1} cNvPr ids must be unique (got ${objectIds.join(',')})`,
      );
    });
    assert.match(slideXml[2], /<a:ea typeface="Microsoft YaHei"/);
    assert.match(slideXml[2], /<a:latin typeface="Arial"/);
    assert.match(slideXml[2], /<a:t>阶段<\/a:t>/);
  }, { realisticLayout: true, webkitBorderBoxRegressionSimulation: true });
});

test('unsupported CSS filter is stripped instead of aborting the export', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    let renderPageCalls = 0;
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <div data-pptx-source-id="filtered"
        style="position:absolute;left:40px;top:40px;width:300px;height:200px;filter:blur(2px)">
        Unsupported
      </div>
    </body></html>`;

    const degradations = [];
    const scenes = await prepareEditableSlides([{ id: 'blocked', html }], {
      renderPage: () => { renderPageCalls += 1; },
      onDegrade: (record) => degradations.push(record),
    });
    assert.equal(scenes.length, 1);
    assert.ok(
      degradations.some((record) => (
        record.code === 'css_filter_removed' && record.sourceId === 'filtered'
      )),
      `expected a css_filter_removed degradation, got ${JSON.stringify(degradations)}`,
    );
    assert.equal(renderPageCalls, 0);
  });
});

test('text box width safety scales with font size to prevent false wraps', async () => {
  const {
    textBoxWidthSafetyInches,
    safeTextBoxGeometry,
    buildSlideFromScene,
    createPptxDeck,
  } = await import('../src/pptx-html-build.js');

  assertNear(textBoxWidthSafetyInches(14), 0.36, '14pt keeps the calibrated body-text safety');
  assertNear(textBoxWidthSafetyInches(28), 0.72, '28pt doubles the body-text safety');
  assertNear(textBoxWidthSafetyInches(42), 1.08, '42pt triples the body-text safety');
  assert.ok(textBoxWidthSafetyInches(8) >= 0.28, 'tiny text still keeps a floor');
  assert.ok(textBoxWidthSafetyInches(72) <= 1.6, 'huge titles are capped');

  const body = safeTextBoxGeometry(1, 4, 'left', false, 14);
  const title = safeTextBoxGeometry(1, 4, 'left', false, 42);
  assertNear(body.w, 4.36, '14pt left text widens by 0.36"');
  assertNear(title.w, 5.08, '42pt left text widens by 1.08"');

  const centered = safeTextBoxGeometry(2, 4, 'center', false, 28);
  assertNear(centered.x, 2 - 0.36, 'center align shifts x by half the scaled safety');
  assertNear(centered.w, 4.72, 'center align keeps the full scaled width');

  const boldTitle = safeTextBoxGeometry(1, 4, 'left', false, 42, { bold: true });
  assert.ok(boldTitle.w > title.w, 'bold titles get a little extra width safety');

  // Near the slide's right edge: still apply the full safety margin even if the
  // box extends past 13.333" — PowerPoint accepts off-slide shape extents.
  const nearRight = safeTextBoxGeometry(12.5, 0.7, 'left', false, 14);
  assertNear(nearRight.w, 0.7 + 0.36, 'right-edge boxes keep the full safety width');
  assert.ok(nearRight.x + nearRight.w > 13.333, 'safety may extend past the slide edge');

  const pptx = createPptxDeck({ title: 'Width safety' });
  await buildSlideFromScene({
    slideNumber: 1,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'text',
      sourceId: 'title-42',
      x: 1,
      y: 1,
      w: 4,
      h: 1,
      text: '大号标题防换行',
      style: {
        fontSize: 42,
        fontFace: 'Microsoft YaHei',
        color: '111111',
        align: 'left',
        valign: 'top',
      },
    }],
  }, pptx);
  const zip = await writeAndOpen(pptx);
  const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
  const titleShape = [...slideXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)]
    .map((match) => match[0])
    .find((xml) => xml.includes('大号标题防换行'));
  assert.ok(titleShape, 'title shape must serialize');
  const expectedCx = Math.round(5.08 * 914400);
  assert.match(titleShape, new RegExp(`<a:ext cx="${expectedCx}"`));
});

test('pptx post-process removes PowerPoint repair triggers and uses cross-platform fonts', async () => {
  const {
    createPptxDeck,
    buildSlideFromScene,
    PPTX_CJK_FONT_FACE,
    PPTX_LATIN_FONT_FACE,
    resolvePptxFontFace,
    resolveCrossPlatformFontPair,
    normalizeOoxmlFonts,
  } = await import('../src/pptx-html-build.js');

  assert.equal(resolvePptxFontFace('PingFang SC'), PPTX_CJK_FONT_FACE);
  assert.equal(resolvePptxFontFace('system-ui', '中文'), PPTX_CJK_FONT_FACE);
  assert.equal(resolvePptxFontFace('Arial', '中文'), PPTX_CJK_FONT_FACE);
  assert.deepEqual(resolveCrossPlatformFontPair('PingFang SC'), {
    latin: PPTX_LATIN_FONT_FACE,
    ea: PPTX_CJK_FONT_FACE,
    cs: PPTX_LATIN_FONT_FACE,
  });
  assert.match(
    normalizeOoxmlFonts('<a:ea typeface=""/><a:cs typeface=""/>'),
    new RegExp(`typeface="${PPTX_CJK_FONT_FACE}"`),
  );

  const pptx = createPptxDeck({ title: 'Repair triggers' });
  for (let index = 0; index < 3; index += 1) {
    await buildSlideFromScene({
      slideNumber: index + 1,
      width: 13.333,
      height: 7.5,
      nodes: [
        {
          type: 'shape',
          sourceId: `bg-${index}`,
          shapeType: 'rect',
          x: 0.5,
          y: 0.5,
          w: 2,
          h: 1,
          style: { fill: 'E11D48' },
        },
        {
          type: 'text',
          sourceId: `title-${index}`,
          x: 1,
          y: 2,
          w: 6,
          h: 1,
          text: '跨平台中文与 Latin',
          style: {
            fontSize: 24,
            fontFace: 'PingFang SC',
            color: '111111',
            align: 'left',
            valign: 'top',
          },
        },
      ],
    }, pptx);
  }

  const zip = await writeAndOpen(pptx);
  const contentTypes = await zipText(zip, '[Content_Types].xml');
  assert.equal(
    (contentTypes.match(/slideMasters\/slideMaster\d+\.xml/g) || []).length,
    1,
    'Content_Types must declare only slideMaster1',
  );
  assert.doesNotMatch(contentTypes, /slideMaster2\.xml/);
  assert.doesNotMatch(contentTypes, /ContentType="image\/jpg"/);

  const notesMaster = await zipText(zip, 'ppt/notesMasters/notesMaster1.xml');
  assert.equal((notesMaster.match(/<p:sp>/g) || []).length, 0);

  const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
  const shapes = [...slideXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)].map((m) => m[0]);
  assert.ok(shapes.length >= 2, 'expected decorative shape + text box');
  for (const shape of shapes) {
    assert.match(shape, /<p:txBody>/, 'every p:sp must include txBody');
  }
  assert.match(slideXml, new RegExp(`<a:ea typeface="${PPTX_CJK_FONT_FACE}"`));
  assert.match(slideXml, new RegExp(`<a:latin typeface="${PPTX_LATIN_FONT_FACE}"`));
  assert.doesNotMatch(slideXml, /typeface="PingFang SC"/);

  const themeXml = await zipText(zip, 'ppt/theme/theme1.xml');
  assert.match(themeXml, new RegExp(`<a:ea typeface="${PPTX_CJK_FONT_FACE}"`));
  assert.doesNotMatch(themeXml, /<a:ea typeface=""/);
});

test('pptx post-process repairs notesSlide placeholders and empty line tags', async () => {
  const {
    createPptxDeck,
    ensureShapeTextBodies,
    ensureLineNoFill,
    fixNegativeExtents,
    fixInvalidTableCellAnchors,
  } = await import('../src/pptx-html-build.js');

  const notesPlaceholder = '<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/>'
    + '<p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>';
  assert.match(ensureShapeTextBodies(notesPlaceholder), /<p:txBody>/);
  assert.equal(
    ensureLineNoFill('<a:solidFill/><a:ln></a:ln>'),
    '<a:solidFill/><a:ln><a:noFill/></a:ln>',
  );
  assert.match(
    fixNegativeExtents('<a:xfrm><a:off x="100" y="200"/><a:ext cx="50" cy="-80"/></a:xfrm>'),
    /<a:xfrm flipV="1"><a:off x="100" y="120"\/><a:ext cx="50" cy="80"\/>/,
  );
  assert.match(
    fixInvalidTableCellAnchors('<a:tcPr marL="0" anchor="mid"><a:noFill/></a:tcPr>'),
    /anchor="ctr"/,
  );
  assert.doesNotMatch(
    fixInvalidTableCellAnchors('<a:tcPr marL="0" anchor="mid"><a:noFill/></a:tcPr>'),
    /anchor="mid"/,
  );

  const pptx = createPptxDeck({ title: 'Notes placeholder' });
  const slide = pptx.addSlide();
  slide.addShape(pptx.shapes.RECTANGLE, {
    x: 0.5,
    y: 0.5,
    w: 2,
    h: 1,
    fill: { color: 'FFFFFF' },
  });
  slide.addText('备注页占位', {
    x: 1,
    y: 2,
    w: 4,
    h: 1,
    fontSize: 18,
    fontFace: 'Arial',
    color: '111111',
  });

  const zip = await writeAndOpen(pptx);
  const notesXml = await zipText(zip, 'ppt/notesSlides/notesSlide1.xml');
  const notesShapes = [...notesXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)].map((m) => m[0]);
  assert.ok(notesShapes.length >= 1, 'notesSlide must contain shapes');
  for (const shape of notesShapes) {
    assert.match(shape, /<p:txBody>/, 'notesSlide shapes must include txBody');
  }
  const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
  assert.doesNotMatch(slideXml, /<a:ln>\s*<\/a:ln>/);
  assert.doesNotMatch(slideXml, /<a:ln\/>/);
});

test('pptx post-process adds effectLst to solid slide backgrounds (#1442)', async () => {
  const {
    createPptxDeck,
    ensureSolidBackgroundEffectList,
  } = await import('../src/pptx-html-build.js');

  assert.equal(
    ensureSolidBackgroundEffectList(
      '<p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></p:bgPr>',
    ),
    '<p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr>',
  );
  assert.equal(
    ensureSolidBackgroundEffectList(
      '<p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr>',
    ),
    '<p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr>',
    'idempotent when effectLst already present',
  );

  const pptx = createPptxDeck({ title: 'Solid background' });
  const slide = pptx.addSlide();
  slide.background = { color: 'F8FAFC' };
  slide.addText('背景加固', {
    x: 1,
    y: 1,
    w: 4,
    h: 1,
    fontSize: 20,
    fontFace: 'Arial',
    color: '111111',
  });

  const zip = await writeAndOpen(pptx);
  const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
  assert.match(
    slideXml,
    /<p:bgPr><a:solidFill><a:srgbClr val="F8FAFC"\/><\/a:solidFill><a:effectLst\/><\/p:bgPr>/,
  );
});

test('CSS letter-spacing is preserved as PPTX charSpacing', async () => {
  await withControllableExportDom(async () => {
    const { prepareEditableSlides } = await import('../src/export-slide-browser.js');
    const { exportEditablePptx } = await import('../src/export-deck-browser.js');
    const html = `<!doctype html><html><body style="width:1280px;height:720px;margin:0">
      <h1 data-pptx-source-id="tracked-title"
        style="position:absolute;left:80px;top:120px;width:900px;height:80px;margin:0;
          font-size:48px;letter-spacing:-0.8px;color:#111">紧排标题</h1>
    </body></html>`;
    const deck = { title: 'Letter spacing', slides: [{ id: 'tracked', html }] };
    const scenes = await prepareEditableSlides(deck.slides);
    const title = scenes[0].nodes.find((node) => node.sourceId === 'tracked-title');
    assert.ok(title, 'tracked title must extract');
    assertNear(title.style.charSpacing, -0.8 * 0.75, 'letter-spacing px maps to charSpacing pt');

    const exported = await exportEditablePptx(deck, scenes);
    const zip = await JSZip.loadAsync(Buffer.from(exported.base64, 'base64'));
    const slideXml = await zipText(zip, 'ppt/slides/slide1.xml');
    const titleShape = [...slideXml.matchAll(/<p:sp>[\s\S]*?<\/p:sp>/g)]
      .map((match) => match[0])
      .find((xml) => xml.includes('紧排标题'));
    assert.ok(titleShape, 'tracked title must serialize');
    const expectedSpc = Math.round((-0.8 * 0.75) * 100);
    assert.match(titleShape, new RegExp(`spc="${expectedSpc}"`));
  });
});
