const I18N = {
  'en-US': {
    title: 'Logo Cutout',
    subtitle: 'Transparent PNG export for solid-color backgrounds',
    dropAria: 'Choose or drop an image',
    dropTitle: 'Drop a logo or graphic',
    dropHint: 'The app samples the image edges, detects the background color, and removes only the connected background.',
    pick: 'Choose image',
    download: 'Download PNG',
    original: 'Original',
    result: 'Transparent result',
    background: 'Background',
    tolerance: 'Tolerance',
    edgeCleanup: 'Edge cleanup',
    autoCrop: 'Trim transparent edges',
    stats: 'Detection',
    size: 'Size',
    removed: 'Removed',
    backgroundColor: 'Color',
    ready: 'Ready for PNG, JPG, WebP, SVG, or BMP.',
    loaded: 'Loaded {{name}}. Adjust tolerance if the background is incomplete.',
    processed: 'Removed {{percent}} of the canvas. Background: {{color}}.',
    loadFailed: 'Could not read this image.',
  },
  'zh-CN': {
    title: 'Logo 抠图',
    subtitle: '识别任意纯色背景并导出透明 PNG',
    dropAria: '选择或拖入图片',
    dropTitle: '拖入 logo 或图形',
    dropHint: '应用会从图片边缘采样背景色，只移除与边缘连通的背景区域。',
    pick: '选择图片',
    download: '下载 PNG',
    original: '原图',
    result: '透明结果',
    background: '背景',
    tolerance: '容差',
    edgeCleanup: '边缘清理',
    autoCrop: '裁掉透明边缘',
    stats: '识别结果',
    size: '尺寸',
    removed: '已移除',
    backgroundColor: '颜色',
    ready: '支持 PNG、JPG、WebP、SVG 或 BMP。',
    loaded: '已载入 {{name}}。如果背景残留，可以调高容差。',
    processed: '已移除画布 {{percent}}。背景色：{{color}}。',
    loadFailed: '无法读取这张图片。',
  },
};

const $ = (id) => document.getElementById(id);
const els = {
  dropZone: $('drop-zone'),
  fileInput: $('file-input'),
  pickBtn: $('pick-btn'),
  emptyState: $('empty-state'),
  previewGrid: $('preview-grid'),
  sourceCanvas: $('source-canvas'),
  resultCanvas: $('result-canvas'),
  downloadBtn: $('download-btn'),
  tolerance: $('tolerance'),
  toleranceValue: $('tolerance-value'),
  edgeCleanup: $('edge-cleanup'),
  edgeValue: $('edge-value'),
  autoCrop: $('auto-crop'),
  bgChip: $('bg-chip'),
  statSize: $('stat-size'),
  statRemoved: $('stat-removed'),
  statColor: $('stat-color'),
  status: $('status'),
};

const state = {
  image: null,
  fileName: 'cutout.png',
  lastObjectUrl: null,
  resultBlobUrl: null,
  processingTimer: 0,
};

function currentLocale() {
  return (window.app && window.app.locale) || 'en-US';
}

function t(key, vars = {}) {
  const dict = I18N[currentLocale()] || I18N['en-US'];
  const template = dict[key] || I18N['en-US'][key] || key;
  return template.replace(/\{\{(\w+)\}\}/g, (_, name) => String(vars[name] ?? ''));
}

function applyStaticI18n() {
  document.documentElement.setAttribute('lang', currentLocale());
  document.querySelectorAll('[data-i18n]').forEach((node) => {
    const key = node.getAttribute('data-i18n');
    const attr = node.getAttribute('data-i18n-attr');
    const value = t(key);
    if (attr) node.setAttribute(attr, value);
    else node.textContent = value;
  });
}

function scheduleProcess() {
  els.toleranceValue.textContent = els.tolerance.value;
  els.edgeValue.textContent = els.edgeCleanup.value;
  if (!state.image) return;
  window.clearTimeout(state.processingTimer);
  state.processingTimer = window.setTimeout(processImage, 40);
}

function setStatus(key, vars) {
  els.status.textContent = t(key, vars);
}

