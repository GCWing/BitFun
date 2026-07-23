const EDITABLE_NODE_TYPES = new Set([
  'text',
  'shape',
  'line',
  'table',
  'image',
]);

const EDITABLE_SHAPE_TYPES = new Set([
  'rect',
  'roundRect',
  'ellipse',
  'triangle',
  'diamond',
  'rightArrow',
  'leftArrow',
  'upArrow',
  'downArrow',
  'chevron',
  'parallelogram',
  'trapezoid',
  'hexagon',
]);

const FORBIDDEN_FALLBACK_FIELDS = [
  'captureStrategy',
  'phase',
  'canvas',
  'sourceIds',
  'buildFailure',
  'diagnostics',
  'html',
  'buildHtml',
  'rasterHtml',
  'rasterBase64',
  'rasterOnly',
  'renderRaster',
];

const LEGACY_FALLBACK_KINDS = new Set([
  'raster',
  'svg-image',
  'fallback',
  'page-visual',
  'full-page',
]);

export class EditableExportError extends Error {
  constructor(diagnostic) {
    const blockingDiagnostic = {
      severity: 'blocking',
      kind: 'blocking',
      ...diagnostic,
    };
    super(`[${blockingDiagnostic.code}] ${blockingDiagnostic.message}`);
    this.name = 'EditableExportError';
    this.diagnostic = blockingDiagnostic;
    this.diagnostics = [blockingDiagnostic];
    this.slideNumber = blockingDiagnostic.slideNumber;
    this.sourceId = blockingDiagnostic.sourceId;
    this.code = blockingDiagnostic.code;
    if (blockingDiagnostic.cause !== undefined) this.cause = blockingDiagnostic.cause;
  }
}

function sceneSourceId(slideNumber) {
  return Number.isInteger(slideNumber) && slideNumber > 0
    ? `slide-${slideNumber}`
    : 'slide';
}

function editableSceneError(scene, code, message, sourceId = null) {
  const slideNumber = scene?.slideNumber ?? null;
  return new EditableExportError({
    slideNumber,
    sourceId: sourceId || sceneSourceId(slideNumber),
    code,
    message,
  });
}

function isObject(value) {
  return Boolean(value) && !Array.isArray(value) && typeof value === 'object';
}

function hasOnlyFields(value, fields) {
  return isObject(value) && Object.keys(value).every((field) => fields.has(field));
}

function isColor(value) {
  return typeof value === 'string' && /^[0-9A-F]{6}$/i.test(value);
}

function isOptionalColor(value) {
  return value === undefined || value === null || isColor(value);
}

function isNonNegativeFinite(value) {
  return Number.isFinite(value) && value >= 0;
}

function isPercentage(value) {
  return Number.isFinite(value) && value >= 0 && value <= 100;
}

