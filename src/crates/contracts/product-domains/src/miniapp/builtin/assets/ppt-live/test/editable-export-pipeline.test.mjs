import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

import {
  exportEditablePptx,
} from '../src/export-deck-browser.js';
import { prepareEditableSlides } from '../src/export-slide-browser.js';
import {
  normalizeElementSlideToEditableScene,
} from '../src/pptx-element-export.js';
import {
  buildSlideFromScene,
  createPptxDeck,
} from '../src/pptx-html-build.js';
import {
  EditableExportError,
  validateEditableSlideScene,
} from '../src/editable-slide-scene.js';

const requireFromPptxGen = createRequire(import.meta.resolve('pptxgenjs'));
const JSZip = requireFromPptxGen('jszip');
const VALID_IMAGE_DATA = {
  png: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
  jpeg: 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////2wBDAf//////////////////////////////////////////////////////////////////////////////////////wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAf/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABBQJ//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAwEBPwF//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAgEBPwF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQAGPwJ//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPyF//9oADAMBAAIAAwAAABD/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAEDAQE/EH//xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAECAQE/EH//xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAE/EH//2Q==',
  webp: 'data:image/webp;base64,UklGRhoAAABXRUJQVlA4TA0AAAAvAAAAAAfQ//73v/+BiA==',
};

function textScene(slideNumber = 1) {
  return {
    slideNumber,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'text',
      sourceId: `title-${slideNumber}`,
      x: 1,
      y: 1,
      w: 4,
      h: 0.6,
      text: `Slide ${slideNumber}`,
      paintOrder: 0,
      subOrder: 0,
      style: { fontFace: 'Arial', fontSize: 20, color: '111111' },
    }],
  };
}

async function openExport(result) {
  return JSZip.loadAsync(Buffer.from(result.base64, 'base64'));
}

test('host export module re-exports only APIs that exist', async () => {
  const hostModule = await import('../src/export-deck-host.js');
  assert.equal(typeof hostModule.exportEditablePptx, 'function');
  assert.equal(typeof hostModule.exportPdfFromBase64Pages, 'function');
  assert.equal(typeof hostModule.exportPngZipFromPages, 'function');
  assert.equal(Object.hasOwn(hostModule, 'exportPptxFromDeck'), false);
});

