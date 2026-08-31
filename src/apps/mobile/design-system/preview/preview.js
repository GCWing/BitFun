import { mobilePreviewScenarios, mobileTokens } from './generated/mobile-design-data.js';

const platforms = [
  { id: 'harmonyos', captureId: 'harmony', name: 'HarmonyOS', mark: 'H', note: 'ArkUI · baseline' },
  { id: 'android', name: 'Android', mark: 'A', note: 'Jetpack Compose' },
  { id: 'ios', name: 'iOS', mark: 'i', note: 'SwiftUI' },
];

const scenarioSelect = document.querySelector('#scenario-select');
const appearanceSelect = document.querySelector('#appearance-select');
const opacityControl = document.querySelector('#opacity-control');
const opacityOutput = document.querySelector('#opacity-output');
const gridControl = document.querySelector('#grid-control');
const platformGrid = document.querySelector('#platform-grid');
const nativeCaptures = new Map();

for (const scenario of mobilePreviewScenarios.scenarios) {
  const option = document.createElement('option');
  option.value = scenario.id;
  option.textContent = scenario.title;
  scenarioSelect.append(option);
}

scenarioSelect.addEventListener('change', render);
appearanceSelect.addEventListener('change', render);
gridControl.addEventListener('change', () => {
  platformGrid.classList.toggle('show-grid', gridControl.checked);
});
opacityControl.addEventListener('input', () => {
  const opacity = Number(opacityControl.value) / 100;
  opacityOutput.textContent = `${opacityControl.value}%`;
  document.documentElement.style.setProperty('--native-opacity', String(opacity));
});

render();

function render() {
  nativeCaptures.clear();
  const scenario = mobilePreviewScenarios.scenarios.find((item) => item.id === scenarioSelect.value)
    ?? mobilePreviewScenarios.scenarios[0];
  const appearance = appearanceSelect.value === 'scenario' ? scenario.appearance : appearanceSelect.value;
  document.querySelector('#scenario-title').textContent = scenario.title;
  document.querySelector('#scenario-description').textContent = scenario.description;
  platformGrid.replaceChildren(...platforms.map((platform) => platformCard(platform, scenario, appearance)));
  platformGrid.classList.toggle('show-grid', gridControl.checked);
}

function platformCard(platform, scenario, appearance) {
  const card = document.createElement('article');
  card.className = 'platform-card';
  card.dataset.platform = platform.id;
  card.innerHTML = `
    <header class="platform-heading">
      <div class="platform-mark" aria-hidden="true">${platform.mark}</div>
      <div class="platform-title"><strong></strong><span></span></div>
      <label class="capture-button">载入原生截图<input type="file" accept="image/*" /></label>
    </header>
    <div class="viewport-stage">
      <div class="device-screen">
        <div class="contract-render">
          <div class="screen-meta"><strong></strong><span></span></div>
          <div class="conversation-header">
            <div class="circle-control" aria-label="Open sidebar">☰</div>
            <div class="header-copy"><strong></strong><span></span></div>
            <div class="circle-control" aria-label="More actions">•••</div>
          </div>
          <div class="timeline"></div>
          <div class="composer-zone">
            <div class="composer">
              <button aria-label="Attach">＋</button>
              <div class="composer-copy"></div>
              <button class="primary" aria-label="Primary action"></button>
            </div>
          </div>
        </div>
        <img class="native-shot" alt="" />
        <div class="alignment-grid"></div>
      </div>
    </div>
    <footer class="capture-status"><strong>CONTRACT RENDER</strong><span>等待原生截图</span><em></em></footer>
  `;

  card.querySelector('.platform-title strong').textContent = platform.name;
  card.querySelector('.platform-title span').textContent = platform.note;

  const screen = card.querySelector('.device-screen');
  applyTokens(screen, appearance);
  screen.style.setProperty('--viewport-width', String(scenario.viewport.width));
  screen.style.setProperty('--viewport-height', String(scenario.viewport.height));
  screen.classList.toggle('wide', scenario.viewport.width >= mobileTokens.breakpoints.wide);

  card.querySelector('.screen-meta strong').textContent = platform.name;
  card.querySelector('.screen-meta span').textContent = `${scenario.viewport.width} × ${scenario.viewport.height}`;
  card.querySelector('.header-copy strong').textContent = scenario.header.title;
  card.querySelector('.header-copy span').textContent = scenario.header.subtitle;

  const timeline = card.querySelector('.timeline');
  for (const message of scenario.messages) {
    const bubble = document.createElement('div');
    bubble.className = `message ${message.role}`;
    bubble.textContent = message.text;
    timeline.append(bubble);
  }
  if (scenario.composer.phase === 'reconnecting') {
    const note = document.createElement('div');
    note.className = 'connection-note';
    note.textContent = 'RECONNECTING · CURSOR PRESERVED';
    timeline.append(note);
  }

  const composer = card.querySelector('.composer');
  const hasDraft = scenario.composer.draft.length > 0;
  composer.classList.toggle('has-draft', hasDraft);
  card.querySelector('.composer-copy').textContent = scenario.composer.draft || scenario.composer.placeholder;
  card.querySelector('.composer .primary').textContent = scenario.composer.streaming ? '■' : hasDraft ? '↑' : '●';

  const fileInput = card.querySelector('input[type="file"]');
  fileInput.addEventListener('change', () => {
    const [file] = fileInput.files;
    if (file) useNativeShot(card, URL.createObjectURL(file), file.name);
  });
  tryConventionalScreenshot(card, platform.captureId ?? platform.id, scenario.id);
  return card;
}