const TEXT_STYLE_FIELDS = new Set([
  'fontSize', 'fontFace', 'color', 'bold', 'italic', 'underline',
  'align', 'valign', 'lineSpacing', 'paraSpaceBefore', 'paraSpaceAfter',
  'charSpacing', 'margin', 'rotate', 'vert', 'transparency', 'bulletColor',
]);
const TEXT_RUN_OPTION_FIELDS = new Set([
  'fontSize', 'fontFace', 'color', 'bold', 'italic', 'underline',
  'breakLine', 'bullet', 'transparency', 'charSpacing',
]);
const BULLET_FIELDS = new Set(['type', 'indent']);
const LINE_STYLE_FIELDS = new Set([
  'color', 'width', 'dash', 'dashType', 'transparency',
]);
const SHAPE_STYLE_FIELDS = new Set([
  'fill', 'line', 'transparency', 'rotate', 'radius', 'shadow',
]);
const SHADOW_FIELDS = new Set(['type', 'angle', 'blur', 'color', 'offset', 'opacity']);
const TABLE_NODE_FIELDS = new Set([
  'type', 'sourceId', 'x', 'y', 'w', 'h', 'columnWidths', 'rows',
  'paintOrder', 'subOrder', 'zIndex',
]);
const TABLE_ROW_FIELDS = new Set(['height', 'cells']);
const TABLE_CELL_FIELDS = new Set(['text', 'style', 'rowspan', 'colspan']);
const TABLE_CELL_STYLE_FIELDS = new Set([
  'fill', 'border', 'align', 'valign', 'padding',
  'fontFamily', 'fontSize', 'fontColor', 'bold',
]);
const TABLE_BORDER_FIELDS = new Set(['color', 'width']);
const TABLE_BORDER_SIDES = new Set(['top', 'right', 'bottom', 'left']);
const IMAGE_NODE_FIELDS = new Set([
  'type', 'intent', 'sourceId', 'x', 'y', 'w', 'h', 'src', 'data',
  'paintOrder', 'subOrder', 'zIndex',
]);
const SCENE_FIELDS = new Set(['slideNumber', 'width', 'height', 'nodes']);
const REWRITE_TYPES = new Set(['css_gradient', 'svg_path_rewrite', 'css_box_shadow_ring']);
const DASH_TYPES = new Set(['dash', 'dot', 'dashDot']);
const TEXT_ALIGNS = new Set(['left', 'center', 'right', 'justify']);
const TEXT_VALIGNS = new Set(['top', 'mid', 'bottom']);
const TEXT_VERTS = new Set(['horz', 'vert', 'vert270', 'wordArtVert', 'eaVert', 'mongolianVert']);

function hasValidMargin(value) {
  return value === undefined
    || isNonNegativeFinite(value)
    || (Array.isArray(value)
      && value.length === 4
      && value.every(isNonNegativeFinite));
}

function hasValidTextStyle(style) {
  return hasOnlyFields(style, TEXT_STYLE_FIELDS)
    && (style.fontSize === undefined || (Number.isFinite(style.fontSize) && style.fontSize > 0))
    && (style.fontFace === undefined
      || (typeof style.fontFace === 'string' && Boolean(style.fontFace.trim())))
    && isOptionalColor(style.color)
    && isOptionalColor(style.bulletColor)
    && ['bold', 'italic'].every((field) => (
      style[field] === undefined || typeof style[field] === 'boolean'
    ))
    && (style.underline === undefined
      || typeof style.underline === 'boolean'
      || typeof style.underline === 'string')
    && (style.align === undefined || TEXT_ALIGNS.has(style.align))
    && (style.valign === undefined || TEXT_VALIGNS.has(style.valign))
    && ['lineSpacing', 'paraSpaceBefore', 'paraSpaceAfter'].every((field) => (
      style[field] === undefined || isNonNegativeFinite(style[field])
    ))
    && (style.charSpacing === undefined || Number.isFinite(style.charSpacing))
    && hasValidMargin(style.margin)
    && (style.rotate === undefined || Number.isFinite(style.rotate))
    && (style.vert === undefined || TEXT_VERTS.has(style.vert))
    && (style.transparency === undefined || style.transparency === null
      || isPercentage(style.transparency));
}

function hasValidTextRunOptions(options) {
  if (options === undefined) return true;
  return hasOnlyFields(options, TEXT_RUN_OPTION_FIELDS)
    && (options.fontSize === undefined || (Number.isFinite(options.fontSize) && options.fontSize > 0))
    && (options.fontFace === undefined
      || (typeof options.fontFace === 'string' && Boolean(options.fontFace.trim())))
    && isOptionalColor(options.color)
    && ['bold', 'italic', 'breakLine'].every((field) => (
      options[field] === undefined || typeof options[field] === 'boolean'
    ))
    && (options.underline === undefined
      || typeof options.underline === 'boolean'
      || typeof options.underline === 'string')
    && (options.transparency === undefined || isPercentage(options.transparency))
    && (options.charSpacing === undefined || Number.isFinite(options.charSpacing))
    && (options.bullet === undefined || (
      hasOnlyFields(options.bullet, BULLET_FIELDS)
      && (options.bullet.type === undefined || options.bullet.type === 'bullet')
      && (options.bullet.indent === undefined || isNonNegativeFinite(options.bullet.indent))
      && (options.bullet.type === 'bullet' || options.bullet.indent !== undefined)
    ));
}