test('element-model decks normalize to scenes and use the shared editable serializer', async () => {
  const deck = {
    title: 'Legacy element deck',
    slides: [{
      id: 'legacy-1',
      title: 'Legacy',
      theme: { background: '#ffffff', ink: '#111111', primary: '#0f766e' },
      elements: [{
        id: 'legacy-title',
        type: 'text',
        x: 10,
        y: 10,
        w: 40,
        h: 12,
        text: 'Legacy editable title',
        style: { fontSize: 28, color: 'ink' },
      }],
    }],
  };
  const scene = normalizeElementSlideToEditableScene(deck.slides[0], {
    slideNumber: 1,
  });
  assert.deepEqual(scene.nodes.map((node) => node.type), ['shape', 'shape', 'text']);

  const scenes = await prepareEditableSlides(deck.slides);
  const result = await exportEditablePptx(deck, scenes);
  const zip = await openExport(result);
  const slideXml = await zip.file('ppt/slides/slide1.xml').async('string');
  assert.match(slideXml, /Legacy editable title/);
  assert.doesNotMatch(slideXml, /<p:pic>/);
  assert.equal(zip.file(/^ppt\/media\//).length, 0);
});

test('element-model lists preserve native bullet options through OOXML', async () => {
  const slide = {
    elements: [{
      id: 'native-list',
      type: 'list',
      x: 10, y: 10, w: 50, h: 30,
      items: ['First item', 'Second item'],
      style: { fontSize: 20, color: 'ink' },
    }],
  };
  const scene = normalizeElementSlideToEditableScene(slide, { slideNumber: 3 });
  const list = scene.nodes.find((node) => node.sourceId === 'native-list');

  assert.deepEqual(list.text.map((run) => run.options.bullet), [
    { type: 'bullet' },
    { type: 'bullet' },
  ]);
  const result = await exportEditablePptx({ title: 'Native list' }, [scene]);
  const zip = await openExport(result);
  const slideXml = await zip.file('ppt/slides/slide1.xml').async('string');
  assert.equal((slideXml.match(/<a:buChar\b/g) || []).length, 2);
});

test('bullet schema accepts supported type and indent but rejects unknown fields and values', () => {
  const valid = textScene();
  valid.nodes[0].text = [{
    text: 'Bullet',
    options: { bullet: { type: 'bullet', indent: 12 }, breakLine: false },
  }];
  assert.equal(validateEditableSlideScene(valid), valid);

  for (const bullet of [
    { type: 'number' },
    { type: 'bullet', indent: -1 },
    { type: 'bullet', mystery: true },
    'bullet',
  ]) {
    const scene = textScene(4);
    scene.nodes[0].text = [{ text: 'Invalid', options: { bullet } }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.slideNumber === 4
        && error.sourceId === 'title-4'
        && error.code === 'editable_scene_payload_invalid',
    );
  }
});

test('scene serializer uses stable paintOrder and subOrder without legacy element fields', async () => {
  const calls = [];
  const targetSlide = {
    addText(value) { calls.push(`text:${value}`); },
    addShape(_type, options) { calls.push(`shape:${options.fill?.color}`); },
    addTable() {},
    addImage() {},
  };
  const scene = textScene();
  scene.nodes = [
    {
      type: 'shape', shapeType: 'rect', sourceId: 'later',
      x: 1, y: 1, w: 1, h: 1, paintOrder: 2, subOrder: 0,
      style: { fill: '222222' },
    },
    {
      type: 'shape', shapeType: 'rect', sourceId: 'first',
      x: 1, y: 1, w: 1, h: 1, paintOrder: 1, subOrder: 1,
      style: { fill: '111111' },
    },
    {
      type: 'text', sourceId: 'middle',
      x: 1, y: 1, w: 2, h: 1, text: 'Middle', paintOrder: 1, subOrder: 2,
      style: { fontSize: 12 },
    },
  ];

  await buildSlideFromScene(scene, {
    addSlide: () => targetSlide,
    ShapeType: { rect: 'rect' },
  });
  assert.deepEqual(calls, ['shape:111111', 'text:Middle', 'shape:222222']);
  await assert.rejects(
    buildSlideFromScene({ ...scene, nodes: undefined }, {
      addSlide: () => targetSlide,
      ShapeType: { rect: 'rect' },
    }),
    /nodes/i,
  );
});

test('scene serializer sorts overlapping objects by zIndex then paintOrder then subOrder', async () => {
  const calls = [];
  const targetSlide = {
    addText(value) { calls.push(value); },
    addShape() {},
    addTable() {},
    addImage() {},
  };
  const scene = textScene();
  scene.nodes = [
    {
      type: 'text', sourceId: 'high-z',
      x: 1, y: 1, w: 2, h: 1, text: 'high-z',
      zIndex: 5, paintOrder: 0, subOrder: 0, style: { fontSize: 12 },
    },
    {
      type: 'text', sourceId: 'low-z-late',
      x: 1, y: 1, w: 2, h: 1, text: 'low-z-late',
      zIndex: 1, paintOrder: 2, subOrder: 0, style: { fontSize: 12 },
    },
    {
      type: 'text', sourceId: 'low-z-first',
      x: 1, y: 1, w: 2, h: 1, text: 'low-z-first',
      zIndex: 1, paintOrder: 1, subOrder: 0, style: { fontSize: 12 },
    },
  ];

  await buildSlideFromScene(scene, {
    addSlide: () => targetSlide,
    ShapeType: {},
  });
  assert.deepEqual(calls, ['low-z-first', 'low-z-late', 'high-z']);
});

test('roundRect radius is injected as distinct editable DrawingML adjustment values', async () => {
  const pptx = createPptxDeck({ title: 'Round rect adjustment' });
  await buildSlideFromScene({
    slideNumber: 1,
    width: 13.333,
    height: 7.5,
    nodes: [
      {
        type: 'shape', shapeType: 'roundRect', sourceId: 'radius-eight',
        x: 1, y: 1, w: 2, h: 1, paintOrder: 0, subOrder: 0,
        style: { fill: '112233', radius: 8 / 96 },
      },
      {
        type: 'shape', shapeType: 'roundRect', sourceId: 'radius-four',
        x: 4, y: 1, w: 2, h: 1, paintOrder: 1, subOrder: 0,
        style: { fill: '445566', radius: 4 / 96 },
      },
    ],
  }, pptx);
  const output = await pptx.write({ outputType: 'nodebuffer' });
  const zip = await JSZip.loadAsync(output);
  const slideXml = await zip.file('ppt/slides/slide1.xml').async('string');
  const adjustments = [...slideXml.matchAll(
    /<a:prstGeom prst="roundRect">[\s\S]*?<a:gd name="adj" fmla="val (\d+)"\/>/g,
  )].map((match) => Number(match[1]));

  assert.deepEqual(adjustments, [8333, 4167]);
  assert.doesNotMatch(slideXml, /<p:pic>/);
  assert.equal(zip.file(/^ppt\/media\//).length, 0);
});

test('roundRect adjustment preserves explicit and default geometry in mixed scenes', async () => {
  const pptx = createPptxDeck({ title: 'Mixed round rect adjustment' });
  await buildSlideFromScene({
    slideNumber: 1,
    width: 13.333,
    height: 7.5,
    nodes: [
      {
        type: 'shape', shapeType: 'roundRect', sourceId: 'radius-eight',
        x: 1, y: 1, w: 2, h: 1, paintOrder: 0, subOrder: 0,
        style: { fill: '112233', radius: 8 / 96 },
      },
      {
        type: 'shape', shapeType: 'roundRect', sourceId: 'default-radius',
        x: 3, y: 1, w: 2, h: 1, paintOrder: 1, subOrder: 0,
        style: { fill: '334455' },
      },
      {
        type: 'shape', shapeType: 'rect', sourceId: 'plain-rect',
        x: 5, y: 1, w: 2, h: 1, paintOrder: 2, subOrder: 0,
        style: { fill: '556677' },
      },
      {
        type: 'shape', shapeType: 'roundRect', sourceId: 'radius-four',
        x: 7, y: 1, w: 2, h: 1, paintOrder: 3, subOrder: 0,
        style: { fill: '778899', radius: 4 / 96 },
      },
    ],
  }, pptx);
  const output = await pptx.write({ outputType: 'nodebuffer' });
  const zip = await JSZip.loadAsync(output);
  const slideXml = await zip.file('ppt/slides/slide1.xml').async('string');
  const adjustments = [...slideXml.matchAll(
    /<a:prstGeom prst="roundRect">[\s\S]*?<a:gd name="adj" fmla="val (\d+)"\/>/g,
  )].map((match) => Number(match[1]));
  assert.deepEqual(adjustments, [8333, 16667, 4167]);
});

test('intentional images remain pictures while generated geometry never creates media', async () => {
  const imageScene = textScene();
  imageScene.nodes.push({
    type: 'image',
    intent: 'user-image',
    sourceId: 'photo',
    x: 6,
    y: 1,
    w: 1,
    h: 1,
    paintOrder: 1,
    subOrder: 0,
    src: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
  });
  const result = await exportEditablePptx({ title: 'Intentional image' }, [imageScene]);
  const zip = await openExport(result);
  const slideXml = await zip.file('ppt/slides/slide1.xml').async('string');
  assert.match(slideXml, /<p:pic>/);
  assert.equal(zip.file(/^ppt\/media\//).length, 1);
});

test('editable scenes reject GIF because static content cannot be proven', () => {
  const scene = textScene();
  scene.nodes = [{
    type: 'image',
    intent: 'user-image',
    sourceId: 'gif-image',
    x: 1, y: 1, w: 1, h: 1,
    src: 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==',
  }];

  assert.throws(
    () => validateEditableSlideScene(scene),
    (error) => error instanceof EditableExportError
      && error.code === 'editable_scene_payload_invalid'
      && error.sourceId === 'gif-image',
  );
});

test('editable scenes accept only base64 PNG JPEG and WebP intentional images', () => {
  for (const mime of ['png', 'jpeg', 'webp']) {
    const scene = textScene();
    scene.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId: `${mime}-image`,
      x: 1, y: 1, w: 1, h: 1,
      src: VALID_IMAGE_DATA[mime],
    }];
    assert.equal(validateEditableSlideScene(scene), scene);
  }
});

test('editable scenes require complete PNG JPEG and WebP containers', () => {
  for (const [mime, src] of Object.entries(VALID_IMAGE_DATA)) {
    const valid = textScene();
    valid.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId: `${mime}-complete`,
      x: 1, y: 1, w: 1, h: 1,
      src,
    }];
    assert.equal(validateEditableSlideScene(valid), valid, mime);

    const bytes = Buffer.from(src.slice(src.indexOf(',') + 1), 'base64');
    const truncated = `data:image/${mime};base64,${bytes.subarray(0, bytes.length - 2).toString('base64')}`;
    const invalid = textScene();
    invalid.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId: `${mime}-truncated`,
      x: 1, y: 1, w: 1, h: 1,
      src: truncated,
    }];
    assert.throws(
      () => validateEditableSlideScene(invalid),
      (error) => error instanceof EditableExportError
        && error.sourceId === `${mime}-truncated`
        && error.code === 'editable_scene_payload_invalid',
      mime,
    );
  }
});