function applyTokens(element, appearance) {
  for (const [name, pair] of Object.entries(mobileTokens.colors)) {
    element.style.setProperty(`--mobile-${name.replaceAll('_', '-')}`, mobileColorToCss(pair[appearance]));
  }
  for (const [name, value] of Object.entries(mobileTokens.geometry)) {
    element.style.setProperty(`--mobile-${name.replaceAll('_', '-')}`, String(value));
  }
  for (const [name, token] of Object.entries(mobileTokens.typography)) {
    const prefix = `--mobile-${name.replaceAll('_', '-')}`;
    element.style.setProperty(`${prefix}-size`, String(token.size));
    element.style.setProperty(`${prefix}-line-height`, String(token.lineHeight));
    element.style.setProperty(`${prefix}-weight`, String(token.weight));
  }
}

function tryConventionalScreenshot(card, platform, scenario) {
  const path = `./snapshots/${scenario}/${platform}.png`;
  const image = new Image();
  image.onload = () => useNativeShot(card, path, `${platform}.png`);
  image.src = path;
}

function useNativeShot(card, source, label) {
  const screen = card.querySelector('.device-screen');
  const image = card.querySelector('.native-shot');
  image.addEventListener('load', () => {
    const expectedAspect = Number(screen.style.getPropertyValue('--viewport-width'))
      / Number(screen.style.getPropertyValue('--viewport-height'));
    const captureAspect = image.naturalWidth / image.naturalHeight;
    const aspectDelta = Math.abs(captureAspect / expectedAspect - 1) * 100;
    card.querySelector('.capture-status span').textContent =
      `${label} · ${image.naturalWidth}×${image.naturalHeight} · 画幅差 ${aspectDelta.toFixed(1)}%`;
    card.classList.toggle('capture-aspect-warning', aspectDelta >= 1);
    if (card.isConnected) {
      nativeCaptures.set(card.dataset.platform, { card, image });
      updatePixelDeltas();
    }
  }, { once: true });
  image.src = source;
  image.alt = `${card.dataset.platform} native screenshot`;
  screen.classList.add('has-native');
  card.querySelector('.capture-status strong').textContent = 'NATIVE CAPTURE';
  card.querySelector('.capture-status span').textContent = `${label} · 读取尺寸…`;
}

function updatePixelDeltas() {
  const baseline = nativeCaptures.get('harmonyos');
  if (!baseline) return;
  baseline.card.querySelector('.capture-delta, .capture-status em').textContent = 'PIXEL BASELINE';
  for (const platform of ['android', 'ios']) {
    const capture = nativeCaptures.get(platform);
    if (!capture) continue;
    const delta = significantPixelDelta(baseline.image, capture.image);
    capture.card.querySelector('.capture-delta, .capture-status em').textContent = `全帧像素差 ${delta.toFixed(1)}%`;
  }
}

function significantPixelDelta(baseline, candidate) {
  const width = 195;
  const height = 422;
  const referencePixels = containedPixels(baseline, width, height);
  const candidatePixels = containedPixels(candidate, width, height);
  let changed = 0;
  const pixelCount = width * height;
  for (let index = 0; index < referencePixels.length; index += 4) {
    const channelDelta = (
      Math.abs(referencePixels[index] - candidatePixels[index])
      + Math.abs(referencePixels[index + 1] - candidatePixels[index + 1])
      + Math.abs(referencePixels[index + 2] - candidatePixels[index + 2])
    ) / 3;
    if (channelDelta >= 16) changed += 1;
  }
  return changed / pixelCount * 100;
}

function containedPixels(image, width, height) {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  context.fillStyle = mobileColorToCss(mobileTokens.colors.media_background.dark);
  context.fillRect(0, 0, width, height);
  const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
  const drawWidth = image.naturalWidth * scale;
  const drawHeight = image.naturalHeight * scale;
  context.drawImage(image, (width - drawWidth) / 2, (height - drawHeight) / 2, drawWidth, drawHeight);
  return context.getImageData(0, 0, width, height).data;
}

function mobileColorToCss(value) {
  if (!/^#[0-9A-F]{8}$/i.test(value)) return value;
  const alpha = value.slice(1, 3);
  const rgb = value.slice(3);
  return `#${rgb}${alpha}`;
}