function hasValidTextValue(value, allowEmpty = false) {
  if (!isTextValue(value, allowEmpty)) return false;
  return !Array.isArray(value) || value.every((run) => hasValidTextRunOptions(run.options));
}

function hasValidLineStyle(style, { allowNull = false } = {}) {
  if (allowNull && (style === null || style === undefined)) return true;
  return hasOnlyFields(style, LINE_STYLE_FIELDS)
    && isColor(style.color)
    && (style.width === undefined || isNonNegativeFinite(style.width))
    && (style.dash === undefined || DASH_TYPES.has(style.dash))
    && (style.dashType === undefined || DASH_TYPES.has(style.dashType))
    && (style.dash === undefined || style.dashType === undefined || style.dash === style.dashType)
    && (style.transparency === undefined || isPercentage(style.transparency));
}

function hasValidShapeStyle(style, shapeType) {
  return hasOnlyFields(style, SHAPE_STYLE_FIELDS)
    && isOptionalColor(style.fill)
    && hasValidLineStyle(style.line, { allowNull: true })
    && (style.transparency === undefined || style.transparency === null
      || isPercentage(style.transparency))
    && (style.rotate === undefined || Number.isFinite(style.rotate))
    && (style.radius === undefined || (
      shapeType === 'roundRect'
      && Number.isFinite(style.radius)
      && style.radius > 0
    ))
    && (style.shadow === undefined || hasValidShadow(style.shadow));
}

function hasValidShadow(shadow) {
  return hasOnlyFields(shadow, SHADOW_FIELDS)
    && shadow.type === 'outer'
    && Number.isFinite(shadow.angle)
    && shadow.angle >= 0
    && shadow.angle < 360
    && isNonNegativeFinite(shadow.blur)
    && isColor(shadow.color)
    && isNonNegativeFinite(shadow.offset)
    && Number.isFinite(shadow.opacity)
    && shadow.opacity >= 0
    && shadow.opacity <= 1;
}

function findForbiddenFallbackMetadata(
  value,
  {
    allowDirectData = false,
    skipKeys = new Set(),
    seen = new Set(),
  } = {},
) {
  if (!value || typeof value !== 'object' || seen.has(value)) return null;
  seen.add(value);

  if (Array.isArray(value)) {
    for (const item of value) {
      const nestedField = findForbiddenFallbackMetadata(item, { seen });
      if (nestedField) return nestedField;
    }
    return null;
  }

  for (const [field, nestedValue] of Object.entries(value)) {
    if (skipKeys.has(field)) continue;
    const normalizedField = field.toLowerCase();
    if (normalizedField.includes('fallback')
      || normalizedField === ['suppressed', 'native', 'visual', 'ids'].join('')
      || FORBIDDEN_FALLBACK_FIELDS.includes(field)) return field;
    if (field === 'data' && !allowDirectData) return field;
    if (field === 'kind' && LEGACY_FALLBACK_KINDS.has(nestedValue)) return field;

    const nestedField = findForbiddenFallbackMetadata(nestedValue, { seen });
    if (nestedField) return nestedField;
  }
  return null;
}

function isTextValue(value, allowEmpty = false) {
  if (typeof value === 'string') return allowEmpty || Boolean(value.trim());
  if (!Array.isArray(value) || !value.length) return false;
  if (!value.every((run) => (
    hasOnlyFields(run, new Set(['text', 'options']))
    && typeof run.text === 'string'
    && (run.options === undefined || isObject(run.options))
  ))) return false;
  return allowEmpty || Boolean(value.map((run) => run.text).join('').trim());
}