test('editable scenes reject malformed PNG IHDR and WebP image chunks with valid boundaries', () => {
  const malformed = [];
  const png = Buffer.from(VALID_IMAGE_DATA.png.slice(VALID_IMAGE_DATA.png.indexOf(',') + 1), 'base64');
  png[26] = 1;
  malformed.push(['png-invalid-compression', `data:image/png;base64,${png.toString('base64')}`]);

  const webp = Buffer.from(VALID_IMAGE_DATA.webp.slice(VALID_IMAGE_DATA.webp.indexOf(',') + 1), 'base64');
  webp[20] = 0;
  malformed.push(['webp-invalid-vp8l', `data:image/webp;base64,${webp.toString('base64')}`]);

  for (const [sourceId, src] of malformed) {
    const scene = textScene();
    scene.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId,
      x: 1, y: 1, w: 1, h: 1,
      src,
    }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.sourceId === sourceId
        && error.code === 'editable_scene_payload_invalid',
      sourceId,
    );
  }
});

test('editable scenes require non-empty PNG IDAT and WebP image data after VP8X', () => {
  const png = Buffer.from(VALID_IMAGE_DATA.png.slice(VALID_IMAGE_DATA.png.indexOf(',') + 1), 'base64');
  const pngWithoutIdat = [png.subarray(0, 8)];
  for (let offset = 8; offset < png.length;) {
    const length = png.readUInt32BE(offset);
    const end = offset + 12 + length;
    if (png.toString('ascii', offset + 4, offset + 8) !== 'IDAT') {
      pngWithoutIdat.push(png.subarray(offset, end));
    }
    offset = end;
  }

  const vp8xOnly = Buffer.alloc(30);
  vp8xOnly.write('RIFF', 0, 'ascii');
  vp8xOnly.writeUInt32LE(22, 4);
  vp8xOnly.write('WEBP', 8, 'ascii');
  vp8xOnly.write('VP8X', 12, 'ascii');
  vp8xOnly.writeUInt32LE(10, 16);

  for (const [sourceId, src] of [
    ['png-without-idat', `data:image/png;base64,${Buffer.concat(pngWithoutIdat).toString('base64')}`],
    ['webp-vp8x-only', `data:image/webp;base64,${vp8xOnly.toString('base64')}`],
  ]) {
    const scene = textScene();
    scene.nodes = [{
      type: 'image', intent: 'user-image', sourceId,
      x: 1, y: 1, w: 1, h: 1, src,
    }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.sourceId === sourceId
        && error.code === 'editable_scene_payload_invalid',
      sourceId,
    );
  }
});