function formatHex(color) {
  return `#${[color.r, color.g, color.b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

function colorDistanceSq(data, index, color) {
  const dr = data[index] - color.r;
  const dg = data[index + 1] - color.g;
  const db = data[index + 2] - color.b;
  return dr * dr + dg * dg + db * db;
}

function estimateBackgroundColor(data, width, height) {
  const bins = new Map();
  const samples = [];
  const step = Math.max(1, Math.floor((width + height) / 900));
  const add = (x, y) => {
    const i = (y * width + x) * 4;
    if (data[i + 3] < 12) return;
    const r = data[i], g = data[i + 1], b = data[i + 2];
    const key = `${r >> 4},${g >> 4},${b >> 4}`;
    let bin = bins.get(key);
    if (!bin) {
      bin = { count: 0, r: 0, g: 0, b: 0 };
      bins.set(key, bin);
    }
    bin.count += 1;
    bin.r += r;
    bin.g += g;
    bin.b += b;
    samples.push([r, g, b]);
  };

  for (let x = 0; x < width; x += step) {
    add(x, 0);
    add(x, height - 1);
  }
  for (let y = 0; y < height; y += step) {
    add(0, y);
    add(width - 1, y);
  }

  let best = null;
  for (const bin of bins.values()) {
    if (!best || bin.count > best.count) best = bin;
  }
  if (!best) return { r: 255, g: 255, b: 255 };

  let color = {
    r: Math.round(best.r / best.count),
    g: Math.round(best.g / best.count),
    b: Math.round(best.b / best.count),
  };

  for (let pass = 0; pass < 2; pass += 1) {
    let count = 0, r = 0, g = 0, b = 0;
    for (const sample of samples) {
      const dr = sample[0] - color.r;
      const dg = sample[1] - color.g;
      const db = sample[2] - color.b;
      if (dr * dr + dg * dg + db * db <= 42 * 42) {
        count += 1;
        r += sample[0];
        g += sample[1];
        b += sample[2];
      }
    }
    if (count > 0) {
      color = { r: Math.round(r / count), g: Math.round(g / count), b: Math.round(b / count) };
    }
  }
  return color;
}

function buildBackgroundMask(data, width, height, color, tolerance) {
  const total = width * height;
  const mask = new Uint8Array(total);
  const queue = new Int32Array(total);
  let head = 0;
  let tail = 0;
  const thresholdSq = tolerance * tolerance;

  const tryPush = (x, y) => {
    if (x < 0 || y < 0 || x >= width || y >= height) return;
    const p = y * width + x;
    if (mask[p]) return;
    const i = p * 4;
    if (data[i + 3] < 12 || colorDistanceSq(data, i, color) <= thresholdSq) {
      mask[p] = 1;
      queue[tail] = p;
      tail += 1;
    }
  };

  for (let x = 0; x < width; x += 1) {
    tryPush(x, 0);
    tryPush(x, height - 1);
  }
  for (let y = 1; y < height - 1; y += 1) {
    tryPush(0, y);
    tryPush(width - 1, y);
  }

  while (head < tail) {
    const p = queue[head];
    head += 1;
    const x = p % width;
    const y = Math.floor(p / width);
    tryPush(x + 1, y);
    tryPush(x - 1, y);
    tryPush(x, y + 1);
    tryPush(x, y - 1);
  }

  return { mask, count: tail };
}

function cleanEdges(data, width, height, mask, color, tolerance, cleanup) {
  const fringe = Math.max(1, cleanup);
  const maxDistance = tolerance + fringe;
  const maxDistanceSq = maxDistance * maxDistance;
  const toleranceSq = tolerance * tolerance;

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const p = y * width + x;
      const i = p * 4;
      if (mask[p]) {
        data[i + 3] = 0;
        continue;
      }

      const nearBg =
        (x > 0 && mask[p - 1]) ||
        (x < width - 1 && mask[p + 1]) ||
        (y > 0 && mask[p - width]) ||
        (y < height - 1 && mask[p + width]);
      if (!nearBg || cleanup <= 0) continue;

      const distSq = colorDistanceSq(data, i, color);
      if (distSq > maxDistanceSq) continue;
      const dist = Math.sqrt(distSq);
      const alphaFactor = Math.max(0, Math.min(1, (dist - tolerance) / Math.max(1, fringe)));
      data[i + 3] = Math.round(data[i + 3] * alphaFactor);
      if (distSq <= toleranceSq) {
        data[i] = color.r;
        data[i + 1] = color.g;
        data[i + 2] = color.b;
      }
    }
  }
}

function cropImageData(imageData, minAlpha = 6) {
  const { data, width, height } = imageData;
  let minX = width, minY = height, maxX = -1, maxY = -1;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (data[(y * width + x) * 4 + 3] > minAlpha) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < minX || maxY < minY) return imageData;
  const cropWidth = maxX - minX + 1;
  const cropHeight = maxY - minY + 1;
  const cropped = new ImageData(cropWidth, cropHeight);
  for (let y = 0; y < cropHeight; y += 1) {
    const sourceStart = ((minY + y) * width + minX) * 4;
    const targetStart = y * cropWidth * 4;
    cropped.data.set(data.subarray(sourceStart, sourceStart + cropWidth * 4), targetStart);
  }
  return cropped;
}

function fitCanvasToImage(canvas, width, height) {
  canvas.width = width;
  canvas.height = height;
}

function drawSource() {
  if (!state.image) return;
  const ctx = els.sourceCanvas.getContext('2d', { willReadFrequently: true });
  fitCanvasToImage(els.sourceCanvas, state.image.width, state.image.height);
  ctx.clearRect(0, 0, state.image.width, state.image.height);
  ctx.drawImage(state.image, 0, 0);
}

function processImage() {
  if (!state.image) return;
  const width = state.image.width;
  const height = state.image.height;
  const sourceCtx = els.sourceCanvas.getContext('2d', { willReadFrequently: true });
  const resultCtx = els.resultCanvas.getContext('2d', { willReadFrequently: true });
  const tolerance = Number(els.tolerance.value);
  const cleanup = Number(els.edgeCleanup.value);

  drawSource();
  let imageData = sourceCtx.getImageData(0, 0, width, height);
  const color = estimateBackgroundColor(imageData.data, width, height);
  const { mask, count } = buildBackgroundMask(imageData.data, width, height, color, tolerance);
  cleanEdges(imageData.data, width, height, mask, color, tolerance, cleanup);
  if (els.autoCrop.checked) {
    imageData = cropImageData(imageData);
  }

  fitCanvasToImage(els.resultCanvas, imageData.width, imageData.height);
  resultCtx.clearRect(0, 0, imageData.width, imageData.height);
  resultCtx.putImageData(imageData, 0, 0);

  const percent = `${Math.round((count / (width * height)) * 1000) / 10}%`;
  const hex = formatHex(color);
  els.bgChip.style.background = hex;
  els.statSize.textContent = `${width} x ${height}`;
  els.statRemoved.textContent = percent;
  els.statColor.textContent = hex;
  setStatus('processed', { percent, color: hex });
  els.downloadBtn.disabled = false;
}

async function loadFile(file) {
  if (!file) return;
  state.fileName = file.name.replace(/\.[^.]+$/, '') || 'cutout';

  try {
    state.image = await decodeImageFile(file);
    els.emptyState.hidden = true;
    els.previewGrid.hidden = false;
    setStatus('loaded', { name: file.name });
    scheduleProcess();
  } catch (_error) {
    setStatus('loadFailed');
  }
}

async function decodeImageFile(file) {
  if ('createImageBitmap' in window) {
    try {
      return await createImageBitmap(file);
    } catch (_error) {
      // Fall back to a data URL for formats not supported by createImageBitmap.
    }
  }

  const dataUrl = await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error || new Error('File read failed'));
    reader.readAsDataURL(file);
  });

  return await new Promise((resolve, reject) => {
    const image = new Image();
    image.decoding = 'async';
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('Image decode failed'));
    image.src = dataUrl;
  });
}

function downloadResult() {
  if (!state.image) return;
  els.resultCanvas.toBlob((blob) => {
    if (!blob) return;
    if (state.resultBlobUrl) URL.revokeObjectURL(state.resultBlobUrl);
    state.resultBlobUrl = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = state.resultBlobUrl;
    link.download = `${state.fileName}-transparent.png`;
    document.body.appendChild(link);
    link.click();
    link.remove();
  }, 'image/png');
}

function bindEvents() {
  els.pickBtn.addEventListener('click', (event) => {
    event.stopPropagation();
    els.fileInput.click();
  });
  els.dropZone.addEventListener('click', () => els.fileInput.click());
  els.dropZone.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      els.fileInput.click();
    }
  });
  els.fileInput.addEventListener('change', () => loadFile(els.fileInput.files && els.fileInput.files[0]));

  ['dragenter', 'dragover'].forEach((name) => {
    els.dropZone.addEventListener(name, (event) => {
      event.preventDefault();
      els.dropZone.classList.add('is-dragging');
    });
  });
  ['dragleave', 'drop'].forEach((name) => {
    els.dropZone.addEventListener(name, (event) => {
      event.preventDefault();
      els.dropZone.classList.remove('is-dragging');
    });
  });
  els.dropZone.addEventListener('drop', (event) => {
    const file = event.dataTransfer && event.dataTransfer.files && event.dataTransfer.files[0];
    loadFile(file);
  });

  els.tolerance.addEventListener('input', scheduleProcess);
  els.edgeCleanup.addEventListener('input', scheduleProcess);
  els.autoCrop.addEventListener('change', scheduleProcess);
  els.downloadBtn.addEventListener('click', downloadResult);

  if (window.app && window.app.onLocaleChange) {
    window.app.onLocaleChange(() => {
      applyStaticI18n();
      setStatus('ready');
    });
  }
}

applyStaticI18n();
bindEvents();
scheduleProcess();