function hasValidBoxGeometry(node, { allowNegativeOrigin = false } = {}) {
  return Number.isFinite(node.x)
    && Number.isFinite(node.y)
    && Number.isFinite(node.w)
    && Number.isFinite(node.h)
    && (allowNegativeOrigin || node.x >= 0)
    && (allowNegativeOrigin || node.y >= 0)
    && node.w > 0
    && node.h > 0;
}

function hasValidLineGeometry(node, { allowNegativeOrigin = false } = {}) {
  return Number.isFinite(node.x1)
    && Number.isFinite(node.y1)
    && Number.isFinite(node.x2)
    && Number.isFinite(node.y2)
    && (allowNegativeOrigin || node.x1 >= 0)
    && (allowNegativeOrigin || node.y1 >= 0)
    && (allowNegativeOrigin || node.x2 >= 0)
    && (allowNegativeOrigin || node.y2 >= 0)
    && (node.x1 !== node.x2 || node.y1 !== node.y2);
}

function isPositiveInteger(value) {
  return Number.isInteger(value) && value > 0;
}

function hasValidTableBorderSide(side) {
  return hasOnlyFields(side, TABLE_BORDER_FIELDS)
    && /^[0-9A-F]{6}$/i.test(side.color)
    && Number.isFinite(side.width)
    && side.width >= 0;
}

function hasValidTableBorder(border) {
  if (!isObject(border)) return false;
  if (hasOnlyFields(border, TABLE_BORDER_SIDES)
    && [...TABLE_BORDER_SIDES].every((side) => hasValidTableBorderSide(border[side]))) {
    return true;
  }
  return hasOnlyFields(border, TABLE_BORDER_FIELDS)
    && /^[0-9A-F]{6}$/i.test(border.color)
    && Number.isFinite(border.width)
    && border.width >= 0;
}

function hasValidTableCellStyle(style) {
  return hasOnlyFields(style, TABLE_CELL_STYLE_FIELDS)
    && (style.fill === null || /^[0-9A-F]{6}$/i.test(style.fill))
    && hasValidTableBorder(style.border)
    && ['left', 'center', 'right'].includes(style.align)
    && (style.valign === undefined || ['top', 'mid', 'bottom'].includes(style.valign))
    && (style.padding === undefined || (
      Array.isArray(style.padding)
      && style.padding.length === 4
      && style.padding.every((value) => Number.isFinite(value) && value >= 0)
    ))
    && (style.fontFamily === undefined || (
      typeof style.fontFamily === 'string' && Boolean(style.fontFamily.trim())
    ))
    && (style.fontSize === undefined || (Number.isFinite(style.fontSize) && style.fontSize > 0))
    && (style.fontColor === undefined || /^[0-9A-F]{6}$/i.test(style.fontColor))
    && (style.bold === undefined || typeof style.bold === 'boolean');
}

function hasValidTableCell(cell) {
  return hasOnlyFields(cell, TABLE_CELL_FIELDS)
    && Object.prototype.hasOwnProperty.call(cell, 'text')
    && hasValidTextValue(cell.text, true)
    && hasValidTableCellStyle(cell.style)
    && (cell.rowspan === undefined || isPositiveInteger(cell.rowspan))
    && (cell.colspan === undefined || isPositiveInteger(cell.colspan));
}