test('editable scenes verify PNG JPEG and WebP signatures instead of trusting MIME', () => {
  const invalidSources = [
    'data:image/png;base64,/9j/',
    'data:image/jpeg;base64,iVBORw0KGgo=',
    'data:image/webp;base64,UklGRgAAAABOT1BFAAAA',
    'data:image/png;base64,AA==',
  ];
  invalidSources.forEach((src, index) => {
    const scene = textScene();
    scene.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId: `bad-signature-${index}`,
      x: 1, y: 1, w: 1, h: 1,
      src,
    }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.slideNumber === 1
        && error.sourceId === `bad-signature-${index}`
        && error.code === 'editable_scene_payload_invalid',
    );
  });
});

test('editable scene strictly validates text shape and line styles and unknown fields', () => {
  const invalidNodes = [
    {
      type: 'text', sourceId: 'negative-font', x: 1, y: 1, w: 2, h: 1,
      text: 'Text', style: { fontSize: -1 },
    },
    {
      type: 'text', sourceId: 'bad-text-color', x: 1, y: 1, w: 2, h: 1,
      text: 'Text', style: { color: 'not-a-color' },
    },
    {
      type: 'text', sourceId: 'unknown-text-style', x: 1, y: 1, w: 2, h: 1,
      text: 'Text', style: { fontSize: 12, glow: { color: 'FFFFFF' } },
    },
    {
      type: 'line', sourceId: 'negative-line-width', x1: 1, y1: 1, x2: 2, y2: 2,
      style: { color: '112233', width: -1 },
    },
    {
      type: 'line', sourceId: 'unknown-dash', x1: 1, y1: 1, x2: 2, y2: 2,
      style: { color: '112233', width: 1, dash: 'morse' },
    },
    {
      type: 'shape', sourceId: 'bad-shape-fill', x: 1, y: 1, w: 2, h: 1,
      shapeType: 'rect', style: { fill: '#112233' },
    },
    {
      type: 'shape', sourceId: 'negative-radius', x: 1, y: 1, w: 2, h: 1,
      shapeType: 'roundRect', style: { fill: '112233', radius: -0.1 },
    },
    {
      type: 'shape', sourceId: 'unknown-effect', x: 1, y: 1, w: 2, h: 1,
      shapeType: 'rect', style: { fill: '112233', effect: 'glow' },
    },
    {
      type: 'shape', sourceId: 'unknown-node-field', x: 1, y: 1, w: 2, h: 1,
      shapeType: 'rect', style: { fill: '112233' }, mystery: true,
    },
  ];

  for (const node of invalidNodes) {
    assert.throws(
      () => validateEditableSlideScene({
        slideNumber: 8,
        width: 13.333,
        height: 7.5,
        nodes: [node],
      }),
      (error) => error instanceof EditableExportError
        && error.slideNumber === 8
        && error.sourceId === node.sourceId
        && error.code === 'editable_scene_payload_invalid',
      node.sourceId,
    );
  }
});

test('editable scene closes top-level common table cell image and ordering schemas', () => {
  const tableNode = {
    type: 'table',
    sourceId: 'strict-table',
    x: 1, y: 1, w: 2, h: 1,
    columnWidths: [2],
    rows: [{
      height: 1,
      cells: [{
        text: 'A',
        style: {
          fill: 'FFFFFF',
          border: { color: '111111', width: 1 },
          align: 'left',
        },
      }],
    }],
  };
  const cases = [
    { ...textScene(), mystery: true },
    { ...textScene(), nodes: [{ ...textScene().nodes[0], zIndex: 'front' }] },
    { ...textScene(), nodes: [{ ...textScene().nodes[0], paintOrder: Number.NaN }] },
    { ...textScene(), nodes: [{ ...textScene().nodes[0], subOrder: {} }] },
    { ...textScene(), nodes: [{ ...tableNode, mystery: true }] },
    { ...textScene(), nodes: [{ ...tableNode, rows: [{ ...tableNode.rows[0], mystery: true }] }] },
    {
      ...textScene(),
      nodes: [{
        ...tableNode,
        rows: [{
          ...tableNode.rows[0],
          cells: [{ ...tableNode.rows[0].cells[0], mystery: true }],
        }],
      }],
    },
    {
      ...textScene(),
      nodes: [{
        ...tableNode,
        rows: [{
          ...tableNode.rows[0],
          cells: [{
            ...tableNode.rows[0].cells[0],
            style: { ...tableNode.rows[0].cells[0].style, mystery: true },
          }],
        }],
      }],
    },
    {
      ...textScene(),
      nodes: [{
        type: 'image', intent: 'user-image', sourceId: 'strict-image',
        x: 1, y: 1, w: 1, h: 1, src: VALID_IMAGE_DATA.png, mystery: true,
      }],
    },
  ];

  for (const scene of cases) {
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError,
    );
  }
});

test('table cell text runs reuse the closed text options schema', () => {
  const sceneForCellText = (options) => ({
    slideNumber: 1,
    width: 13.333,
    height: 7.5,
    nodes: [{
      type: 'table', sourceId: 'table-runs',
      x: 1, y: 1, w: 2, h: 1,
      columnWidths: [2],
      rows: [{
        height: 1,
        cells: [{
          text: [{ text: 'Cell', options }],
          style: {
            fill: null,
            border: { color: '111111', width: 1 },
            align: 'left',
          },
        }],
      }],
    }],
  });

  const valid = sceneForCellText({ bold: true, color: '112233' });
  assert.equal(validateEditableSlideScene(valid), valid);
  for (const options of [
    { bold: true, mystery: 'lost' },
    { fontSize: -1 },
    { bullet: { type: 'number' } },
  ]) {
    assert.throws(
      () => validateEditableSlideScene(sceneForCellText(options)),
      (error) => error instanceof EditableExportError
        && error.sourceId === 'table-runs'
        && error.code === 'editable_scene_payload_invalid',
    );
  }
});