function hasValidTableGrid(rows, columnCount) {
  let activeRowspans = Array(columnCount).fill(0);

  for (const row of rows) {
    const occupied = activeRowspans.map((remaining) => remaining > 0);
    const nextRowspans = activeRowspans.map((remaining) => Math.max(0, remaining - 1));

    for (const cell of row.cells) {
      const colspan = cell.colspan || 1;
      const rowspan = cell.rowspan || 1;
      const startColumn = occupied.indexOf(false);
      if (startColumn < 0 || startColumn + colspan > columnCount) return false;

      for (let column = startColumn; column < startColumn + colspan; column += 1) {
        if (occupied[column]) return false;
        occupied[column] = true;
        if (rowspan > 1) nextRowspans[column] = Math.max(nextRowspans[column], rowspan - 1);
      }
    }

    if (occupied.some((isOccupied) => !isOccupied)) return false;
    activeRowspans = nextRowspans;
  }

  return activeRowspans.every((remaining) => remaining === 0);
}

function hasValidTablePayload(node) {
  if (!Array.isArray(node.columnWidths)
    || !node.columnWidths.length
    || !node.columnWidths.every((width) => Number.isFinite(width) && width > 0)
    || !Array.isArray(node.rows)
    || !node.rows.length) {
    return false;
  }

  const hasValidRows = node.rows.every((row) => (
      hasOnlyFields(row, TABLE_ROW_FIELDS)
      && Array.isArray(row.cells)
      && row.cells.length > 0
      && row.cells.every(hasValidTableCell)
      && Number.isFinite(row.height)
      && row.height > 0
  ));
  if (!hasValidRows || !hasValidTableGrid(node.rows, node.columnWidths.length)) return false;
  const nearlyEqual = (left, right) => (
    Math.abs(left - right) <= Math.max(0.001, Math.abs(right) * 0.0001)
  );
  const totalWidth = node.columnWidths.reduce((sum, width) => sum + width, 0);
  const totalHeight = node.rows.reduce((sum, row) => sum + row.height, 0);
  return nearlyEqual(totalWidth, node.w) && nearlyEqual(totalHeight, node.h);
}

function readUint32BigEndian(bytes, offset) {
  return (((bytes[offset] << 24) >>> 0)
    + (bytes[offset + 1] << 16)
    + (bytes[offset + 2] << 8)
    + bytes[offset + 3]) >>> 0;
}

function readUint32LittleEndian(bytes, offset) {
  return (bytes[offset]
    + (bytes[offset + 1] << 8)
    + (bytes[offset + 2] << 16)
    + ((bytes[offset + 3] << 24) >>> 0)) >>> 0;
}

function bytesEqualAscii(bytes, offset, value) {
  return [...value].every((character, index) => (
    bytes[offset + index] === character.charCodeAt(0)
  ));
}

function pngCrc32(bytes, start, end) {
  let crc = 0xFFFFFFFF;
  for (let index = start; index < end; index += 1) {
    crc ^= bytes[index];
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ ((crc & 1) ? 0xEDB88320 : 0);
    }
  }
  return (crc ^ 0xFFFFFFFF) >>> 0;
}

function hasValidPngContainer(bytes) {
  const signature = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
  if (bytes.length < 45 || !signature.every((byte, index) => bytes[index] === byte)) {
    return false;
  }
  let offset = 8;
  let chunkIndex = 0;
  let foundIdat = false;
  let idatClosed = false;
  let foundIend = false;
  while (offset < bytes.length) {
    if (offset + 12 > bytes.length) return false;
    const length = readUint32BigEndian(bytes, offset);
    const typeOffset = offset + 4;
    const dataOffset = offset + 8;
    const chunkEnd = dataOffset + length + 4;
    if (!Number.isSafeInteger(chunkEnd) || chunkEnd > bytes.length) return false;
    const type = String.fromCharCode(...bytes.slice(typeOffset, typeOffset + 4));
    if (pngCrc32(bytes, typeOffset, dataOffset + length)
      !== readUint32BigEndian(bytes, dataOffset + length)) return false;
    if (chunkIndex === 0) {
      if (type !== 'IHDR' || length !== 13) return false;
      const width = readUint32BigEndian(bytes, dataOffset);
      const height = readUint32BigEndian(bytes, dataOffset + 4);
      if (width === 0 || height === 0) return false;
      const bitDepth = bytes[dataOffset + 8];
      const colorType = bytes[dataOffset + 9];
      const validDepths = {
        0: new Set([1, 2, 4, 8, 16]),
        2: new Set([8, 16]),
        3: new Set([1, 2, 4, 8]),
        4: new Set([8, 16]),
        6: new Set([8, 16]),
      };
      if (!validDepths[colorType]?.has(bitDepth)
        || bytes[dataOffset + 10] !== 0
        || bytes[dataOffset + 11] !== 0
        || ![0, 1].includes(bytes[dataOffset + 12])) return false;
    } else if (type === 'IHDR') {
      return false;
    }
    if (type === 'PLTE' && foundIdat) return false;
    if (type === 'IDAT') {
      if (length === 0 || idatClosed) return false;
      foundIdat = true;
    } else if (foundIdat && type !== 'IEND') {
      idatClosed = true;
    }
    offset = chunkEnd;
    chunkIndex += 1;
    if (type === 'IEND') {
      if (length !== 0 || !foundIdat || offset !== bytes.length) return false;
      foundIend = true;
      break;
    }
  }
  return foundIend;
}

const JPEG_SOF_MARKERS = new Set([
  0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
  0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
]);

function hasValidJpegContainer(bytes) {
  if (bytes.length < 8 || bytes[0] !== 0xFF || bytes[1] !== 0xD8) return false;
  let offset = 2;
  let foundSof = false;
  let foundScan = false;
  while (offset < bytes.length) {
    if (bytes[offset] !== 0xFF) return false;
    while (offset < bytes.length && bytes[offset] === 0xFF) offset += 1;
    if (offset >= bytes.length) return false;
    const marker = bytes[offset];
    offset += 1;
    if (marker === 0xD9) return foundSof && foundScan && offset === bytes.length;
    if (marker === 0x00 || marker === 0xD8 || (marker >= 0xD0 && marker <= 0xD7)) return false;
    if (offset + 2 > bytes.length) return false;
    const segmentLength = (bytes[offset] << 8) + bytes[offset + 1];
    if (segmentLength < 2 || offset + segmentLength > bytes.length) return false;
    if (JPEG_SOF_MARKERS.has(marker)) {
      if (segmentLength < 8) return false;
      foundSof = true;
    }
    offset += segmentLength;
    if (marker !== 0xDA) continue;
    foundScan = true;
    while (offset < bytes.length) {
      if (bytes[offset] !== 0xFF) {
        offset += 1;
        continue;
      }
      let markerOffset = offset + 1;
      while (markerOffset < bytes.length && bytes[markerOffset] === 0xFF) markerOffset += 1;
      if (markerOffset >= bytes.length) return false;
      const scanMarker = bytes[markerOffset];
      if (scanMarker === 0x00 || (scanMarker >= 0xD0 && scanMarker <= 0xD7)) {
        offset = markerOffset + 1;
        continue;
      }
      break;
    }
  }
  return false;
}