test('editable shape shadow schema accepts native outer shadows and rejects invalid values', () => {
  const valid = textScene();
  valid.nodes = [{
    type: 'shape', sourceId: 'valid-shadow', shapeType: 'rect',
    x: 1, y: 1, w: 2, h: 1,
    style: {
      fill: 'FFFFFF',
      shadow: {
        type: 'outer',
        angle: 45,
        blur: 6,
        color: '112233',
        offset: 4,
        opacity: 0.4,
      },
    },
  }];
  assert.equal(validateEditableSlideScene(valid), valid);

  for (const [sourceId, shadow] of [
    ['unknown-shadow-field', {
      type: 'outer', angle: 45, blur: 6, color: '112233', offset: 4, opacity: 0.4,
      spread: 2,
    }],
    ['invalid-shadow-type', {
      type: 'inner', angle: 45, blur: 6, color: '112233', offset: 4, opacity: 0.4,
    }],
    ['invalid-shadow-angle', {
      type: 'outer', angle: 360, blur: 6, color: '112233', offset: 4, opacity: 0.4,
    }],
    ['invalid-shadow-opacity', {
      type: 'outer', angle: 45, blur: 6, color: '112233', offset: 4, opacity: 2,
    }],
  ]) {
    const scene = textScene();
    scene.nodes = [{
      type: 'shape', sourceId, shapeType: 'rect',
      x: 1, y: 1, w: 2, h: 1,
      style: { fill: 'FFFFFF', shadow },
    }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.code === 'editable_scene_payload_invalid'
        && error.sourceId === sourceId,
    );
  }
});

test('image serialization failures are wrapped with located EditableExportError cause', async () => {
  const cause = new Error('pptxgen image decoder failed');
  const scene = textScene(6);
  scene.nodes = [{
    type: 'image',
    intent: 'user-image',
    sourceId: 'broken-photo',
    x: 1, y: 1, w: 1, h: 1,
    src: VALID_IMAGE_DATA.png,
  }];
  const targetSlide = {
    addImage() { throw cause; },
    addShape() {},
    addTable() {},
    addText() {},
  };

  await assert.rejects(
    buildSlideFromScene(scene, {
      addSlide: () => targetSlide,
      ShapeType: {},
    }),
    (error) => error instanceof EditableExportError
      && error.slideNumber === 6
      && error.sourceId === 'broken-photo'
      && error.code === 'pptx_image_serialization'
      && error.cause === cause,
  );
});

test('editable scenes reject unsafe or non-raster intentional image sources', () => {
  const invalidSources = [
    { src: 'data:image/svg+xml;base64,PHN2Zy8+' },
    { src: 'data:text/plain;base64,SGVsbG8=' },
    { src: 'data:image/png,not-base64' },
    { src: 'https://example.invalid/photo.png' },
    { src: 'http://example.invalid/photo.png' },
    { src: 'file:///tmp/photo.png' },
    { src: 'blob:unsafe' },
    { path: '/tmp/photo.png' },
    { data: 'data:application/octet-stream;base64,AA==' },
  ];
  invalidSources.forEach((source, index) => {
    const scene = textScene();
    scene.nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId: `unsafe-image-${index}`,
      x: 1, y: 1, w: 1, h: 1,
      ...source,
    }];
    assert.throws(
      () => validateEditableSlideScene(scene),
      (error) => error instanceof EditableExportError
        && error.code === 'editable_scene_payload_invalid'
        && error.sourceId === `unsafe-image-${index}`,
    );
  });
});

test('element-model normalization blocks video unknown types and incomplete payloads', () => {
  const invalidElements = [
    { id: 'video', type: 'video', x: 1, y: 1, w: 10, h: 10, src: 'data:video/mp4;base64,AA==' },
    { id: 'unknown', type: 'mystery', x: 1, y: 1, w: 10, h: 10, text: 'Unknown' },
    { id: 'image-missing', type: 'image', x: 1, y: 1, w: 10, h: 10 },
    { id: 'metric-missing', type: 'metric', x: 1, y: 1, w: 10, h: 10 },
    { id: 'list-missing', type: 'list', x: 1, y: 1, w: 10, h: 10, items: [] },
  ];
  invalidElements.forEach((element) => {
    assert.throws(
      () => normalizeElementSlideToEditableScene({
        elements: [element],
      }, { slideNumber: 7 }),
      (error) => error instanceof EditableExportError
        && error.slideNumber === 7
        && error.sourceId === element.id,
      element.id,
    );
  });
});