function hasValidWebpContainer(bytes) {
  if (bytes.length < 30
    || !bytesEqualAscii(bytes, 0, 'RIFF')
    || !bytesEqualAscii(bytes, 8, 'WEBP')
    || readUint32LittleEndian(bytes, 4) + 8 !== bytes.length) {
    return false;
  }
  let offset = 12;
  let foundImageDataChunk = false;
  let foundExtendedHeader = false;
  let chunkIndex = 0;
  while (offset < bytes.length) {
    if (offset + 8 > bytes.length) return false;
    const chunkType = String.fromCharCode(...bytes.slice(offset, offset + 4));
    const chunkLength = readUint32LittleEndian(bytes, offset + 4);
    const dataEnd = offset + 8 + chunkLength;
    const paddedEnd = dataEnd + (chunkLength % 2);
    if (!Number.isSafeInteger(paddedEnd) || paddedEnd > bytes.length) return false;
    if (['VP8 ', 'VP8L', 'VP8X'].includes(chunkType)) {
      const dataOffset = offset + 8;
      const validImageChunk = (
        (chunkType === 'VP8 ' && chunkLength >= 10
          && bytes[dataOffset + 3] === 0x9D
          && bytes[dataOffset + 4] === 0x01
          && bytes[dataOffset + 5] === 0x2A)
        || (chunkType === 'VP8L' && chunkLength >= 5 && bytes[dataOffset] === 0x2F)
        || (chunkType === 'VP8X' && chunkLength === 10)
      );
      if (!validImageChunk) return false;
      if (chunkType === 'VP8X') {
        if (chunkIndex !== 0 || foundExtendedHeader) return false;
        foundExtendedHeader = true;
      } else {
        foundImageDataChunk = true;
      }
    }
    offset = paddedEnd;
    chunkIndex += 1;
  }
  return offset === bytes.length && foundImageDataChunk;
}

function hasValidImagePayload(node) {
  if (!hasOnlyFields(node, IMAGE_NODE_FIELDS)) return false;
  if (node.intent !== 'user-image') return false;
  const imageSources = ['src', 'data'].filter((field) => (
    typeof node[field] === 'string' && Boolean(node[field].trim())
  ));
  if (imageSources.length !== 1 || Object.hasOwn(node, 'path')) return false;
  const source = node[imageSources[0]].trim();
  const match = source.match(
    /^data:image\/(png|jpeg|webp);base64,([A-Za-z0-9+/]+={0,2})$/i,
  );
  if (!match || match[2].length % 4 !== 0) return false;
  let bytes;
  try {
    const binary = typeof atob === 'function'
      ? atob(match[2])
      : Buffer.from(match[2], 'base64').toString('binary');
    bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  } catch {
    return false;
  }
  const mime = match[1].toLowerCase();
  if (mime === 'png') return hasValidPngContainer(bytes);
  if (mime === 'jpeg') return hasValidJpegContainer(bytes);
  return hasValidWebpContainer(bytes);
}

function hasValidCommonNodeFields(node) {
  return (node.zIndex === undefined || Number.isSafeInteger(node.zIndex))
    && (node.paintOrder === undefined || Number.isSafeInteger(node.paintOrder))
    && (node.subOrder === undefined || Number.isSafeInteger(node.subOrder))
    && (node.rewrite === undefined || REWRITE_TYPES.has(node.rewrite))
    && (node.order === undefined || (Number.isSafeInteger(node.order) && node.order >= 0));
}

function hasValidNodePayload(node) {
  if (!hasValidCommonNodeFields(node)) return false;
  switch (node.type) {
    case 'text':
      return hasOnlyFields(node, new Set([
        'type', 'sourceId', 'x', 'y', 'w', 'h', 'text', 'style',
        'paintOrder', 'subOrder', 'zIndex', 'rewrite',
      ]))
        && hasValidTextValue(node.text)
        && hasValidTextStyle(node.style);
    case 'shape':
      return hasOnlyFields(node, new Set([
        'type', 'sourceId', 'x', 'y', 'w', 'h', 'shapeType', 'style',
        'paintOrder', 'subOrder', 'zIndex', 'rewrite', 'order',
      ]))
        && EDITABLE_SHAPE_TYPES.has(node.shapeType)
        && hasValidShapeStyle(node.style, node.shapeType);
    case 'line':
      return hasOnlyFields(node, new Set([
        'type', 'sourceId', 'x1', 'y1', 'x2', 'y2', 'style',
        'paintOrder', 'subOrder', 'zIndex', 'rewrite',
      ]))
        && hasValidLineStyle(node.style);
    case 'table':
      return hasOnlyFields(node, TABLE_NODE_FIELDS) && hasValidTablePayload(node);
    case 'image':
      return hasValidImagePayload(node);
    default:
      return false;
  }
}

export function validateEditableSlideScene(scene) {
  if (!scene || Array.isArray(scene) || typeof scene !== 'object') {
    throw editableSceneError(
      scene,
      'editable_scene_invalid',
      'Editable slide scene must be an object.',
    );
  }
  const fallbackField = findForbiddenFallbackMetadata(scene, {
    skipKeys: new Set(['nodes']),
  });
  if (fallbackField) {
    throw editableSceneError(
      scene,
      'editable_scene_fallback_forbidden',
      `Editable slide scene must not contain ${fallbackField}.`,
    );
  }
  if (!hasOnlyFields(scene, SCENE_FIELDS)) {
    throw editableSceneError(
      scene,
      'editable_scene_invalid',
      'Editable slide scene contains unknown top-level fields.',
    );
  }

  if (!Number.isInteger(scene.slideNumber) || scene.slideNumber <= 0) {
    throw editableSceneError(
      scene,
      'editable_scene_invalid',
      'Editable slide scene requires a positive integer slideNumber.',
    );
  }
  if (!Number.isFinite(scene.width) || scene.width <= 0
    || !Number.isFinite(scene.height) || scene.height <= 0) {
    throw editableSceneError(
      scene,
      'editable_scene_geometry_invalid',
      'Editable slide scene width and height must be finite positive numbers.',
    );
  }
  if (!Array.isArray(scene.nodes)) {
    throw editableSceneError(
      scene,
      'editable_scene_invalid',
      'Editable slide scene nodes must be an array.',
    );
  }

  scene.nodes.forEach((node, index) => {
    const defaultSourceId = `node-${index + 1}`;
    if (!node || Array.isArray(node) || typeof node !== 'object') {
      throw editableSceneError(
        scene,
        'editable_scene_node_invalid',
        'Editable slide scene nodes must be objects.',
        defaultSourceId,
      );
    }

    const hasValidSourceId = typeof node.sourceId === 'string' && Boolean(node.sourceId.trim());
    const sourceId = hasValidSourceId ? node.sourceId : defaultSourceId;
    const nodeFallbackField = findForbiddenFallbackMetadata(node, {
      allowDirectData: node.type === 'image',
    });
    if (nodeFallbackField) {
      throw editableSceneError(
        scene,
        'editable_scene_fallback_forbidden',
        `Editable scene nodes must not contain ${nodeFallbackField}.`,
        sourceId,
      );
    }
    if (!EDITABLE_NODE_TYPES.has(node.type)) {
      throw editableSceneError(
        scene,
        'editable_scene_node_type_unsupported',
        `Editable scene node type "${String(node.type)}" is not supported.`,
        sourceId,
      );
    }
    if (!hasValidSourceId) {
      throw editableSceneError(
        scene,
        'editable_scene_source_id_invalid',
        'Editable scene nodes require a non-empty sourceId.',
        sourceId,
      );
    }

    // Shapes/images/lines may intentionally bleed past the slide origin
    // (decorative bands, full-bleed accents). Text/tables stay non-negative.
    const allowNegativeOrigin = node.type === 'shape'
      || node.type === 'image'
      || node.type === 'line';
    const hasValidGeometry = node.type === 'line'
      ? hasValidLineGeometry(node, { allowNegativeOrigin })
      : hasValidBoxGeometry(node, { allowNegativeOrigin });
    if (!hasValidGeometry) {
      throw editableSceneError(
        scene,
        'editable_scene_geometry_invalid',
        `Editable scene node "${sourceId}" has invalid geometry.`,
        sourceId,
      );
    }
    if (!hasValidNodePayload(node)) {
      throw editableSceneError(
        scene,
        'editable_scene_payload_invalid',
        `Editable scene node "${sourceId}" has an invalid ${node.type} payload.`,
        sourceId,
      );
    }
  });

  return scene;
}
