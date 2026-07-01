import { translate as t, getLocale } from './src/i18n.js';
import {
  ELEMENT_TYPES,
  HISTORY_KEY,
  STORAGE_KEY,
  clamp,
  clone,
  createInitialState,
  defaultOutline,
  defaultElement,
  ensureState,
  escapeHtml,
  getActiveIndex,
  getActiveSlide,
  getSelectedElement,
  makeSlide,
  normalizeElement,
  normalizeGeneration,
  normalizeSlide,
  normalizeDensity,
  densityToIndex,
  indexToDensity,
  uid,
} from './src/state.js';
import { getAllStylePresets, getStylePreset, DEFAULT_STYLE_PRESET, resolveStylePalette } from './src/style-presets.js';
import { enhanceFlatSelect, refreshFlatSelect } from './src/flat-select.js';
import { applyI18n, readInputs, readStyleInputs, renderAll, renderInspector, renderSlideCanvas, renderGeneration, renderGenerationOverlay, renderThumbs, slideHtml, fitSlideCanvas, fitHtmlSlideFrame, buildExportPreviewStage, fitExportPreviewFrame, fitThumbPreviews, normalizeSlideDocument, observeThumbPreviews, ensureCanvasFitted, syncDensitySlider, syncFontFamilyToggle, syncColorModeToggle, syncStylePanelFromState } from './src/render.js';
import {
  prepareSlidesForPptxExport,
  slideExportHtml,
  EXPORT_VIEWPORT,
} from './src/export-slide-browser.js';
import {
  exportPdfFromBase64Pages,
  exportPngZipFromPages,
  exportPptxFromDeck,
  exportPptxPrepared,
} from './src/export-deck-host.js';
import { downloadBase64File, downloadHtmlDeck, fileSafe } from './src/export-html.js';
import { exportFormatIcon, exportFormatTone } from './src/export-format-icons.js';
import {
  installBitFunBackendAdapter,
  PPT_DESIGN_SKILL_KEY,
} from './src/bitfun-backend-adapter.js';

let state = createInitialState();
let busy = false;
let dragState = null;
/** @type {{ sessionId: string, turnId: string }[]} */
let backendRuns = [];
let deckEpoch = 0;
let promptSubmitGuard = false;
let backendRunInFlight = false;
let historyItems = [];
let lastHistoryWriteAt = 0;

const $ = (id) => document.getElementById(id);
const runtime = () => window.app || {};
installBitFunBackendAdapter(runtime());
const STORAGE_TIMEOUT_MS = 2500;
const memoryStorage = new Map();

function safeLocalStorageGet(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return memoryStorage.has(key) ? memoryStorage.get(key) : null;
  }
}

function safeLocalStorageSet(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    memoryStorage.set(key, value);
  }
}

const localStorageBackend = {
  get: async (key) => JSON.parse(safeLocalStorageGet(key) || 'null'),
  set: async (key, value) => safeLocalStorageSet(key, JSON.stringify(value)),
};

function storage() {
  const host = runtime();
  if (host.storage) return host.storage;
  return localStorageBackend;
}

async function storageGet(key) {
  const backend = storage();
  if (backend === localStorageBackend || !runtime().storage) {
    return backend.get(key);
  }
  try {
    return await Promise.race([
      backend.get(key),
      new Promise((_, reject) => setTimeout(() => reject(new Error('storage-timeout')), STORAGE_TIMEOUT_MS)),
    ]);
  } catch (error) {
    runtime().log?.warn?.('Host storage read timed out, using local fallback', { key, error: String(error) });
    return localStorageBackend.get(key);
  }
}

async function storageSet(key, value) {
  const backend = storage();
  if (backend === localStorageBackend || !runtime().storage) {
    await backend.set(key, value);
    return;
  }
  try {
    await Promise.race([
      backend.set(key, value),
      new Promise((_, reject) => setTimeout(() => reject(new Error('storage-timeout')), STORAGE_TIMEOUT_MS)),
    ]);
  } catch (error) {
    runtime().log?.warn?.('Host storage write timed out, using local fallback', { key, error: String(error) });
    await localStorageBackend.set(key, value);
  }
}

async function loadState() {
  try {
    historyItems = await loadHistory();
    const saved = await storageGet(STORAGE_KEY);
    if (saved) {
      state = ensureState(saved);
      if (isRecoverableWorkingOnlyState(state)) {
        state = createInitialState();
        await storageSet(STORAGE_KEY, { ...state, updatedAt: Date.now() });
      }
      return;
    }
    state = createInitialState();
    await persist(true);
  } catch (error) {
    runtime().log?.warn?.('Failed to load PPT Live state', { error: String(error) });
    state = createInitialState();
  }
}

async function persist(silent = false) {
  state = ensureState(state);
  await storageSet(STORAGE_KEY, { ...state, updatedAt: Date.now() });
  await saveHistorySnapshot(silent ? 'autosave' : 'manual');
  if (!silent) setStatus(t('saved'));
}

async function loadHistory() {
  try {
    const value = await storageGet(HISTORY_KEY);
    return Array.isArray(value) ? value.map(normalizeHistoryItem).filter(Boolean).slice(0, 40) : [];
  } catch (error) {
    runtime().log?.warn?.('Failed to load PPT Live history', { error: String(error) });
    return [];
  }
}

async function saveHistorySnapshot(reason = 'autosave') {
  if (!state?.slides?.length) return;
  if (isRecoverableWorkingOnlyState(state)) return;
  const now = Date.now();
  if (reason === 'autosave' && lastHistoryWriteAt && now - lastHistoryWriteAt < 15000) return;
  lastHistoryWriteAt = now;
  const item = normalizeHistoryItem({
    id: state.sessionId || uid('deck'),
    title: state.title || t('blankDeckTitle'),
    updatedAt: now,
    slideCount: state.slides.length,
    reason,
    prompt: state.promptDraft || state.brief?.topic || '',
    state: clone({ ...state, generation: { ...state.generation, active: false } }),
  });
  if (!item) return;
  historyItems = [item, ...historyItems.filter((entry) => entry.id !== item.id)].slice(0, 40);
  await storageSet(HISTORY_KEY, historyItems);
  renderHistory();
}

function isRecoverableWorkingOnlyState(value) {
  const slides = Array.isArray(value?.slides) ? value.slides : [];
  return slides.length === 1
    && !slides[0]?.html
    && String(slides[0]?.id || '').startsWith('agent-working-slide')
    && String(value?.title || '') === t('agentWorkingTitle')
    && !value?.generation?.active;
}

function normalizeHistoryItem(item) {
  if (!item?.id || !item?.state) return null;
  return {
    id: String(item.id),
    title: String(item.title || item.state?.title || t('blankDeckTitle')),
    updatedAt: Number(item.updatedAt || Date.now()),
    slideCount: Number(item.slideCount || item.state?.slides?.length || 0),
    reason: String(item.reason || 'autosave'),
    prompt: String(item.prompt || item.state?.brief?.topic || ''),
    state: item.state,
  };
}

function renderHistory() {
  const list = $('historyList');
  if (!list) return;
  list.innerHTML = '';
  if (!historyItems.length) {
    const empty = document.createElement('div');
    empty.className = 'history-empty';
    empty.textContent = t('historyEmpty');
    list.append(empty);
    return;
  }
  historyItems.slice(0, 12).forEach((item) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = `history-card${item.id === state.sessionId ? ' is-active' : ''}`;
    button.innerHTML = `
      <strong>${escapeHtmlInline(item.title)}</strong>
      <span>${t('historyMeta', { count: item.slideCount, time: formatHistoryTime(item.updatedAt) })}</span>
      ${item.prompt ? `<small>${escapeHtmlInline(item.prompt)}</small>` : ''}
    `;
    button.addEventListener('click', () => void restoreHistory(item.id));
    list.append(button);
  });
}

async function restoreHistory(id) {
  const item = historyItems.find((entry) => entry.id === id);
  if (!item) return;
  deckEpoch += 1;
  await cancelTrackedBackendRuns();
  state = ensureState(clone(item.state));
  state.generation.active = false;
  resetGeneration();
  rerender();
  syncStylePanelFromState(state);
  setStatus(t('historyRestored'));
  await storageSet(STORAGE_KEY, { ...state, updatedAt: Date.now() });
}

function formatHistoryTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const mm = String(date.getMonth() + 1).padStart(2, '0');
  const dd = String(date.getDate()).padStart(2, '0');
  const hh = String(date.getHours()).padStart(2, '0');
  const min = String(date.getMinutes()).padStart(2, '0');
  return `${mm}/${dd} ${hh}:${min}`;
}

function escapeHtmlInline(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function setStatus(message) {
  const node = $('statusLine');
  if (node) node.textContent = message;
}

function setExportStatus(message) {
  const node = $('exportStatus');
  if (node) node.textContent = message;
}

function setBusy(nextBusy, message) {
  busy = nextBusy;
  document.querySelector('.ppt-live')?.classList.toggle('is-busy', busy);
  document.querySelectorAll('button, input, select, textarea').forEach((node) => {
    if (['closePreview', 'prevPresent', 'nextPresent'].includes(node.id)) return;
    if (node.id === 'cancelGeneration') {
      node.disabled = !busy;
      node.hidden = !busy;
      return;
    }
    if (node.id === 'newDeck') return;
    node.disabled = busy;
  });
  const pill = $('aiStatusPill');
  if (pill) {
    pill.textContent = busy ? t('statusPillBusy') : t('statusPillReady');
    pill.classList.toggle('is-busy', busy);
  }
  if (message) setStatus(message);
}

function setGenerationStep(id, status, message) {
  state.generation.current = id;
  state.generation.steps = state.generation.steps.map((step) => ({
    ...step,
    status: step.id === id ? status : step.status,
  }));
  state.generation.active = status === 'running' || state.generation.steps.some((step) => step.status === 'running');
  renderGeneration(state);
  renderGenerationOverlay(state);
  if (message) setStatus(message);
}

function resetGeneration() {
  state.generation.active = false;
  state.generation.current = 'idle';
  state.generation.draftedCount = 0;
  state.generation.slideTarget = 0;
  state.generation.eventSeq = 0;
  state.generation.steps = state.generation.steps.map((step) => ({ ...step, status: 'pending' }));
  state.generation.events = [];
  state.generation.agentStream = [];
  renderGeneration(state);
  renderGenerationOverlay(state);
}

function addGenerationEvent(event, detail = '', kind = 'info') {
  state.generation = normalizeGeneration(state.generation || {});
  const source = typeof event === 'string' ? { title: event, detail, kind } : { ...(event || {}) };
  const title = compactText(source.title || source.label || source.message || t('processEventUnknown'), 160);
  const eventDetail = compactText(source.detail ?? detail ?? '', 260);
  const eventKind = String(source.kind || kind || 'info').toLowerCase().replace(/[^a-z0-9-]/g, '') || 'info';
  if (!title && !eventDetail) return;

  const events = Array.isArray(state.generation.events) ? state.generation.events : [];
  const last = events[events.length - 1];
  if (last && last.title === title && last.detail === eventDetail && last.kind === eventKind) {
    last.timestamp = Date.now();
    state.generation.events = events;
  } else {
    const lastSeq = events.reduce((max, item) => Math.max(max, Number(item.seq) || 0), 0);
    const seq = Math.max(Number(state.generation.eventSeq) || 0, lastSeq) + 1;
    state.generation.eventSeq = seq;
    state.generation.events = [
      ...events,
      {
        id: uid('generation-event'),
        seq,
        title: title || t('processEventUnknown'),
        detail: eventDetail,
        kind: eventKind,
        timestamp: Date.now(),
      },
    ].slice(-80);
  }
  renderGeneration(state);
  renderGenerationOverlay(state);
}

/**
 * Push a raw Agent stream entry (tool call, text chunk, or turn lifecycle)
 * into the live process panel so users can see exactly what the Cowork
 * session is doing — not just the abstracted 5-step state machine.
 * Entries are capped (see GENERATION_STREAM_LIMIT) and rendered by
 * renderGeneration's agent-stream section.
 */
function pushAgentStreamEntry(entry) {
  // Skip noisy internal tool calls that don't help users understand progress.
  const toolName = String(entry.toolName || '').toLowerCase();
  if (toolName && STREAM_HIDDEN_TOOLS.has(toolName)) return;
  // Skip lifecycle system messages — they're implementation details, not
  // user-facing progress. The event log already covers phase transitions.
  if (entry.kind === 'system') return;

  state.generation = normalizeGeneration(state.generation || {});
  const stream = Array.isArray(state.generation.agentStream)
    ? state.generation.agentStream
    : [];
  // Coalesce consecutive text deltas into a single growing entry so the
  // stream reads like a chat transcript instead of thousands of fragments.
  if (entry.kind === 'text') {
    const last = stream[stream.length - 1];
    if (last && last.kind === 'text') {
      last.text = String(last.text || '') + String(entry.text || '');
      last.timestamp = Date.now();
      state.generation.agentStream = stream;
      renderGeneration(state);
      return;
    }
  }
  stream.push({ id: uid('agent-stream'), timestamp: Date.now(), ...entry });
  state.generation.agentStream = stream;
  renderGeneration(state);
}

async function waitFrame() {
  await new Promise((resolve) => setTimeout(resolve, 120));
}

function rerender() {
  state = ensureState(state);
  renderAll(state, handlers);
  renderHistory();
}

function updateBriefFromInputs(options = {}) {
  readInputs(state, options);
  state = ensureState(state);
}

function promptValue() {
  return $('topicInput')?.value.trim() || '';
}

function isDefaultDraft() {
  const defaultSpine = defaultOutline().join('\n');
  return !state.outline.length
    || state.outline.join('\n') === defaultSpine
    || state.title === t('defaultDeckTitle')
    || isStarterDeck();
}

function isStarterDeck() {
  const title = String(state.title || '').trim();
  const onlyStarterSlide = state.slides.length === 1
    && state.outline.length === 1
    && state.outline[0] === t('newSlideTitle');
  return onlyStarterSlide
    && (title === t('blankDeckTitle') || title === t('newSlideTitle'));
}

function hasUsableDeckForRevision() {
  return Array.isArray(state.slides)
    && state.slides.length > 0
    && !isDefaultDraft()
    && !isStarterDeck()
    && !isRecoverableWorkingOnlyState(state);
}

async function generateOutline() {
  await handlePromptSubmit();
}

async function generateDeck() {
  await handlePromptSubmit();
}

async function generateDeckFromPrompt() {
  await handlePromptSubmit();
}

async function handlePromptSubmit() {
  if (promptSubmitGuard || backendRunInFlight) {
    return;
  }
  const instruction = promptValue();
  if (!instruction) {
    setStatus(t('promptRequired'));
    return;
  }
  promptSubmitGuard = true;
  const reviseExistingDeck = hasUsableDeckForRevision();
  state.promptDraft = instruction;
  state.lastSubmittedPrompt = instruction;
  updateBriefFromInputs({ includeTopic: !reviseExistingDeck });
  if (!reviseExistingDeck) state.brief.topic = instruction;
  try {
    await runPptLiveBackend('auto', instruction, {
      includeTopic: !reviseExistingDeck,
      persistBeforeRun: true,
    });
    return;
  } catch (error) {
    if (isStoppedBackendError(error)) return;
    runtime().log?.warn?.('PPT Live backend generation failed', { error: String(error) });
    failGenerationFromError(error);
    rerender();
    await persist(true);
  } finally {
    promptSubmitGuard = false;
  }
}

function finishGenerationUi(statusMessage = t('deckReady')) {
  state.generation.active = false;
  state.generation.draftedCount = state.slides.length;
  state.generation.slideTarget = 0;
  state.generation.steps = (state.generation.steps || []).map((step) => ({
    ...step,
    status: step.status === 'error' ? 'error' : 'done',
  }));
  setStatus(statusMessage);
  renderGeneration(state);
  renderGenerationOverlay(state);
}

function failGenerationUi(statusMessage = t('backendGenerationFailed'), detail = '') {
  state.generation.active = false;
  state.generation.steps = (state.generation.steps || []).map((step) => ({
    ...step,
    status: step.status === 'done' ? 'done' : 'error',
  }));
  setStatus(statusMessage);
  addGenerationEvent({ title: statusMessage, detail: detail || t('agentOnlyRetryHint'), kind: 'error' });
  setBusy(false);
  renderGeneration(state);
  renderGenerationOverlay(state);
}

function errorMessageChain(error, maxDepth = 5) {
  const messages = [];
  const seen = new Set();
  let current = error;
  for (let depth = 0; current && depth < maxDepth; depth += 1) {
    const raw = String(current?.message || current || '').trim();
    if (raw && !seen.has(raw)) {
      seen.add(raw);
      messages.push(raw);
    }
    current = current?.cause;
  }
  return messages;
}

function backendErrorDetail(error, maxLength = 220) {
  const raw = errorMessageChain(error).join(' Root cause: ');
  if (!raw) return '';
  return compactText(raw
    .replace(/^Error:\s*/i, '')
    .replace(/^Tauri command .*? failed:\s*/i, '')
    .replace(/^live_app_backend_call:\s*/i, '')
    .replace(/^Failed to start PPT Live generation:\s*/i, '')
    .trim(), maxLength);
}

function failGenerationFromError(error) {
  const detail = backendErrorDetail(error, error?.pptLiveRecoveryExhausted ? 520 : 220);
  let statusMessage;
  let hint = detail;
  if (error?.pptLiveRecoveryExhausted) {
    const recovery = error.pptLiveRecoveryExhausted;
    statusMessage = t('generationRecoveryExhausted', {
      stage: t(recovery.stageKey, recovery.stageVars || {}),
      retries: recovery.stepAttempts,
      continuations: recovery.continuationAttempts,
    });
    hint = t('generationRecoveryFailureDetail', {
      reason: detail || t('agentOnlyRetryHint'),
    });
  } else if (isTimeoutBackendError(error)) {
    statusMessage = t('generationTimedOut');
  } else if (isRoundBudgetBackendError(error)) {
    statusMessage = t('generationRoundBudgetFailed');
    hint = t('generationRoundBudgetHint');
  } else if (detail) {
    statusMessage = t('backendGenerationFailedWithReason', { reason: detail });
  } else {
    statusMessage = t('backendGenerationFailed');
  }
  failGenerationUi(statusMessage, hint || t('agentOnlyRetryHint'));
}

function buildGenerationBrief({ includeEvidence = true } = {}) {
  const brief = {
    topic: String(state.brief?.topic || state.promptDraft || '').trim(),
    audience: String(state.brief?.audience || '').trim(),
  };
  if (includeEvidence) {
    brief.material = String(state.brief?.material || '').trim().slice(0, 12000);
    brief.sources = state.sources
      ? {
          summary: String(state.sources.summary || '').slice(0, 4000),
          facts: (state.sources.facts || []).slice(0, 16),
          warnings: (state.sources.warnings || []).slice(0, 8),
          items: (state.sources.items || []).slice(0, 6).map((item) => ({
            kind: item.kind,
            title: item.title,
            url: item.url,
            text: String(item.text || '').slice(0, 6000),
          })),
        }
      : null;
  }
  const slideTarget = Number(state.brief?.slideTarget) || 0;
  if (slideTarget > 0) brief.slideTarget = slideTarget;
  return brief;
}

function buildGenerationStyle({ includePreset = true } = {}) {
  const preset = getStylePreset(state.style?.stylePreset);
  const colorMode = state.style?.colorMode === 'dark' ? 'dark' : 'light';
  const style = {
    fontFamily: state.style?.fontFamily === 'serif' ? 'serif' : 'sans',
    density: normalizeDensity(state.style?.density),
    colorMode,
    theme: colorMode,
    palette: resolveStylePalette(preset, colorMode),
  };
  if (includePreset) style.stylePreset = state.style?.stylePreset || DEFAULT_STYLE_PRESET;
  return style;
}

function textFromHtml(html) {
  const raw = String(html || '').trim();
  if (!raw) return '';
  try {
    const doc = new DOMParser().parseFromString(raw, 'text/html');
    doc.querySelectorAll('style,script,svg').forEach((node) => node.remove());
    return compactText(doc.body?.textContent || doc.documentElement?.textContent || '', 1800);
  } catch {
    return compactText(raw.replace(/<[^>]+>/g, ' '), 1800);
  }
}

function mentionedSlideIndexes(instruction) {
  const indexes = new Set();
  const textValue = String(instruction || '');
  const activeIndex = getActiveIndex(state);
  if (/(当前|本页|这一页|此页|current\s+(slide|page)|this\s+(slide|page))/i.test(textValue)) {
    indexes.add(activeIndex);
  }
  const patterns = [
    /第\s*(\d{1,2})\s*(页|頁|张|張)/gi,
    /\b(?:slide|page)\s*(\d{1,2})\b/gi,
    /\b(\d{1,2})\s*(?:slide|slides|page|pages)\b/gi,
  ];
  patterns.forEach((pattern) => {
    let match = pattern.exec(textValue);
    while (match) {
      const index = Number(match[1]) - 1;
      if (index >= 0 && index < state.slides.length) indexes.add(index);
      match = pattern.exec(textValue);
    }
  });
  return [...indexes].sort((a, b) => a - b);
}

function buildCurrentDeckSnapshot(instruction) {
  const targetIndexes = mentionedSlideIndexes(instruction);
  const activeIndex = getActiveIndex(state);
  const fullHtmlIndexes = new Set(targetIndexes.length ? targetIndexes : [activeIndex]);
  return {
    title: state.title,
    outline: clone(state.outline || []),
    slideCount: state.slides.length,
    activeSlideIndex: activeIndex,
    activeSlideId: state.slides[activeIndex]?.id || '',
    targetHints: targetIndexes.map((index) => ({
      slideIndex: index,
      slideNumber: index + 1,
      slideId: state.slides[index]?.id || '',
      title: state.slides[index]?.title || '',
    })),
    slides: state.slides.map((slide, index) => {
      const visibleText = slide.html
        ? textFromHtml(slide.html)
        : compactText((slide.elements || [])
          .flatMap((element) => [element.text, element.label, ...(Array.isArray(element.items) ? element.items : [])])
          .filter(Boolean)
          .join('\n'), 1800);
      const snapshot = {
        slideIndex: index,
        slideNumber: index + 1,
        id: slide.id,
        title: slide.title,
        kicker: slide.kicker,
        claim: slide.claim,
        proofObject: slide.proofObject,
        supportNote: slide.supportNote,
        sourceNote: slide.sourceNote,
        notes: slide.notes,
        layout: slide.layout,
        visibleText,
        hasHtml: Boolean(slide.html),
      };
      if (fullHtmlIndexes.has(index) && slide.html) {
        snapshot.html = String(slide.html).slice(0, 12000);
      }
      return snapshot;
    }),
  };
}

function pickDensityIndexFromClientX(clientX, track) {
  const rect = track.getBoundingClientRect();
  const ratio = clamp((clientX - rect.left) / rect.width, 0, 1);
  return Math.round(ratio * 2);
}

function setDensitySliderUi(index) {
  syncDensitySlider(indexToDensity(clamp(Math.round(Number(index)), 0, 2)));
}

function readGenerationStyleFromPropertyPanel() {
  readStyleInputs(state);
  state = ensureState(state);
}

// Interrupted turns are retried as "continue" turns inside the same agent
// session. One retry is usually enough — if the model is fundamentally stuck
// (e.g. looping on the same file), more retries in the same session just
// repeat the failure and burn tokens.
const PPT_BACKEND_MAX_ATTEMPTS = 2;
const PPT_RETRY_DELAY_MS = 750;

function isRetryableBackendError(error) {
  const raw = String(error?.message || error || '');
  if (isStoppedBackendError(error)) return false;
  if (/Generation stopped/i.test(raw)) return false;
  if (/backend is unavailable|did not return sessionId/i.test(raw)) return false;
  if (/permission|workspacePath is required|unsupported PPT Live action/i.test(raw)) return false;
  return true;
}

function retryDelayMs(error, attempt) {
  const raw = String(error?.message || error || '');
  const transient = /rate limit|network|timed? out|connection|temporar|overload|service unavailable|502|503|504/i
    .test(raw);
  if (!transient) return PPT_RETRY_DELAY_MS;
  return Math.min(15000, 1000 * (2 ** Math.min(Math.max(0, attempt - 1), 4)));
}

// The hidden agent session lives in backend memory only; a backend restart or
// a stale persisted sessionId surfaces as this error. The caller should drop
// the sessionId and fall back to a self-contained turn.
function isUnknownSessionBackendError(error) {
  return /Unknown MiniApp agent session|session workspace does not match/i.test(
    String(error?.message || error || ''),
  );
}

// ─── Deck project files (ppt-design native protocol) ─────────────────────────
//
// Staged generation runs the agent inside a dedicated deck project directory
// (`decks/<runId>` under this app's appdata storage). The agent follows the
// ppt-design skill's own conventions — `project.json` for the plan and
// `slides/slide-NN.html` per page — and ui.js reads the files back. Files on
// disk are the source of truth, which makes interruption recovery natural:
// whatever was written stays written.

function backendUsesFileProtocol() {
  const host = runtime();
  return host.backend?.protocol === 'files'
    && Boolean(host.appDataDir)
    && Boolean(host.fs?.readFile);
}

function newDeckProject() {
  const runId = `deck-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  return {
    runId,
    workspaceSubdir: `decks/${runId}`,
    dir: `${runtime().appDataDir}/decks/${runId}`,
  };
}

function currentDeckProject() {
  const workspaceSubdir = String(state.agentSession?.workspaceSubdir || '');
  if (!workspaceSubdir || !runtime().appDataDir) return null;
  const runId = String(state.agentSession?.runId || workspaceSubdir.split('/').pop() || '');
  return {
    runId,
    workspaceSubdir,
    dir: `${runtime().appDataDir}/${workspaceSubdir}`,
  };
}

function deckSlideFileName(slideNumber) {
  return `slides/slide-${String(slideNumber).padStart(2, '0')}.html`;
}

function outlineItemTitle(item) {
  return typeof item === 'string' ? item : String(item?.title || '');
}

function planOutlineTitles(plan) {
  return Array.isArray(plan?.outline)
    ? plan.outline.map(outlineItemTitle).filter(Boolean)
    : [];
}

/** Placeholder titles shown during generation — must never become the deck/export title. */
function isEphemeralDeckTitle(title) {
  const value = String(title || '').trim();
  if (!value) return true;
  return [
    t('agentWorkingTitle'),
    t('generationAgentWorking'),
    t('blankDeckTitle'),
    t('defaultDeckTitle'),
    t('newSlideTitle'),
  ].includes(value);
}

/**
 * Resolve a user-visible deck title. ppt-design writes outline[] in project.json
 * but often omits a top-level title field, so derive from outline/brief when needed.
 */
function resolveDeckTitle({
  plan = null,
  payload = null,
  state = null,
  instruction = '',
  slides = [],
} = {}) {
  const outline = Array.isArray(plan?.outline)
    ? plan.outline
    : (Array.isArray(payload?.outline) ? payload.outline : []);
  const firstOutlineTitle = outline.length ? outlineItemTitle(outline[0]) : '';
  const firstSlideTitle = Array.isArray(slides) && slides.length
    ? String(slides[0]?.title || '').trim()
    : '';
  const candidates = [
    payload?.deckPatch?.title,
    payload?.patch?.title,
    payload?.title,
    plan?.title,
    firstOutlineTitle,
    state?.brief?.topic,
    instruction,
    state?.promptDraft,
    firstSlideTitle,
  ];
  for (const candidate of candidates) {
    const value = String(candidate || '').trim();
    if (value && !isEphemeralDeckTitle(value)) return value;
  }
  return t('blankDeckTitle');
}

async function readDeckProjectFile(project, relPath) {
  const fs = runtime().fs;
  if (!fs?.readFile) throw new Error('PPT Live fs API is unavailable');
  return await fs.readFile(`${project.dir}/${relPath}`);
}

/** Parsed JSON project artifact, or null when missing or not yet valid JSON. */
async function tryReadDeckJsonFile(project, relPath) {
  try {
    const raw = String(await readDeckProjectFile(project, relPath) || '');
    if (!raw.trim()) return null;
    return extractBackendJson(raw);
  } catch {
    return null;
  }
}

/** Parsed `project.json`, or null when missing or not yet valid JSON. */
async function tryReadDeckPlanFile(project) {
  return await tryReadDeckJsonFile(project, 'project.json');
}

/** Complete slide HTML from disk, or null when missing or incomplete. */
async function tryReadDeckSlideFile(project, slideNumber) {
  try {
    const raw = String(await readDeckProjectFile(project, deckSlideFileName(slideNumber)) || '').trim();
    if (!raw || !/<\/html>\s*$/i.test(raw)) return null;
    return raw;
  } catch {
    return null;
  }
}

/** Retry slide reads briefly after Write completes — disk/fs bridge may lag the tool event. */
async function tryReadDeckSlideFileWithRetry(project, slideNumber, maxAttempts = 6, delayMs = 120) {
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const html = await tryReadDeckSlideFile(project, slideNumber);
    if (html) return html;
    if (attempt < maxAttempts) {
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
  return null;
}

/**
 * Read every slide HTML file referenced by `project.json` from the deck
 * project directory. Returns `{ title, language, outline, researchReport,
 * design, slides }` shaped for `applyDeckPayload`.
 */
async function readDeckFromProjectFiles(project) {
  const plan = await tryReadDeckPlanFile(project);
  if (!plan) throw new Error('PPT Live agent finished without a valid project.json');
  const slideOrder = Array.isArray(plan.slide_order) && plan.slide_order.length
    ? plan.slide_order
    : (Array.isArray(plan.outline) ? plan.outline.map((_, index) => `slide-${String(index + 1).padStart(2, '0')}`) : []);
  const slides = [];
  for (let index = 0; index < slideOrder.length; index += 1) {
    const slideId = String(slideOrder[index] || `slide-${String(index + 1).padStart(2, '0')}`);
    const slideNumber = index + 1;
    const html = await tryReadDeckSlideFile(project, slideNumber);
    const outlineEntry = plan.outline?.[index];
    const title = typeof outlineEntry === 'string' ? outlineEntry : (outlineEntry?.title || `${t('newSlideTitle')} ${slideNumber}`);
    if (html) {
      slides.push({
        id: `ppt-live-slide-${slideNumber}`,
        slideNumber,
        title,
        html,
      });
    }
  }
  if (!slides.length) throw new Error('PPT Live agent did not produce any slide files');
  return {
    title: resolveDeckTitle({ plan, slides }),
    language: plan.language || '',
    outline: planOutlineTitles(plan),
    researchReport: plan.researchReport || null,
    design: plan.design || {},
    slides,
  };
}

/**
 * Best-effort: write the current deck's slides into a fresh project directory
 * so the cowork agent can read unchanged pages from disk during edits and only
 * rewrite the ones it changes. Skips slides without HTML content.
 */
async function seedDeckProjectFromState(project) {
  const fs = runtime().fs;
  if (!project || !fs?.writeFile || !state.slides?.length) return;
  const hasExistingProject = await tryReadDeckPlanFile(project);
  if (hasExistingProject) return; // directory already has files from a prior run
  try {
    const outline = state.slides.map((slide, index) => ({
      id: `slide-${String(index + 1).padStart(2, '0')}`,
      title: String(slide.title || ''),
      bullets: [],
      slide_id: `slide-${String(index + 1).padStart(2, '0')}`,
    }));
    const projectJson = {
      title: state.title || '',
      language: getLocale(),
      outline,
      slide_order: outline.map((item) => item.slide_id),
      style: buildGenerationStyle(),
    };
    await fs.writeFile(`${project.dir}/project.json`, `${JSON.stringify(projectJson, null, 2)}\n`);
    for (let index = 0; index < state.slides.length; index += 1) {
      const slide = state.slides[index];
      if (slide.html) {
        await fs.writeFile(`${project.dir}/${deckSlideFileName(index + 1)}`, slide.html);
      }
    }
  } catch {
    // Seeding is best-effort; the agent can still work from the currentDeck
    // snapshot in the prompt if disk seeding fails.
  }
}

/** Best-effort: drop old deck project dirs so appdata storage stays bounded. */
async function pruneOldDeckProjects(currentRunId) {
  const fs = runtime().fs;
  if (!fs?.readdir || !fs?.rm) return;
  try {
    const decksDir = `${runtime().appDataDir}/decks`;
    const entries = await fs.readdir(decksDir);
    const names = (Array.isArray(entries) ? entries : [])
      .map((entry) => (typeof entry === 'string' ? entry : entry?.name))
      .filter((name) => typeof name === 'string' && name.startsWith('deck-') && name !== currentRunId);
    for (const name of names) {
      await fs.rm(`${decksDir}/${name}`, { recursive: true });
    }
  } catch {
    // Old artifacts are harmless; never block generation on cleanup.
  }
}

async function runPptLiveBackend(operation, instruction, options = {}) {
  const host = runtime();
  if (!host.backend?.call) throw new Error('PPT Live backend is unavailable');
  if (backendRunInFlight) {
    return;
  }
  backendRunInFlight = true;
  try {
    updateBriefFromInputs({ includeTopic: options.includeTopic !== false });
    readGenerationStyleFromPropertyPanel();
    if (options.persistBeforeRun) {
      await persist(true);
    }
    await runCoworkDeckGeneration(operation, instruction);
  } finally {
    backendRunInFlight = false;
  }
}

/**
 * Run one `ppt.generate` backend turn and return `{ payload, sessionId }`.
 * Handles event wiring, streaming buffers, idle/absolute timeouts,
 * cancel-on-abandon, and run tracking. UI step transitions are delegated to
 * `hooks`:
 * - `hooks.onTextProgress(buffer)`: called as answer text streams in.
 * - `hooks.onToolPhase(kind)`: called with 'detected' | 'completed' | 'research' | 'round'.
 * `options.sessionId` submits the turn into an existing hidden agent session
 * (follow-up edits reuse the session with the loaded skill/preset context).
 * `options.appDataWorkspace` points the agent at the deck project directory.
 * `options.resultKind === 'text'` returns the raw assistant text instead of
 * demanding parseable JSON (file-protocol turns deliver through files and
 * only reply with a short status line).
 * On failure the session id is attached to the error as `pptLiveSessionId`
 * so callers can retry with a "continue" turn in the same session.
 */
async function executeBackendTurn(requestInput, hooks = {}, options = {}) {
  const host = runtime();
  const runEpoch = deckEpoch;
  let sessionId = null;
  let turnId = null;
  let textBuffer = '';
  let thinkingBuffer = '';
  let settled = false;
  let lastTextProgressAt = 0;
  let completion = null;
  const cleanup = [];
  const loggedToolEvents = new Set();
  const toolTrace = [];
  const linkedSubagentSessionIds = new Set();
  const progressTracker = createGenerationProgressTracker();
  const activity = { lastEventAt: Date.now() };

  try {
    const result = await host.backend.call('ppt.generate', requestInput, {
      entityId: 'deck',
      idempotencyKey: `ppt-live-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      sessionId: options.sessionId || undefined,
      appDataWorkspace: options.appDataWorkspace || undefined,
    });
    sessionId = result?.sessionId || null;
    turnId = result?.turnId || result?.actionRunId || null;
    if (!sessionId || !turnId) throw new Error('PPT Live backend did not return sessionId/turnId');
    trackBackendRun(sessionId, turnId);
    if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');

    const waitForResult = new Promise((resolve, reject) => {
      const listener = (event) => {
        const eventSessionId = event.sessionId;
        const isParentTurn = eventSessionId === sessionId;
        const sourceEvent = String(event.sourceEvent || '');

        if (sourceEvent.endsWith('subagent-session-linked')) {
          if (event.parentSessionId === sessionId && eventSessionId) {
            linkedSubagentSessionIds.add(eventSessionId);
            addGenerationEvent({ title: t('eventSubagentStarted'), detail: '', kind: 'tool' });
            pushAgentStreamEntry({ kind: 'system', text: `[subagent started] ${String(event.subagentName || event.sessionName || eventSessionId).slice(0, 120)}` });
          }
          return;
        }

        const isLinkedSubagent = linkedSubagentSessionIds.has(eventSessionId);
        if (!isParentTurn && !isLinkedSubagent) return;
        if (isParentTurn && event.turnId && event.turnId !== turnId) return;

        activity.lastEventAt = Date.now();
        if (!isParentTurn && eventSessionId) {
          linkedSubagentSessionIds.add(eventSessionId);
        }
        if (sourceEvent.endsWith('dialog-turn-started')) {
          progressTracker.note(t('eventTurnStarted'), '', 'turn');
        } else if (sourceEvent.endsWith('model-round-started')) {
          if (isParentTurn) {
            hooks.onToolPhase?.('round');
          } else {
            progressTracker.note(t('eventSubagentWorking'), '', 'pulse', 8000);
          }
          progressTracker.touch();
        } else if (sourceEvent.endsWith('model-round-completed')) {
          progressTracker.touch();
        } else if (sourceEvent.endsWith('tool-event')) {
          const toolEvent = normalizeToolEvent(event.toolEvent || {});
          const eventType = toolEvent.event_type || toolEvent.eventType || '';
          const rawToolName = String(toolEvent.tool_name || toolEvent.toolName || '').trim().toLowerCase();
          if (isParentTurn) {
            if (eventType === 'Started') {
              toolTrace.push({
                eventType,
                toolId: toolEvent.tool_id || toolEvent.toolId || '',
                toolName: toolEvent.tool_name || toolEvent.toolName || '',
                params: toolEvent.params || {},
              });
            } else if (eventType === 'Completed') {
              toolTrace.push({
                eventType,
                toolId: toolEvent.tool_id || toolEvent.toolId || '',
                toolName: toolEvent.tool_name || toolEvent.toolName || '',
                result: toolEvent.result || {},
              });
            } else if (eventType === 'Failed' || eventType === 'Cancelled') {
              toolTrace.push({
                eventType,
                toolId: toolEvent.tool_id || toolEvent.toolId || '',
                toolName: toolEvent.tool_name || toolEvent.toolName || '',
                error: toolEvent.error || toolEvent.message || eventType,
              });
            }
          }
          if (eventType === 'Started' && rawToolName === 'task') {
            progressTracker.note(t('eventToolTaskStarted'), '', 'tool');
            progressTracker.touch();
          }
          // Capture raw tool calls for the live agent stream (transparency).
          if (eventType === 'Started' && rawToolName) {
            const params = toolEvent.params || toolEvent.result || {};
            const paramSummary = summarizeToolParams(rawToolName, params);
            pushAgentStreamEntry({
              kind: 'tool-start',
              toolName: rawToolName,
              isSubagent: !isParentTurn,
              text: paramSummary,
            });
          } else if (eventType === 'Completed' && rawToolName) {
            const resultSummary = summarizeToolResult(rawToolName, toolEvent.result || {});
            if (resultSummary) {
              pushAgentStreamEntry({
                kind: 'tool-done',
                toolName: rawToolName,
                isSubagent: !isParentTurn,
                text: resultSummary,
              });
            }
          } else if (eventType === 'Failed' || eventType === 'Cancelled') {
            pushAgentStreamEntry({
              kind: 'tool-error',
              toolName: rawToolName,
              isSubagent: !isParentTurn,
              text: compactText(String(toolEvent.error || toolEvent.message || eventType), 200),
            });
          }
          if (shouldLogToolEvent(toolEvent, loggedToolEvents, { isSubagent: !isParentTurn })) {
            const described = describeToolEvent(event, { isSubagent: !isParentTurn });
            if (described) addGenerationEvent(described);
            progressTracker.touch();
          }
          if (isParentTurn && (eventType === 'EarlyDetected' || eventType === 'Started')) {
            hooks.onToolPhase?.('detected');
          } else if (isParentTurn && eventType === 'Completed') {
            const toolName = rawToolName;
            hooks.onToolPhase?.('completed');
            if (toolName === 'skill') {
              progressTracker.note(t('eventToolSkillReady'), '', 'phase');
            } else if (toolName === 'websearch' || toolName === 'webfetch') {
              hooks.onToolPhase?.('research');
            }
            // Progressive preview: when the agent writes/edits a slide file,
            // notify the caller so it can read the file from disk and render
            // the completed page immediately instead of waiting for the whole
            // turn to finish.
            if ((toolName === 'write' || toolName === 'edit') && typeof hooks.onSlideFileWritten === 'function') {
              const filePath = resolveToolEventFilePath(toolEvent, toolTrace);
              const slideMatch = filePath.match(/slides\/slide-(\d{2})\.html/i);
              if (slideMatch) {
                const slideNumber = parseInt(slideMatch[1], 10);
                if (Number.isFinite(slideNumber) && slideNumber > 0) {
                  // Completed tool events carry file_path in result, not params.
                  void Promise.resolve(hooks.onSlideFileWritten(slideNumber)).catch(() => {
                    // Progressive preview is best-effort; never break the turn.
                  });
                }
              }
            }
          }
        } else if (sourceEvent.endsWith('text-chunk')) {
          const chunk = String(event.text || '');
          const isThinking = event.contentType === 'thinking';
          if (isThinking) thinkingBuffer += chunk;
          else {
            textBuffer += chunk;
            progressTracker.touch();
            // Stream the raw assistant text into the live agent panel so the
            // user can see what the model is actually saying, not just the
            // abstracted 5-step state machine.
            pushAgentStreamEntry({ kind: 'text', text: chunk });
            // Throttle: progress hooks rescan the whole buffer, which is far
            // too expensive to run on every one of tens of thousands of chunks.
            const now = Date.now();
            if (now - lastTextProgressAt >= 500) {
              lastTextProgressAt = now;
              hooks.onTextProgress?.(textBuffer);
            }
          }
        } else if (sourceEvent.endsWith('token-usage-updated')) {
          // Keep token stats internal; do not surface them in the user-facing log.
        } else if (sourceEvent.endsWith('dialog-turn-completed')) {
          if (!isParentTurn) {
            progressTracker.note(t('eventSubagentDone'), '', 'tool');
            pushAgentStreamEntry({ kind: 'system', isSubagent: true, text: '[subagent done]' });
            return;
          }
          pushAgentStreamEntry({ kind: 'system', text: '[turn completed]' });
          settled = true;
          completion = {
            success: event.success,
            finishReason: event.finishReason || event.finish_reason || '',
            partialRecoveryReason:
              event.partialRecoveryReason || event.partial_recovery_reason || '',
          };
          resolve({ answer: textBuffer, thinking: thinkingBuffer });
        } else if (sourceEvent.endsWith('dialog-turn-failed') || sourceEvent.endsWith('dialog-turn-cancelled')) {
          if (!isParentTurn) {
            progressTracker.note(t('eventSubagentFailed'), '', 'error');
            pushAgentStreamEntry({ kind: 'system', isSubagent: true, text: '[subagent failed]' });
            return;
          }
          settled = true;
          // Final flush so checkpoint extractors see every slide that finished
          // streaming before the failure; retries resume from those slides.
          if (textBuffer) hooks.onTextProgress?.(textBuffer);
          const eventError = compactText(event.error || event.message || '');
          pushAgentStreamEntry({
            kind: 'system',
            text: sourceEvent.endsWith('dialog-turn-cancelled') ? '[turn cancelled]' : '[turn failed]',
          });
          addGenerationEvent({
            title: sourceEvent.endsWith('dialog-turn-cancelled') ? t('eventTurnCancelled') : t('eventTurnFailed'),
            detail: eventError,
            kind: 'error',
          });
          reject(new Error(eventError || sourceEvent));
        }
      };
      host.backend.onEvent(listener);
      cleanup.push(() => host.backend.offEvent?.(listener));
      const heartbeat = setInterval(() => {
        if (settled) return;
        const now = Date.now();
        if (now - progressTracker.lastProgressLogAt < 12000) return;
        const current = (state.generation?.steps || []).find((step) => step.status === 'running');
        progressTracker.note(current?.label ? `${current.label}…` : t('generationProgressPulse'), current?.detail || '', 'pulse', 0);
      }, 12000);
      cleanup.push(() => clearInterval(heartbeat));
    });

    const expectJson = options.resultKind !== 'text';
    const streamed = await waitForBackendResultOrPersistedText(waitForResult, sessionId, turnId, activity, { expectJson });
    const streamedText = typeof streamed === 'string' ? streamed : streamed?.answer || '';
    const streamedThinking = typeof streamed === 'string' ? '' : streamed?.thinking || '';
    if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');
    if (!expectJson) {
      // File-protocol turn: the deliverable is on disk; the reply is only a
      // short status line. The caller reads and validates the files.
      return { payload: null, text: streamedText, sessionId, toolTrace, completion };
    }
    const finalText = await resolveBackendTurnText(sessionId, turnId, streamedText, streamedThinking);
    if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');
    const payload = extractBackendJson(finalText);
    if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');
    return { payload, sessionId, toolTrace, completion };
  } catch (error) {
    if (error && typeof error === 'object' && sessionId) {
      error.pptLiveSessionId = sessionId;
      error.pptLiveToolTrace = toolTrace;
    }
    // Do not leave an orphaned backend turn running when this attempt is abandoned.
    if (!settled && sessionId && turnId && host.backend?.cancel) {
      try {
        await host.backend.cancel(sessionId, turnId);
      } catch (cancelError) {
        runtime().log?.warn?.('PPT Live backend cancel after failure failed', {
          sessionId,
          turnId,
          error: String(cancelError),
        });
      }
    }
    throw error;
  } finally {
    cleanup.forEach((fn) => fn());
    if (sessionId && turnId) untrackBackendRun(sessionId, turnId);
  }
}

function buildBackendRequestBase(operation, instruction) {
  return {
    operation,
    instruction,
    locale: getLocale(),
    brief: buildGenerationBrief(),
    style: buildGenerationStyle(),
  };
}

/**
 * Single-turn cowork deck generation. One agent turn loads the ppt-design
 * skill, researches, plans the outline, and writes every slide HTML file
 * into the deck project directory. After the turn finishes, ui.js reads
 * `project.json` and all `slides/slide-NN.html` files back from disk and
 * applies them to the UI. Interrupted attempts retry as "continue" turns
 * inside the same agent session so the model resumes with its prior context.
 */
async function runCoworkDeckGeneration(operation, instruction) {
  const runEpoch = deckEpoch;
  setBusy(true, t('working'));
  resetGeneration();
  setGenerationStep('brief', 'running', t('generationReadingBrief'));
  addGenerationEvent({ title: t('processEventStarted'), detail: t('processEventWaiting'), kind: 'start' });
  prepareAgentGenerationSurface(operation, instruction);
  let completed = false;

  // The agent works inside a dedicated deck project directory in this app's
  // appdata storage, following the ppt-design skill's native project.json +
  // slides/slide-NN.html layout.
  const project = backendUsesFileProtocol() ? (currentDeckProject() || newDeckProject()) : null;
  if (project && !state.agentSession?.workspaceSubdir) {
    await pruneOldDeckProjects(project.runId);
    // For edits of an existing deck, seed the project directory with the
    // current slides so the agent can read unchanged pages from disk and
    // only rewrite the ones it changes.
    await seedDeckProjectFromState(project);
  }
  const retrySession = {
    id: state.agentSession?.id || null,
    project,
  };
  const lastStreamPhase = { value: '' };
  const progressShim = { touch: () => {}, note: () => {}, lastProgressLogAt: 0 };
  // Track slides that have been progressively previewed so the final
  // readDeckFromProjectFiles can skip re-rendering ones already shown.
  const progressiveSlides = new Map();
  // Cache for project.json during progressive preview — reading and parsing it
  // on every slide write is wasteful IO. Refreshed lazily when a new slide
  // number appears that isn't in the cached plan's outline.
  let progressivePlan = null;

  try {
    let lastError = null;
    for (let attempt = 1; attempt <= PPT_BACKEND_MAX_ATTEMPTS; attempt += 1) {
      try {
        if (attempt > 1) {
          addGenerationEvent({
            title: t('generationRetryAttempt', { attempt, max: PPT_BACKEND_MAX_ATTEMPTS }),
            detail: backendErrorDetail(lastError),
            kind: 'start',
          });
          setStatus(t('generationRetrying', { attempt, max: PPT_BACKEND_MAX_ATTEMPTS }));
          await new Promise((resolve) => setTimeout(resolve, retryDelayMs(lastError, attempt)));
        }
        const requestInput = {
          ...buildBackendRequestBase(operation, instruction),
          ...(retrySession?.id ? { continueAfterInterruption: true } : {}),
        };
        // Only attach currentDeck context for edit operations on an existing
        // deck. For first-pass generation, sending an empty/irrelevant
        // currentDeck wastes hundreds of tokens in the prompt's Input JSON.
        const hasExistingDeck = hasUsableDeckForRevision();
        if (hasExistingDeck) {
          requestInput.currentSlideIndex = getActiveIndex(state);
          requestInput.currentDeck = buildCurrentDeckSnapshot(instruction);
        }
        const { sessionId } = await executeBackendTurn(requestInput, {
          onToolPhase: (kind) => {
            if (kind === 'detected') {
              setGenerationStep('brief', 'running', t('generationReadingBrief'));
            } else if (kind === 'completed') {
              setGenerationStep('brief', 'done');
            } else if (kind === 'research') {
              setGenerationStep('proof', 'running', t('generationChoosingProof'));
            }
            // 'round' = new model round; don't override the current phase —
            // the stream entries already show what the agent is doing.
          },
          onTextProgress: (buffer) => noteTextStreamProgress(buffer, progressShim, lastStreamPhase),
          // Progressive preview: when the agent writes a slide file during the
          // turn, read it from disk and show it immediately so the user sees
          // pages appearing one by one instead of all at once at the end.
          onSlideFileWritten: project
            ? async (slideNumber) => {
                const html = await tryReadDeckSlideFileWithRetry(project, slideNumber);
                if (!html) return;
                progressiveSlides.set(slideNumber, html);
                // Read project.json lazily; cache it so we don't re-read on
                // every slide write. Re-read only if a new slide number beyond
                // the cached plan's outline appears.
                const planOutlineLen = Array.isArray(progressivePlan?.outline) ? progressivePlan.outline.length : 0;
                if (!progressivePlan || slideNumber > planOutlineLen) {
                  const fresh = await tryReadDeckPlanFile(project);
                  if (fresh) {
                    progressivePlan = fresh;
                    const planTitle = resolveDeckTitle({ plan: fresh, state, instruction });
                    if (!isEphemeralDeckTitle(planTitle)) {
                      state.title = planTitle;
                    }
                  }
                }
                const plan = progressivePlan || {};
                // Build an incremental payload from all slides known so far.
                const knownSlides = [...progressiveSlides.entries()]
                  .sort((a, b) => a[0] - b[0])
                  .map(([number, slideHtml]) => ({
                    id: `ppt-live-slide-${number}`,
                    slideNumber: number,
                    title: typeof plan?.outline?.[number - 1] === 'string'
                      ? plan.outline[number - 1]
                      : (plan?.outline?.[number - 1]?.title || `${t('newSlideTitle')} ${number}`),
                    html: slideHtml,
                  }));
                const deckTitle = resolveDeckTitle({
                  plan,
                  state,
                  instruction,
                  slides: knownSlides,
                });
                setGenerationStep('design', 'running', t('generationSlideReady', {
                  slide: slideNumber,
                  total: knownSlides.length,
                }));
                setStatus(t('generationRenderingSlide', {
                  slide: slideNumber,
                  total: plan?.outline?.length || knownSlides.length,
                }));
                addGenerationEvent({
                  title: t('generationSlideReady', {
                    slide: slideNumber,
                    total: plan?.outline?.length || knownSlides.length,
                  }),
                  detail: '',
                  kind: 'slide',
                });
                applyDeckPayload({
                  title: deckTitle,
                  language: plan?.language || '',
                  outline: planOutlineTitles(plan || {}),
                  researchReport: plan?.researchReport || null,
                  design: plan?.design || {},
                  slides: knownSlides,
                }, { instruction });
                state.activeSlideId = `ppt-live-slide-${slideNumber}`;
                state.selectedElementId = '';
                rerender();
              }
            : undefined,
        }, {
          sessionId: retrySession?.id || undefined,
          appDataWorkspace: retrySession?.project?.workspaceSubdir,
          resultKind: project ? 'text' : undefined,
        });
        retrySession.id = sessionId || retrySession.id;
        state.agentSession = {
          id: retrySession.id || '',
          workspaceSubdir: retrySession?.project?.workspaceSubdir || '',
          runId: retrySession?.project?.runId || '',
          skillKey: PPT_DESIGN_SKILL_KEY,
        };

        // The agent delivered through files; read them back.
        addGenerationEvent({ title: t('generationParsingDeck'), detail: '', kind: 'parsing' });
        setStatus(t('generationParsingDeck'));
        setGenerationStep('design', 'running', t('generationDesigningLayouts'));
        const payload = project
          ? await readDeckFromProjectFiles(project)
          : null;
        if (!payload) throw new Error('PPT Live agent did not produce a readable deck');
        applyDeckPayload(payload, { instruction });
        await saveHistorySnapshot(`agent:${operation}`);
        addGenerationEvent({ title: t('processEventDone'), detail: '', kind: 'done' });
        setGenerationStep('spine', 'done');
        setGenerationStep('proof', 'done');
        setGenerationStep('design', 'done');
        setGenerationStep('compile', 'done', t('generationCompiled'));
        finishGenerationUi(t('deckReady'));
        completed = true;
        rerender();
        await persist(true);
        break;
      } catch (error) {
        lastError = error;
        if (isUnknownSessionBackendError(error)) retrySession.id = null;
        else if (error?.pptLiveSessionId) retrySession.id = error.pptLiveSessionId;
        if (!isRetryableBackendError(error) || attempt >= PPT_BACKEND_MAX_ATTEMPTS) throw error;
        runtime().log?.warn?.('PPT Live cowork generation attempt failed, retrying', {
          attempt,
          maxAttempts: PPT_BACKEND_MAX_ATTEMPTS,
          continueInSession: Boolean(retrySession.id),
          error: String(error),
        });
      }
    }
  } finally {
    const ownsEpoch = !isDeckEpochStale(runEpoch);
    if (ownsEpoch) {
      if (state.generation.active && !completed) state.generation.active = false;
      setBusy(false);
    }
    renderGeneration(state);
    renderGenerationOverlay(state);
  }
}

function prepareAgentGenerationSurface(operation, instruction) {
  setStatus(t('generationAgentWorking'));
  addGenerationEvent({ title: t('generationAgentWorking'), detail: compactText(instruction || ''), kind: 'start' });
  if (operation === 'auto' && (isDefaultDraft() || isStarterDeck())) {
    state.title = t('agentWorkingTitle');
  }
  rerender();
}

function showAgentWorkingCanvas(instruction) {
  try {
    const slide = normalizeSlide({
      id: uid('agent-working-slide'),
      title: t('agentWorkingTitle'),
      subtitle: '',
      kicker: t('agentWorkingKicker'),
      claim: t('agentWorkingClaim'),
      proofObject: t('agentWorkingProof'),
      supportNote: instruction || t('agentWorkingDetail'),
      sourceNote: t('agentWorkingSourceNote'),
      notes: t('agentWorkingSourceNote'),
      layout: 'brief',
      theme: {
        background: '#fbfcff',
        ink: '#111827',
        muted: '#5b6575',
        primary: '#ff4f46',
        accent: '#14b8a6',
        panel: '#ffffff',
      },
      elements: [
        {
          type: 'text',
          text: t('agentWorkingTitle'),
          x: 9,
          y: 16,
          w: 72,
          h: 13,
          style: { fontSize: 32, fontWeight: 820, color: 'ink', background: 'transparent', borderRadius: 0, opacity: 1, align: 'left' },
        },
        {
          type: 'text',
          text: t('agentWorkingDetail'),
          x: 10,
          y: 34,
          w: 58,
          h: 10,
          style: { fontSize: 16, fontWeight: 650, color: 'muted', background: 'transparent', borderRadius: 0, opacity: 1, align: 'left' },
        },
        {
          type: 'list',
          items: [
            t('generationReadingBrief'),
            t('generationWritingClaims'),
            t('generationChoosingProof'),
            t('generationDesigningLayouts'),
          ],
          x: 10,
          y: 50,
          w: 50,
          h: 29,
          style: { fontSize: 18, fontWeight: 650, color: 'ink', background: 'transparent', borderRadius: 0, opacity: 1, align: 'left' },
        },
        {
          type: 'shape',
          x: 67,
          y: 20,
          w: 22,
          h: 52,
          style: { fontSize: 18, fontWeight: 700, color: 'accent', background: 'primary', borderRadius: 24, opacity: 0.12, align: 'center' },
        },
        {
          type: 'metric',
          text: t('agentWorkingMetric'),
          label: t('agentWorkingMetricLabel'),
          x: 65,
          y: 42,
          w: 26,
          h: 20,
          style: { fontSize: 34, fontWeight: 830, color: 'primary', background: 'panel', borderRadius: 14, opacity: 1, align: 'left' },
        },
      ],
    }, 0, { ...state, slides: [] });
    state.title = t('agentWorkingTitle');
    state.slides = [slide];
    state.outline = [slide.title];
    state.activeSlideId = slide.id;
    state.selectedElementId = getActiveSlide(state)?.elements[0]?.id || '';
    setStatus(t('generationAgentWorking'));
    addGenerationEvent(t('generationAgentWorking'));
    rerender();
  } catch (error) {
    runtime().log?.warn?.('PPT Live working canvas failed', { instruction, error: String(error) });
  }
}

const SILENT_TOOL_EVENT_TYPES = new Set([
  'ParamsPartial',
  'Queued',
  'Waiting',
  'Progress',
  'Streaming',
  'StreamChunk',
  'Confirmed',
  'Rejected',
  'EarlyDetected',
  'Started',
]);

const SILENT_COMPLETED_TOOL_NAMES = new Set([
  'read',
  'write',
  'grep',
  'glob',
  'list',
  'todowrite',
  'todo_write',
  'skill',
  'bash',
  'shell',
  'edit',
  'delete',
  'apply_patch',
  'strreplace',
  'search_replace',
]);

/** Tools whose raw calls are too noisy for the user-facing timeline. */
const STREAM_HIDDEN_TOOLS = new Set([
  'todowrite',
  'todo_write',
  'grep',
  'glob',
  'ls',
  'list',
  'execcommand',
  'bash',
  'shell',
]);

/** Shorten an absolute file path to its last two segments (e.g. slides/slide-01.html). */
function shortFilePath(path) {
  if (!path) return '';
  const parts = String(path).replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length <= 2) return parts.join('/');
  return parts.slice(-2).join('/');
}

function friendlyToolName(name) {
  const raw = String(name || '').trim();
  if (!raw) return t('eventUnknownTool');
  if (/^skill$/i.test(raw)) return t('eventToolSkillName');
  if (/^websearch$/i.test(raw)) return t('eventToolWebSearchName');
  if (/^webfetch$/i.test(raw)) return t('eventToolWebFetchName');
  return raw;
}

function completedToolProgressTitle(rawToolName, options = {}) {
  const name = String(rawToolName || '').trim().toLowerCase();
  if (!name || SILENT_COMPLETED_TOOL_NAMES.has(name)) return '';
  if (name === 'websearch') return options.isSubagent ? t('eventSubagentWebSearchDone') : t('eventToolWebSearchDone');
  if (name === 'webfetch') return options.isSubagent ? t('eventSubagentWebFetchDone') : t('eventToolWebFetchDone');
  if (name === 'task') return t('eventToolTaskDone');
  return '';
}

function shouldLogToolEvent(toolEvent, loggedToolEvents, options = {}) {
  const normalized = normalizeToolEvent(toolEvent);
  const eventType = normalized.event_type || normalized.eventType || '';
  if (SILENT_TOOL_EVENT_TYPES.has(eventType)) return false;
  const toolName = String(normalized.tool_name || normalized.toolName || 'tool').toLowerCase();
  if (eventType === 'Completed' && !completedToolProgressTitle(toolName, options)) return false;
  const path = resolveToolEventFilePath(normalized) || String(
    (normalized.params && typeof normalized.params === 'object'
      ? (normalized.params.command || '')
      : ''),
  ).trim();
  const key = path ? `${toolName}:${path}:${eventType}` : `${toolName}:${eventType}`;
  if (loggedToolEvents.has(key)) return false;
  loggedToolEvents.add(key);
  return eventType === 'Completed'
    || eventType === 'Failed'
    || eventType === 'Cancelled'
    || eventType === 'ConfirmationNeeded';
}

function createGenerationProgressTracker() {
  let lastProgressLogAt = 0;
  let lastProgressTitle = '';
  return {
    get lastProgressLogAt() {
      return lastProgressLogAt;
    },
    touch() {
      lastProgressLogAt = Date.now();
    },
    note(title, detail = '', kind = 'phase', minIntervalMs = 0) {
      const now = Date.now();
      const sameTitle = title === lastProgressTitle;
      if (minIntervalMs > 0 && sameTitle && now - lastProgressLogAt < minIntervalMs) return false;
      lastProgressTitle = title;
      lastProgressLogAt = now;
      addGenerationEvent({ title, detail, kind });
      return true;
    },
  };
}

/** Compact a tool's input params into a human-readable single line for the live agent stream. */
function summarizeToolParams(toolName, params = {}) {
  const name = String(toolName || '').toLowerCase();
  const p = params && typeof params === 'object' ? params : {};
  if (name === 'websearch') return compactText(String(p.query || p.prompt || ''), 160);
  if (name === 'webfetch' || name === 'mcp__web_reader__webreader') return compactText(String(p.url || ''), 160);
  if (name === 'read') return shortFilePath(String(p.file_path || p.path || ''));
  if (name === 'write' || name === 'edit') return shortFilePath(String(p.file_path || p.path || ''));
  if (name === 'grep' || name === 'glob') return compactText(String(p.pattern || ''), 120);
  if (name === 'skill') return compactText(String(p.command || p.skill || p.key || ''), 120);
  if (name === 'task') return compactText(String(p.description || p.prompt || ''), 200);
  if (name === 'todowrite' || name === 'todo_write') {
    const todos = Array.isArray(p.todos) ? p.todos : [];
    return compactText(todos.map((todo) => todo?.content || '').filter(Boolean).join(' | '), 200);
  }
  if (name === 'execcommand' || name === 'bash' || name === 'shell') return compactText(String(p.cmd || p.command || ''), 160);
  // Generic fallback: pick the first string-valued field.
  const firstVal = Object.values(p).find((val) => typeof val === 'string' && val.trim());
  return compactText(String(firstVal || ''), 160);
}

/** Compact a tool's result into a human-readable single line for the live agent stream. */
function summarizeToolResult(toolName, result = {}) {
  const name = String(toolName || '').toLowerCase();
  const r = result && typeof result === 'object' ? result : {};
  if (name === 'websearch') {
    const results = Array.isArray(r.results) ? r.results : [];
    return results.length ? `${results.length} 条结果` : '';
  }
  if (name === 'webfetch' || name === 'mcp__web_reader__webreader') {
    const len = String(r.content || r.text || r.markdown || '').length;
    return len ? `${len > 1000 ? Math.round(len / 1000) + 'k' : len} 字符` : '';
  }
  if (name === 'read') {
    const lines = Number(r.lineCount || (Array.isArray(r.lines) ? r.lines.length : 0));
    return lines ? `${lines} 行` : '';
  }
  // write/edit completion is signaled by the slide-ready event; skip the
  // raw "written" message to keep the timeline clean.
  if (name === 'write' || name === 'edit') return '';
  if (name === 'grep' || name === 'glob') return '';
  if (name === 'skill') return '';
  if (name === 'task') return compactText(String(r.result || r.message || ''), 160);
  return '';
}

function inferGenerationPhaseFromBuffer(buffer) {
  const text = String(buffer || '');
  if (/"html"\s*:/.test(text)) return 'design';
  if (/"slides"\s*:/.test(text)) return 'proof';
  if (/"outline"\s*:/.test(text)) return 'spine';
  return 'spine';
}

function generationPhaseMessage(phase) {
  switch (phase) {
    case 'proof':
      return t('generationChoosingProof');
    case 'design':
      return t('generationDesigningLayouts');
    default:
      return t('generationWritingClaims');
  }
}

function extractJsonArraySection(text, key) {
  const pattern = new RegExp(`"${key}"\\s*:\\s*\\[`);
  const match = pattern.exec(String(text || ''));
  if (!match) return '';
  return String(text).slice(match.index + match[0].length);
}

function countJsonArrayObjects(section) {
  let depth = 0;
  let objects = 0;
  let inString = false;
  let escaped = false;
  for (let i = 0; i < section.length; i += 1) {
    const ch = section[i];
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') {
      inString = true;
      continue;
    }
    if (ch === '{') {
      if (depth === 0) objects += 1;
      depth += 1;
    } else if (ch === '}') {
      depth = Math.max(0, depth - 1);
    } else if (ch === ']' && depth === 0) {
      break;
    }
  }
  return objects;
}

function countJsonArrayStrings(section) {
  let depth = 0;
  let count = 0;
  let inString = false;
  let escaped = false;
  let stringAtArrayDepth = false;
  for (let i = 0; i < section.length; i += 1) {
    const ch = section[i];
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === '"') {
        inString = false;
        if (stringAtArrayDepth) count += 1;
        stringAtArrayDepth = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
      stringAtArrayDepth = depth === 0;
      continue;
    }
    if (ch === '[') depth += 1;
    else if (ch === ']') {
      if (depth === 0) break;
      depth = Math.max(0, depth - 1);
    }
  }
  return count;
}

function estimateGenerationSlideCount(buffer, phase) {
  const text = String(buffer || '');
  let count = 0;

  if (phase === 'design') {
    count = (text.match(/"html"\s*:/g) || []).length;
  }
  if (count === 0 && (phase === 'design' || phase === 'proof')) {
    const slidesSection = extractJsonArraySection(text, 'slides');
    if (slidesSection) count = countJsonArrayObjects(slidesSection);
  }
  if (count === 0 && phase === 'spine') {
    const outlineSection = extractJsonArraySection(text, 'outline');
    if (outlineSection) count = countJsonArrayStrings(outlineSection);
  }

  return count;
}

function updateGenerationSlideProgress(buffer, phase) {
  const count = estimateGenerationSlideCount(buffer, phase);
  if (count > 0) state.generation.draftedCount = count;
  renderGeneration(state);
  renderGenerationOverlay(state);
}

function estimateGenerationDetail(buffer, phase) {
  const count = estimateGenerationSlideCount(buffer, phase);
  return count > 0 ? t('generationSlideProgress', { count }) : '';
}

function noteTextStreamProgress(buffer, progressTracker, lastPhaseRef) {
  // The file-protocol agent writes slides via tool calls (Write/Edit), not
  // inline JSON text. Inferring a phase from the text buffer was unreliable
  // and produced mismatched labels (e.g. "生成大纲" during slide editing).
  // Only touch the progress tracker; let tool events drive the phase labels.
  progressTracker.touch();
  void lastPhaseRef;
}

function describeToolEvent(event, options = {}) {
  const toolEvent = normalizeToolEvent(event.toolEvent || {});
  const eventType = toolEvent.event_type || toolEvent.eventType || 'ToolEvent';
  const rawToolName = String(toolEvent.tool_name || toolEvent.toolName || '').trim();
  if (eventType === 'Completed') {
    const title = completedToolProgressTitle(rawToolName, options);
    if (!title) return null;
    return { title, detail: '', kind: 'tool' };
  }
  if (eventType === 'Failed' || eventType === 'Cancelled') {
    return {
      title: t('eventToolFailedUser'),
      detail: '',
      kind: 'error',
    };
  }
  if (eventType === 'ConfirmationNeeded') {
    return {
      title: t('processEventWaiting'),
      detail: '',
      kind: 'tool',
    };
  }
  return null;
}

function userFacingToolDetail(eventType, toolEvent) {
  if (eventType === 'Failed') return compactText(toolEvent.error || t('backendGenerationFailed'));
  if (eventType === 'Completed') return '';
  if (eventType === 'Progress') return compactText(toolEvent.message || '');
  return '';
}

function resolveToolEventFilePath(toolEvent, toolTrace = []) {
  const params = toolEvent?.params && typeof toolEvent.params === 'object' ? toolEvent.params : {};
  const directPath = String(params.file_path || params.path || '').trim();
  if (directPath) return directPath;

  const result = toolEvent?.result && typeof toolEvent.result === 'object' ? toolEvent.result : {};
  const resultPath = String(result.file_path || result.path || '').trim();
  if (resultPath) return resultPath;

  const toolId = String(toolEvent?.tool_id || toolEvent?.toolId || '').trim();
  if (!toolId) return '';

  const started = [...toolTrace].reverse().find((entry) => (
    entry.eventType === 'Started'
    && String(entry.toolId || entry.tool_id || '') === toolId
  ));
  const startedParams = started?.params && typeof started.params === 'object' ? started.params : {};
  return String(startedParams.file_path || startedParams.path || '').trim();
}

function normalizeToolEvent(toolEvent) {
  if (toolEvent.event_type || toolEvent.eventType || toolEvent.tool_name || toolEvent.toolName) return toolEvent;
  const keys = [
    'EarlyDetected',
    'ParamsPartial',
    'Queued',
    'Waiting',
    'Started',
    'Progress',
    'Streaming',
    'StreamChunk',
    'ConfirmationNeeded',
    'Confirmed',
    'Rejected',
    'Completed',
    'Failed',
    'Cancelled',
  ];
  const key = keys.find((candidate) => toolEvent && Object.prototype.hasOwnProperty.call(toolEvent, candidate));
  if (!key) return toolEvent || {};
  const value = toolEvent[key] || {};
  return { ...value, event_type: key };
}

function compactText(value, limit = 180) {
  const text = String(value || '').replace(/\s+/g, ' ').trim();
  if (!text) return '';
  return text.length > limit ? `${text.slice(0, limit - 1)}...` : text;
}

function trackBackendRun(sessionId, turnId) {
  if (!sessionId || !turnId) return;
  const exists = backendRuns.some((run) => run.sessionId === sessionId && run.turnId === turnId);
  if (!exists) backendRuns.push({ sessionId, turnId });
}

function untrackBackendRun(sessionId, turnId) {
  backendRuns = backendRuns.filter((run) => !(run.sessionId === sessionId && run.turnId === turnId));
}

function isDeckEpochStale(epoch) {
  return epoch !== deckEpoch;
}

async function cancelTrackedBackendRuns() {
  const runs = [...backendRuns];
  backendRuns = [];
  if (!runs.length || !runtime().backend?.cancel) return;
  await Promise.all(runs.map(async (run) => {
    try {
      await runtime().backend.cancel(run.sessionId, run.turnId);
    } catch (error) {
      runtime().log?.warn?.('PPT Live backend cancel failed', {
        sessionId: run.sessionId,
        turnId: run.turnId,
        error: String(error),
      });
    }
  }));
}

async function stopAllBackendRuns(fromTimeout = false, options = {}) {
  const hadRuns = backendRuns.length > 0;
  deckEpoch += 1;
  await cancelTrackedBackendRuns();
  state.generation.active = false;
  state.generation.steps = state.generation.steps.map((step) => step.status === 'running' ? { ...step, status: 'error' } : step);
  if (!options.silent && hadRuns) {
    setStatus(fromTimeout ? t('generationTimedOut') : t('generationStopped'));
    addGenerationEvent(fromTimeout ? t('generationTimedOut') : t('generationStopped'));
  }
  setBusy(false);
  renderGeneration(state);
  renderGenerationOverlay(state);
  if (!options.silent) await persist(true);
}

async function stopBackendRun(fromTimeout = false) {
  await stopAllBackendRuns(fromTimeout);
}

function applyDeckPayload(payload, options = {}) {
  if (applyDeckPatchPayload(payload, options)) {
    if (payload.researchReport) applyResearchReport(payload.researchReport);
    if (payload.design?.palette && typeof payload.design.palette === 'object') {
      state.deckPalette = payload.design.palette;
    }
    return;
  }
  const resolvedTitle = resolveDeckTitle({
    payload,
    state,
    instruction: options.instruction || '',
    slides: payload?.slides || [],
  });
  const htmlSlides = normalizeHtmlSlides(payload);
  if (htmlSlides.length) {
    state.title = resolvedTitle;
    state.slides = htmlSlides.map((slide, index) => normalizeSlide(slide, index, {
      ...state,
      slides: htmlSlides,
    }));
    state.outline = state.slides.map((slide) => slide.title);
    state.activeSlideId = state.slides[0]?.id || '';
    state.selectedElementId = '';
  } else if (!Array.isArray(payload?.slides) || payload.slides.length === 0) {
    throw new Error('PPT Live deck payload has no slides');
  } else {
    state.title = resolvedTitle;
    state.slides = payload.slides.map((slide, index) => normalizeSlide({
      ...slide,
      html: slide.html || slide.sourceHtml || slide.slideHtml || '',
    }, index, {
      ...state,
      slides: payload.slides,
    }));
    state.outline = state.slides.map((slide) => slide.title);
    state.activeSlideId = state.slides[0]?.id || '';
    state.selectedElementId = state.slides[0]?.elements[0]?.id || '';
  }
  if (Array.isArray(payload.outline) && payload.outline.length) {
    state.outline = payload.outline.map(outlineItemTitle).filter(Boolean);
  }
  if (payload.researchReport) applyResearchReport(payload.researchReport);
  if (payload.design?.palette && typeof payload.design.palette === 'object') {
    state.deckPalette = payload.design.palette;
  }
}

function applyResearchReport(report) {
  state.sources = {
    ...state.sources,
    facts: report.verifiedFacts || state.sources?.facts || [],
    warnings: report.warnings || state.sources?.warnings || [],
    summary: report.summary || state.sources?.summary || '',
    fetchedAt: Date.now(),
  };
}

function payloadPatchChanges(payload) {
  if (Array.isArray(payload?.deckPatch?.changes)) return payload.deckPatch.changes;
  if (Array.isArray(payload?.patch?.changes)) return payload.patch.changes;
  if (Array.isArray(payload?.changes)) return payload.changes;
  if (Array.isArray(payload?.patches)) return payload.patches;
  return [];
}

function resolvePatchIndex(change, slides, fallback = 0) {
  const slideId = String(change?.slideId || change?.id || change?.targetSlideId || change?.targetId || '').trim();
  if (slideId) {
    const byId = slides.findIndex((slide) => slide.id === slideId);
    if (byId >= 0) return byId;
  }
  const rawNumber = Number(change?.slideNumber ?? change?.pageNumber);
  if (Number.isFinite(rawNumber) && rawNumber > 0) {
    return clamp(Math.round(rawNumber) - 1, 0, Math.max(0, slides.length - 1));
  }
  const rawIndex = Number(change?.slideIndex ?? change?.index ?? change?.targetSlideIndex);
  if (Number.isFinite(rawIndex)) {
    if (rawIndex >= slides.length && rawIndex - 1 >= 0 && rawIndex - 1 < slides.length) {
      return Math.round(rawIndex) - 1;
    }
    return clamp(Math.round(rawIndex), 0, Math.max(0, slides.length - 1));
  }
  return clamp(fallback, 0, Math.max(0, slides.length - 1));
}

function resolveInsertIndex(change, slides) {
  const afterId = String(change?.afterSlideId || '').trim();
  if (afterId) {
    const afterIndex = slides.findIndex((slide) => slide.id === afterId);
    if (afterIndex >= 0) return afterIndex + 1;
  }
  const beforeId = String(change?.beforeSlideId || '').trim();
  if (beforeId) {
    const beforeIndex = slides.findIndex((slide) => slide.id === beforeId);
    if (beforeIndex >= 0) return beforeIndex;
  }
  if (change?.afterSlideNumber) {
    return clamp(Number(change.afterSlideNumber), 0, slides.length);
  }
  if (change?.beforeSlideNumber) {
    return clamp(Number(change.beforeSlideNumber) - 1, 0, slides.length);
  }
  if (change?.slideNumber) {
    return clamp(Number(change.slideNumber) - 1, 0, slides.length);
  }
  if (change?.slideIndex !== undefined) {
    return clamp(Number(change.slideIndex), 0, slides.length);
  }
  return Math.min(slides.length, getActiveIndex(state) + 1);
}

function normalizePatchSlide(change, existing, index, slides) {
  const rawSlide = change?.slide || change?.replacement || change?.newSlide || change?.payload || change;
  if (!rawSlide || typeof rawSlide !== 'object') return null;
  const slide = {
    ...(existing || {}),
    ...rawSlide,
    id: rawSlide.id || rawSlide.slideId || existing?.id || uid('html-slide'),
    html: rawSlide.html || rawSlide.sourceHtml || rawSlide.slideHtml || existing?.html || '',
  };
  return normalizeSlide(slide, index, { ...state, slides });
}

function applyDeckPatchPayload(payload, options = {}) {
  const changes = payloadPatchChanges(payload);
  if (!changes.length) return false;
  const slides = clone(state.slides || []);
  const changedIds = [];
  let applied = 0;
  changes.forEach((change) => {
    const op = String(change?.op || change?.operation || change?.type || 'replace_slide').toLowerCase();
    if (op === 'delete_slide' || op === 'delete' || op === 'remove_slide' || op === 'remove') {
      if (!slides.length) return;
      const index = resolvePatchIndex(change, slides, getActiveIndex(state));
      const [removed] = slides.splice(index, 1);
      if (removed?.id) changedIds.push(removed.id);
      applied += 1;
      return;
    }
    if (op === 'insert_slide' || op === 'insert' || op === 'add_slide' || op === 'add') {
      const index = resolveInsertIndex(change, slides);
      const slide = normalizePatchSlide(change, null, index, slides);
      if (!slide) return;
      slides.splice(index, 0, slide);
      changedIds.push(slide.id);
      applied += 1;
      return;
    }
    const index = resolvePatchIndex(change, slides, getActiveIndex(state));
    const existing = slides[index];
    const slide = normalizePatchSlide(change, existing, index, slides);
    if (!slide) return;
    slides[index] = slide;
    changedIds.push(slide.id);
    applied += 1;
  });
  if (!applied) throw new Error('PPT Live deck patch had no applicable changes');
  state.title = resolveDeckTitle({
    payload,
    state,
    instruction: options.instruction || '',
    slides,
  });
  state.slides = slides.map((slide, index) => normalizeSlide(slide, index, { ...state, slides }));
  state.outline = Array.isArray(payload.outline) && payload.outline.length
    ? payload.outline.map(String)
    : state.slides.map((slide) => slide.title);
  const activeId = changedIds.find((id) => state.slides.some((slide) => slide.id === id));
  state.activeSlideId = activeId || state.slides[Math.min(getActiveIndex(state), state.slides.length - 1)]?.id || state.slides[0]?.id || '';
  state.selectedElementId = getActiveSlide(state)?.elements?.[0]?.id || '';
  return true;
}

function normalizeHtmlSlides(payload) {
  const candidates = [];
  if (Array.isArray(payload?.htmlSlides)) candidates.push(...payload.htmlSlides);
  if (Array.isArray(payload?.slides)) candidates.push(...payload.slides.filter((slide) => slide?.html || slide?.sourceHtml || slide?.slideHtml));
  return candidates.map((slide, index) => {
    const html = String(slide?.html || slide?.sourceHtml || slide?.slideHtml || '').trim();
    if (!html) return null;
    return {
      id: slide.id || slide.slideId || uid('html-slide'),
      title: String(slide.title || slide.label || `${t('newSlideTitle')} ${index + 1}`),
      subtitle: String(slide.subtitle || ''),
      kicker: String(slide.kicker || ''),
      claim: String(slide.claim || slide.title || ''),
      proofObject: String(slide.proofObject || ''),
      supportNote: String(slide.supportNote || ''),
      sourceNote: String(slide.sourceNote || ''),
      notes: String(slide.notes || ''),
      layout: 'html',
      theme: slide.theme || {},
      html,
      elements: [],
    };
  }).filter(Boolean);
}

function pickParseableBackendText(...candidates) {
  for (const raw of candidates) {
    const text = String(raw || '').trim();
    if (!text) continue;
    try {
      extractBackendJson(text);
      return text;
    } catch {
      // try next candidate
    }
  }
  return String(candidates.find((raw) => String(raw || '').trim()) || '').trim();
}

async function waitForBackendResultOrPersistedText(waitForResult, sessionId, turnId, activity = null, options = {}) {
  const host = runtime();
  const expectJson = options.expectJson !== false;
  if (!sessionId || !turnId || !host.backend?.turnText) return waitForResult;
  let settled = false;
  const streamedResult = Promise.resolve(waitForResult).finally(() => {
    settled = true;
  });
  const persistedResult = new Promise((resolve, reject) => {
    const startedAt = Date.now();
    // Give up only when the backend turn looks dead (no events for a while),
    // never on a short wall-clock cap while the agent is still making progress.
    const idleTimeoutMs = 5 * 60 * 1000;
    const fallbackPollIdleMs = 10 * 1000;
    const absoluteMaxWaitMs = 60 * 60 * 1000;
    const lastEventAt = () => Number(activity?.lastEventAt || startedAt);
    const poll = async () => {
      while (!settled && Date.now() - startedAt < absoluteMaxWaitMs) {
        const idleForMs = Date.now() - lastEventAt();
        if (idleForMs > idleTimeoutMs) break;
        if (idleForMs < fallbackPollIdleMs) {
          await new Promise((resolveDelay) => setTimeout(
            resolveDelay,
            Math.min(1000, fallbackPollIdleMs - idleForMs),
          ));
          continue;
        }
        try {
          const result = await host.backend.turnText(sessionId, turnId);
          const text = String(result?.text || '').trim();
          if (text) {
            if (!expectJson) {
              // File-protocol turns only reply with a status line; any
              // persisted text means the turn produced its answer.
              resolve({ answer: text, thinking: '' });
              return;
            }
            try {
              extractBackendJson(text);
              resolve({ answer: text, thinking: '' });
              return;
            } catch {
              // Keep waiting until the persisted assistant text becomes a complete deck JSON.
            }
          }
        } catch {
          // The turn may not be persisted yet.
        }
        await new Promise((resolveDelay) => setTimeout(resolveDelay, 2000));
      }
      if (!settled) reject(new Error('PPT Live backend did not publish a final deck JSON'));
    };
    void poll();
  });
  return Promise.race([streamedResult, persistedResult]);
}

async function resolveBackendTurnText(sessionId, turnId, streamedText, streamedThinking = '') {
  const startedAt = Date.now();
  const maxWaitMs = 25000;
  const answer = String(streamedText || '').trim();
  const thinking = String(streamedThinking || '').trim();
  const tryPick = () => pickParseableBackendText(answer, thinking, `${answer}\n${thinking}`.trim());
  let merged = tryPick();
  if (merged) {
    try {
      extractBackendJson(merged);
      return merged;
    } catch {
      // fall through to persisted turn text
    }
  }
  const host = runtime();
  if (!sessionId || !turnId || !host.backend?.turnText) {
    if (!merged) throw new Error('PPT Live backend produced no text');
    return merged;
  }
  let attempt = 0;
  while (Date.now() - startedAt < maxWaitMs && attempt < 8) {
    attempt += 1;
    try {
      const result = await Promise.race([
        host.backend.turnText(sessionId, turnId),
        new Promise((_, reject) => {
          setTimeout(() => reject(new Error('turnText timeout')), 4000);
        }),
      ]);
      const persisted = String(result?.text || '').trim();
      merged = pickParseableBackendText(persisted, merged, thinking, answer);
      if (merged) {
        extractBackendJson(merged);
        return merged;
      }
    } catch (error) {
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  if (!merged) throw new Error('PPT Live backend produced no text');
  return merged;
}

function extractBackendJson(text) {
  const raw = String(text || '').trim();
  if (!raw) throw new Error('PPT Live backend produced no text');
  try {
    return JSON.parse(raw);
  } catch {
    const fenced = raw.match(/```(?:json)?\s*([\s\S]*?)```/i);
    if (fenced) return JSON.parse(fenced[1]);
    const start = raw.indexOf('{');
    const end = raw.lastIndexOf('}');
    if (start >= 0 && end > start) return JSON.parse(raw.slice(start, end + 1));
    throw new Error('PPT Live backend returned invalid JSON');
  }
}

function isRoundBudgetBackendError(error) {
  const raw = String(error?.message || error || '');
  return /ppt_live:\/\/round-budget-exhausted|exhausted its \d+-round tool budget|tool budget before producing deck JSON/i.test(raw);
}

function isTimeoutBackendError(error) {
  const message = String(error || '');
  return message.includes('timed out');
}

function isStoppedBackendError(error) {
  const message = String(error || '');
  return message.includes('dialog-turn-cancelled')
    || message.includes('Generation stopped');
}

async function applyAiAction(action, options = {}) {
  const reviseExistingDeck = hasUsableDeckForRevision();
  if (options.readBrief !== false) updateBriefFromInputs({ includeTopic: !reviseExistingDeck });
  const instruction = [action, promptValue()].filter(Boolean).join(': ');
  if (!instruction) {
    setStatus(t('promptRequired'));
    return;
  }
  try {
    await runPptLiveBackend('revise_slide', instruction, { includeTopic: !reviseExistingDeck });
  } catch (error) {
    if (isStoppedBackendError(error)) return;
    runtime().log?.warn?.('PPT Live backend slide revision failed', { action, error: String(error) });
    failGenerationFromError(error);
    await persist(true);
  }
}

async function reviseCurrentSlide() {
  await applyAiAction('redesign', { readBrief: false });
}

async function reviseDeck() {
  const instruction = promptValue();
  if (!instruction) {
    setStatus(t('promptRequired'));
    return;
  }
  const reviseExistingDeck = hasUsableDeckForRevision();
  updateBriefFromInputs({ includeTopic: !reviseExistingDeck });
  try {
    await runPptLiveBackend('revise_deck', instruction, { includeTopic: !reviseExistingDeck });
    return;
  } catch (error) {
    if (isStoppedBackendError(error)) return;
    runtime().log?.warn?.('PPT Live backend revision failed', { error: String(error) });
    failGenerationFromError(error);
    await persist(true);
  }
}

async function insertSlideFromPrompt() {
  const instruction = promptValue();
  if (!instruction) {
    setStatus(t('promptRequired'));
    return;
  }
  const reviseExistingDeck = hasUsableDeckForRevision();
  try {
    await runPptLiveBackend('insert_slide', instruction, { includeTopic: !reviseExistingDeck });
  } catch (error) {
    if (isStoppedBackendError(error)) return;
    runtime().log?.warn?.('PPT Live backend insert slide failed', { error: String(error) });
    failGenerationFromError(error);
    await persist(true);
  }
}

async function deleteSlideFromPrompt() {
  const instruction = promptValue() || t('deleteSlideDefaultPrompt');
  if (state.slides.length <= 1) {
    setStatus(t('cannotDelete'));
    return;
  }
  const reviseExistingDeck = hasUsableDeckForRevision();
  try {
    await runPptLiveBackend('delete_slide', instruction, { includeTopic: !reviseExistingDeck });
  } catch (error) {
    if (isStoppedBackendError(error)) return;
    runtime().log?.warn?.('PPT Live backend delete slide failed', { error: String(error) });
    failGenerationFromError(error);
    await persist(true);
  }
}

function replaceActiveSlide(nextSlide) {
  if (!nextSlide) return;
  const index = getActiveIndex(state);
  state.slides[index] = normalizeSlide(nextSlide, index, state);
  state.outline[index] = state.slides[index].title;
  state.selectedElementId = state.slides[index].elements[0]?.id || '';
}

async function restyleDeck() {
  updateBriefFromInputs({ includeTopic: !hasUsableDeckForRevision() });
  if ((state.slides || []).some((slide) => String(slide?.html || '').trim())) {
    const instruction = `Restyle the existing deck without changing its facts or narrative. Apply these exact settings to every slide HTML: ${JSON.stringify(buildGenerationStyle())}. Preserve each page's informationIntent and visualStrategy while making the deck visually coherent.`;
    try {
      await runPptLiveBackend('revise_deck', instruction, { includeTopic: false });
      return;
    } catch (error) {
      if (isStoppedBackendError(error)) return;
      runtime().log?.warn?.('PPT Live Agent restyle failed', { error: String(error) });
      failGenerationFromError(error);
      await persist(true);
      return;
    }
  }
  state.slides = state.slides.map((slide, index) => normalizeSlide({ ...slide, theme: undefined }, index, state));
  setStatus(t('deckRestyled'));
  rerender();
  await persist(true);
}

function syncSlidesFromOutline() {
  updateBriefFromInputs({ includeTopic: !hasUsableDeckForRevision() });
  const previous = new Map(state.slides.map((slide) => [slide.title, slide]));
  state.slides = state.outline.map((title, index) => {
    const existing = previous.get(title);
    return existing ? normalizeSlide(existing, index, state) : makeSlide(title, index, state.outline.length, state);
  });
  state.activeSlideId = state.slides[0]?.id || '';
  state.selectedElementId = state.slides[0]?.elements[0]?.id || '';
  rerender();
  void persist(true);
}

async function newDeck() {
  deckEpoch += 1;
  await saveHistorySnapshot('before-new');
  await cancelTrackedBackendRuns();
  state.generation.active = false;
  setBusy(false);
  state = createBlankDeckState();
  resetGeneration();
  rerender();
  syncStylePanelFromState(state);
  setStatus(t('blankDeckReady'));
  await persist(true);
}

function createBlankDeckState() {
  return ensureState(createInitialState());
}

function addElement(type) {
  if (!ELEMENT_TYPES.includes(type)) return;
  const slide = getActiveSlide(state);
  if (!slide) return;
  const element = normalizeElement({
    ...defaultElement(type),
    x: 10 + (slide.elements.length % 5) * 4,
    y: 14 + (slide.elements.length % 5) * 4,
  });
  slide.elements.push(element);
  state.selectedElementId = element.id;
  rerender();
  void persist(true);
}

function deleteElement() {
  const slide = getActiveSlide(state);
  if (!slide || !state.selectedElementId) return;
  slide.elements = slide.elements.filter((element) => element.id !== state.selectedElementId);
  state.selectedElementId = slide.elements[0]?.id || '';
  rerender();
  void persist(true);
}

function updateSlideTitleFromElements(slide) {
  const titleElement = slide.elements.find((element) => element.type === 'text' && element.text);
  if (!titleElement) return;
  slide.title = titleElement.text.slice(0, 90);
  state.outline[getActiveIndex(state)] = slide.title;
  if (getActiveIndex(state) === 0) state.title = slide.title;
}

function openPreview() {
  state.presentIndex = getActiveIndex(state);
  renderPresent();
  $('previewDialog')?.showModal();
}

function renderPresent() {
  const slide = state.slides[state.presentIndex] || state.slides[0];
  if ($('presentSlide')) $('presentSlide').innerHTML = slide ? slideHtml(slide) : '';
  if ($('presentCounter')) $('presentCounter').textContent = `${Math.max(1, state.presentIndex + 1)} / ${Math.max(1, state.slides.length)}`;
  ensureCanvasFitted();
}

function movePresent(delta) {
  state.presentIndex = clamp(state.presentIndex + delta, 0, state.slides.length - 1);
  renderPresent();
}

function exportHtml() {
  if (!(state.slides || []).length) {
    setExportStatus(t('exportDeckEmpty'));
    return null;
  }
  updateBriefFromInputs({ includeTopic: !hasUsableDeckForRevision() });
  const filename = downloadHtmlDeck(state);
  setExportStatus(t('exportSavedTo', { path: filename }));
  return filename;
}

function ensureExportableDeck() {
  updateBriefFromInputs({ includeTopic: !hasUsableDeckForRevision() });
  if (!(state.slides || []).length) {
    setExportStatus(t('exportDeckEmpty'));
    return false;
  }
  return true;
}

function getExportLabels(format) {
  const labels = {
    html: {
      working: t('exportHtmlWorking'),
      done: t('exportHtmlDone'),
      failed: t('exportHtmlFailed'),
    },
    pptx: {
      working: t('exportPptxWorking'),
      done: t('exportPptxDone'),
      failed: t('exportPptxFailed'),
    },
    pdf: {
      working: t('exportPdfWorking'),
      done: t('exportPdfDone'),
      failed: t('exportPdfFailed'),
    },
    png: {
      working: t('exportPngWorking'),
      done: t('exportPngDone'),
      failed: t('exportPngFailed'),
    },
  };
  return labels[format] || null;
}

function setExportRenderProgress(index, total, format) {
  const labels = getExportLabels(format === 'pptx' ? 'pptx' : format);
  if (!labels || total <= 0) return;
  const page = Math.min(total, Math.max(1, index + 1));
  setExportModalFeedback('loading', `${labels.working} (${page}/${total})`);
}

async function renderSlidesInHostWebView(slides, format) {
  const deck = runtime();
  if (!deck?.deck?.renderPage) {
    throw new Error('Host WebView export is unavailable in this runtime.');
  }
  const pages = [];
  const total = slides.length;
  for (const [index, slide] of slides.entries()) {
    setExportRenderProgress(index, total, format);
    const base64 = await deck.deck.renderPage({
      html: slideExportHtml(slide),
      format,
      width: EXPORT_VIEWPORT.width,
      height: EXPORT_VIEWPORT.height,
    });
    if (!base64) throw new Error(`Host WebView returned empty ${format} for slide ${index + 1}`);
    pages.push({ index, base64: String(base64).replace(/^data:.*;base64,/, '') });
  }
  return pages;
}

async function executeExport(format) {
  if (format === 'html') {
    updateBriefFromInputs({ includeTopic: !hasUsableDeckForRevision() });
    const filename = downloadHtmlDeck(state);
    if (!filename) throw new Error(t('exportDeckEmpty'));
    return { filename };
  }
  const slides = state.slides || [];
  if (!slides.length) throw new Error(t('exportDeckEmpty'));

  let result;
  const deckPayload = clone(state);
  if (format === 'pptx') {
    if (slides.some((slide) => slide?.html)) {
      const hostDeck = runtime();
      const renderRaster = typeof hostDeck?.deck?.renderPage === 'function'
        ? async (html, index) => {
            setExportRenderProgress(index, slides.length, 'pptx');
            const base64 = await hostDeck.deck.renderPage({
              html,
              format: 'png',
              width: EXPORT_VIEWPORT.width,
              height: EXPORT_VIEWPORT.height,
            });
            return String(base64 || '').replace(/^data:.*;base64,/, '');
          }
        : null;
      const preparedSlides = await prepareSlidesForPptxExport(slides, {
        renderRaster,
        onRasterProgress: (index) => setExportRenderProgress(index, slides.length, 'pptx'),
      });
      result = await exportPptxPrepared(deckPayload, preparedSlides);
    } else {
      result = await exportPptxFromDeck(deckPayload);
    }
  } else if (format === 'pdf') {
    const pages = await renderSlidesInHostWebView(slides, 'pdf');
    result = await exportPdfFromBase64Pages(deckPayload, pages.map((page) => page.base64));
  } else if (format === 'png') {
    const pages = await renderSlidesInHostWebView(slides, 'png');
    result = await exportPngZipFromPages(deckPayload, pages);
  } else {
    throw new Error(t('exportFormatUnavailable'));
  }

  const base64 = typeof result?.base64 === 'string'
    ? result.base64.replace(/^data:.*;base64,/, '')
    : '';
  if (!base64) throw new Error(`export${format} returned no data`);
  const filename = result.filename || `${fileSafe(state.title || 'ppt-live')}`;
  downloadBase64File(
    base64,
    filename,
    result.mimeType || 'application/octet-stream',
  );
  return { filename };
}

let exportInFlight = false;

const handlers = {
  updateOutline(index, value) {
    state.outline[index] = value;
    if (state.slides[index]) state.slides[index].title = value;
    rerender();
    void persist(true);
  },
  moveOutline(index, delta) {
    const next = index + delta;
    if (next < 0 || next >= state.outline.length) return;
    [state.outline[index], state.outline[next]] = [state.outline[next], state.outline[index]];
    syncSlidesFromOutline();
  },
  removeOutline(index) {
    if (state.outline.length <= 1) return;
    state.outline.splice(index, 1);
    syncSlidesFromOutline();
  },
  selectSlide(id) {
    state.activeSlideId = id;
    state.selectedElementId = getActiveSlide(state)?.elements[0]?.id || '';
    rerender();
    void persist(true);
  },
  selectElement(id) {
    state.selectedElementId = id;
    renderSlideCanvas(state, handlers);
    renderInspector(state, handlers);
    void persist(true);
  },
  updateElementTextDirect(id, value) {
    const slide = getActiveSlide(state);
    const element = slide?.elements.find((item) => item.id === id);
    if (!element) return;
    element.text = String(value || '').trim();
    updateSlideTitleFromElements(slide);
    renderThumbs(state, handlers);
    renderOutline(state, handlers);
    void persist(false);
  },
  updateElementListItemDirect(id, index, value) {
    const slide = getActiveSlide(state);
    const element = slide?.elements.find((item) => item.id === id);
    if (!element || !Array.isArray(element.items)) return;
    element.items[index] = String(value || '').trim();
    element.items = element.items.filter(Boolean);
    renderSlideCanvas(state, handlers);
    renderThumbs(state, handlers);
    void persist(false);
  },
  updateSlideHtmlDirect(id, html) {
    const slide = state.slides.find((item) => item.id === id);
    if (!slide) return;
    const next = String(html || '');
    if (slide.html === next) return;
    slide.html = next;
    renderThumbs(state, handlers);
    void persist(false);
  },
  updateSlideNotes(value) {
    const slide = getActiveSlide(state);
    if (slide) slide.notes = value;
    void persist(true);
  },
  updateSlideMethodology() {
    const slide = getActiveSlide(state);
    if (!slide) return;
    slide.kicker = $('slideKickerInput')?.value || slide.kicker;
    slide.claim = $('slideClaimInput')?.value || slide.claim;
    slide.proofObject = $('slideProofInput')?.value || slide.proofObject;
    slide.supportNote = $('slideSupportInput')?.value || slide.supportNote;
    slide.sourceNote = $('slideSourceInput')?.value || slide.sourceNote;
    renderSlideCanvas(state, handlers);
    renderThumbs(state, handlers);
    void persist(true);
  },
  updateElementFromInspector() {
    const slide = getActiveSlide(state);
    const element = getSelectedElement(state);
    if (!slide || !element) return;
    element.text = $('elementTextInput')?.value || '';
    element.items = ($('elementItemsInput')?.value || '').split('\n').map((item) => item.trim()).filter(Boolean);
    element.data = parseChartData($('elementDataInput')?.value || '');
    element.x = clamp(Number($('elementXInput')?.value ?? element.x), 0, 100);
    element.y = clamp(Number($('elementYInput')?.value ?? element.y), 0, 100);
    element.w = clamp(Number($('elementWInput')?.value ?? element.w), 3, 100);
    element.h = clamp(Number($('elementHInput')?.value ?? element.h), 3, 100);
    element.style.fontSize = clamp(Number($('elementFontInput')?.value ?? element.style.fontSize), 8, 88);
    element.style.fontWeight = clamp(Number($('elementWeightInput')?.value ?? element.style.fontWeight), 100, 900);
    element.style.color = $('elementColorInput')?.value || element.style.color;
    element.style.background = $('elementBgInput')?.value || element.style.background;
    handlers.updateSlideMethodology();
    slide.notes = $('slideNotesInput')?.value || slide.notes;
    updateSlideTitleFromElements(slide);
    renderSlideCanvas(state, handlers);
    void persist(true);
  },
  beginDrag(event, elementId) {
    if (event.button !== 0) return;
    const slide = getActiveSlide(state);
    const element = slide?.elements.find((item) => item.id === elementId);
    if (!element) return;
    state.selectedElementId = element.id;
    const rect = $('slideCanvas').getBoundingClientRect();
    dragState = {
      resizing: event.target.classList.contains('resize-handle'),
      startX: event.clientX,
      startY: event.clientY,
      rect,
      start: { x: element.x, y: element.y, w: element.w, h: element.h },
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    window.addEventListener('pointermove', dragMove);
    window.addEventListener('pointerup', endDrag, { once: true });
  },
};

function dragMove(event) {
  if (!dragState) return;
  const element = getSelectedElement(state);
  if (!element) return;
  const dx = ((event.clientX - dragState.startX) / dragState.rect.width) * 100;
  const dy = ((event.clientY - dragState.startY) / dragState.rect.height) * 100;
  if (dragState.resizing) {
    element.w = clamp(dragState.start.w + dx, 3, 100 - element.x);
    element.h = clamp(dragState.start.h + dy, 3, 100 - element.y);
  } else {
    element.x = clamp(dragState.start.x + dx, 0, 100 - element.w);
    element.y = clamp(dragState.start.y + dy, 0, 100 - element.h);
  }
  renderSlideCanvas(state, handlers);
  renderInspector(state, handlers);
}

function endDrag() {
  dragState = null;
  window.removeEventListener('pointermove', dragMove);
  void persist(true);
}

function parseChartData(raw) {
  return raw
    .split('\n')
    .map((line, index) => {
      const [label, value] = line.split(':');
      return { label: (label || `Item ${index + 1}`).trim(), value: Number(value || 0) };
    })
    .filter((point) => point.label);
}

function bindPanelResizers() {
  const shell = document.querySelector('.studio-shell');
  if (!shell) return;
  const root = document.documentElement;
  const storedFilmstrip = Number(safeLocalStorageGet('pptLiveFilmstripWidth') || 0);
  const storedAgent = Number(safeLocalStorageGet('pptLiveAgentWidth') || 0);
  if (storedFilmstrip >= 128 && storedFilmstrip <= 360) {
    root.style.setProperty('--filmstrip-width', `${storedFilmstrip}px`);
  }
  if (storedAgent >= 240 && storedAgent <= 460) {
    root.style.setProperty('--agent-width', `${storedAgent}px`);
  }

  const dragPanel = (side, startX) => {
    const rect = shell.getBoundingClientRect();
    const minFilmstrip = 128;
    const maxFilmstrip = Math.min(360, rect.width * 0.34);
    const minAgent = 240;
    const maxAgent = Math.min(460, rect.width * 0.42);
    const minStage = 360;
    const onMove = (event) => {
      if (side === 'filmstrip') {
        const next = Math.max(minFilmstrip, Math.min(maxFilmstrip, event.clientX - rect.left));
        if (rect.width - next - parseFloat(getComputedStyle(root).getPropertyValue('--agent-width')) - 12 < minStage) return;
        root.style.setProperty('--filmstrip-width', `${next}px`);
      } else {
        const next = Math.max(minAgent, Math.min(maxAgent, rect.right - event.clientX));
        if (rect.width - next - parseFloat(getComputedStyle(root).getPropertyValue('--filmstrip-width')) - 12 < minStage) return;
        root.style.setProperty('--agent-width', `${next}px`);
      }
    };
    const onUp = () => {
      shell.classList.remove('is-resizing');
      document.querySelectorAll('.panel-resizer.is-dragging').forEach((node) => node.classList.remove('is-dragging'));
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      safeLocalStorageSet('pptLiveFilmstripWidth', String(parseFloat(getComputedStyle(root).getPropertyValue('--filmstrip-width')) || ''));
      safeLocalStorageSet('pptLiveAgentWidth', String(parseFloat(getComputedStyle(root).getPropertyValue('--agent-width')) || ''));
      ensureCanvasFitted();
    };
    shell.classList.add('is-resizing');
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp, { once: true });
    window.addEventListener('pointercancel', onUp, { once: true });
    onMove({ clientX: startX });
  };

  $('filmstripResizer')?.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.classList.add('is-dragging');
    dragPanel('filmstrip', event.clientX);
  });
  $('agentResizer')?.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.classList.add('is-dragging');
    dragPanel('agent', event.clientX);
  });
}

function bindEvents() {
  let resizeTimer = null;
  const scheduleCanvasFit = () => {
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      ensureCanvasFitted();
    }, 60);
  };
  window.addEventListener('resize', scheduleCanvasFit);

  $('toggleHistory')?.addEventListener('click', () => {
    const drawer = $('historyDrawer');
    if (!drawer) return;
    drawer.hidden = !drawer.hidden;
  });
  $('closeHistory')?.addEventListener('click', () => {
    const drawer = $('historyDrawer');
    if (drawer) drawer.hidden = true;
  });
  document.querySelectorAll('[data-sidebar-tab]').forEach((button) => {
    button.addEventListener('click', () => {
      const tab = button.dataset.sidebarTab;
      document.querySelectorAll('[data-sidebar-tab]').forEach((node) => {
        node.classList.toggle('is-active', node.dataset.sidebarTab === tab);
      });
      document.querySelectorAll('[data-sidebar-panel]').forEach((node) => {
        node.classList.toggle('is-active', node.dataset.sidebarPanel === tab);
      });
    });
  });

  $('topicInput')?.addEventListener('input', () => {
    const reviseExistingDeck = hasUsableDeckForRevision();
    if (reviseExistingDeck) {
      state.promptDraft = $('topicInput')?.value || '';
      void persist(true);
      return;
    }
    updateBriefFromInputs({ includeTopic: true });
    void persist(true);
  });
  $('newDeck')?.addEventListener('click', () => void newDeck());
  $('cancelGeneration')?.addEventListener('click', () => void stopBackendRun(false));
  $('sendPrompt')?.addEventListener('click', () => void handlePromptSubmit());
  $('generateOutline')?.addEventListener('click', () => void generateOutline());
  $('generateDeck')?.addEventListener('click', () => void generateDeckFromPrompt());
  $('addOutlineItem')?.addEventListener('click', () => {
    state.outline.push(t('newSlideTitle'));
    rerender();
    void persist(true);
  });
  $('syncSlidesFromOutline')?.addEventListener('click', syncSlidesFromOutline);
  $('deleteElement')?.addEventListener('click', deleteElement);
  $('previewDeck')?.addEventListener('click', openPreview);
  $('closePreview')?.addEventListener('click', () => $('previewDialog')?.close());
  $('prevPresent')?.addEventListener('click', () => movePresent(-1));
  $('nextPresent')?.addEventListener('click', () => movePresent(1));
  $('exportHtml')?.addEventListener('click', exportHtml);
  $('restyleDeck')?.addEventListener('click', restyleDeck);
  document.querySelectorAll('[data-add-element]').forEach((button) => {
    button.addEventListener('click', () => addElement(button.dataset.addElement));
  });
  document.querySelectorAll('.ai-action').forEach((button) => {
    button.addEventListener('click', () => void applyAiAction(button.dataset.action));
  });
  document.querySelectorAll('.segment').forEach((button) => {
    button.addEventListener('click', () => {
      state.mode = button.dataset.mode;
      if (state.mode === 'present') openPreview();
      rerender();
      void persist(true);
    });
  });
  document.addEventListener('keydown', (event) => {
    if (!$('previewDialog')?.open) return;
    if (event.key === 'ArrowRight' || event.key === 'PageDown') movePresent(1);
    if (event.key === 'ArrowLeft' || event.key === 'PageUp') movePresent(-1);
    if (event.key === 'Escape') $('previewDialog')?.close();
  });

  try {
    bindPanelResizers();
  } catch (error) {
    runtime().log?.warn?.('Failed to bind PPT Live panel resizers', { error: String(error) });
  }
  if (typeof ResizeObserver !== 'undefined') {
    const fitTargets = [
      document.querySelector('.ppt-live'),
      document.querySelector('.studio-shell'),
      document.querySelector('.stage-shell'),
      document.querySelector('.canvas-area'),
    ].filter(Boolean);
    const layoutObserver = new ResizeObserver(scheduleCanvasFit);
    fitTargets.forEach((node) => layoutObserver.observe(node));
  }

  /* === New v2 UI interactions === */
  bindCanvasZoom();
  bindFloatingToolbar();
  bindPropertyPanels();
  bindExportModal();
  bindHostTheme();
}

/* ============================================
   CANVAS ZOOM
   ============================================ */
let currentZoom = 1;
const ZOOM_STEP = 0.25;
const ZOOM_MIN = 0.25;
const ZOOM_MAX = 2.0;

function setCanvasZoom(zoom) {
  currentZoom = clamp(zoom, ZOOM_MIN, ZOOM_MAX);
  const stage = document.querySelector('.canvas-stage');
  if (stage) stage.style.transform = currentZoom === 1 ? '' : `scale(${currentZoom})`;
  const zoomValue = $('zoomValue');
  const statusZoomValue = $('statusZoomValue');
  const pct = Math.round(currentZoom * 100) + '%';
  if (zoomValue) zoomValue.textContent = pct;
  if (statusZoomValue) statusZoomValue.textContent = pct;
}

function bindCanvasZoom() {
  $('zoomIn')?.addEventListener('click', () => setCanvasZoom(currentZoom + ZOOM_STEP));
  $('zoomOut')?.addEventListener('click', () => setCanvasZoom(currentZoom - ZOOM_STEP));
  $('statusZoomIn')?.addEventListener('click', () => setCanvasZoom(currentZoom + ZOOM_STEP));
  $('statusZoomOut')?.addEventListener('click', () => setCanvasZoom(currentZoom - ZOOM_STEP));
  document.querySelector('.canvas-area')?.addEventListener('wheel', (e) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
      setCanvasZoom(currentZoom + delta);
    }
  }, { passive: false });
}

/* ============================================
   FLOATING TOOLBAR
   ============================================ */
function bindFloatingToolbar() {
  const toolbar = $('floatingToolbar');
  if (!toolbar) return;
  document.querySelectorAll('.floating-toolbar-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const tool = btn.dataset.tool;
      if (!tool) return;
      const slide = getActiveSlide(state);
      const element = getSelectedElement(state);
      if (!slide || !element) return;
      switch (tool) {
        case 'bold':
          element.fontWeight = element.fontWeight === '700' ? '400' : '700';
          break;
        case 'italic':
          element.fontStyle = element.fontStyle === 'italic' ? 'normal' : 'italic';
          break;
        case 'underline':
          element.textDecoration = element.textDecoration === 'underline' ? 'none' : 'underline';
          break;
        case 'align-left': element.align = 'left'; break;
        case 'align-center': element.align = 'center'; break;
        case 'align-right': element.align = 'right'; break;
        case 'duplicate':
          slide.elements.push({ ...clone(element), id: uid('el'), x: element.x + 5, y: element.y + 5 });
          break;
        case 'delete':
          slide.elements = slide.elements.filter((el) => el.id !== element.id);
          state.selectedElementId = null;
          break;
      }
      renderSlideCanvas(state, handlers);
      renderThumbs(state, handlers);
      void persist(true);
    });
  });
}

/* ============================================
   COLLAPSIBLE PROPERTY PANELS
   ============================================ */
function bindPropertyPanels() {
  document.querySelectorAll('.property-section__header').forEach((header) => {
    const section = header.closest('.property-section');
    if (!section) return;
    const toggle = () => {
      section.classList.toggle('is-collapsed');
      const expanded = !section.classList.contains('is-collapsed');
      header.setAttribute('aria-expanded', String(expanded));
    };
    header.addEventListener('click', toggle);
    header.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggle(); }
    });
  });

  /* Density slider (3 snap points) */
  const densitySlider = $('densitySlider');
  const densityTrack = densitySlider?.querySelector('.density-slider__track');
  if (densitySlider && densityTrack) {
    densityTrack.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      setDensitySliderUi(pickDensityIndexFromClientX(event.clientX, densityTrack));
      densityTrack.setPointerCapture(event.pointerId);
    });
    densityTrack.addEventListener('pointermove', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      setDensitySliderUi(pickDensityIndexFromClientX(event.clientX, densityTrack));
    });
    densityTrack.addEventListener('pointerup', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      densityTrack.releasePointerCapture(event.pointerId);
    });
    densityTrack.addEventListener('pointercancel', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      densityTrack.releasePointerCapture(event.pointerId);
      syncDensitySlider(state.style?.density);
    });
    densitySlider.querySelectorAll('[data-density-index]').forEach((tick) => {
      tick.addEventListener('click', (event) => {
        event.stopPropagation();
        setDensitySliderUi(tick.dataset.densityIndex);
      });
    });
    densitySlider.addEventListener('keydown', (event) => {
      const densitySliderRoot = $('densitySlider');
      const currentIndex = Number(densitySliderRoot?.dataset.index ?? 1);
      if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
        event.preventDefault();
        setDensitySliderUi(currentIndex - 1);
      } else if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
        event.preventDefault();
        setDensitySliderUi(currentIndex + 1);
      } else if (event.key === 'Home') {
        event.preventDefault();
        setDensitySliderUi(0);
      } else if (event.key === 'End') {
        event.preventDefault();
        setDensitySliderUi(2);
      }
    });
  }

  /* Font family */
  document.querySelectorAll('[data-font-family]').forEach((button) => {
    button.addEventListener('click', () => {
      syncFontFamilyToggle(button.dataset.fontFamily === 'serif' ? 'serif' : 'sans');
    });
  });

  /* Slide color mode */
  document.querySelectorAll('[data-color-mode]').forEach((button) => {
    button.addEventListener('click', () => {
      syncColorModeToggle(button.dataset.colorMode === 'dark' ? 'dark' : 'light');
    });
  });

  /* Style preset */
  const stylePresetSelect = $('stylePresetSelect');
  if (stylePresetSelect) {
    renderStylePresetOptions();
    enhanceFlatSelect(stylePresetSelect);
    syncStylePanelFromState(state);
    stylePresetSelect.addEventListener('change', () => {
      const selected = stylePresetSelect.value;
      if (!selected) return;
      const preset = getStylePreset(selected);
      if (preset) {
        syncColorModeToggle(preset.colorMode || 'light');
        syncFontFamilyToggle(preset.fontFamily || 'sans');
        setDensitySliderUi(densityToIndex(preset.density || 'standard'));
      }
      refreshFlatSelect(stylePresetSelect);
    });
  }
}

/* ============================================
   EXPORT MODAL
   ============================================ */
let exportPreviewIndex = 0;

function getSelectedExportFormat() {
  return $('formatGrid')?.querySelector('.format-card.is-selected')?.dataset.format || 'pptx';
}

function openExportModal() {
  const overlay = $('exportOverlay');
  if (!overlay) return;
  resetExportModalFeedback();
  exportPreviewIndex = Math.max(0, getActiveIndex(state));
  overlay.classList.add('is-visible');
  overlay.setAttribute('aria-hidden', 'false');
  renderExportFormats();
  updateExportPreview();
  requestAnimationFrame(() => fitExportPreview());
}

function fitExportPreview() {
  fitExportPreviewFrame($('exportPreviewFrame'));
}

function resetExportModalFeedback() {
  const feedback = $('exportModalFeedback');
  const text = $('exportModalFeedbackText');
  const spinner = $('exportModalSpinner');
  $('exportOverlay')?.classList.remove('is-exporting');
  if (feedback) {
    feedback.hidden = true;
    feedback.classList.remove('is-success', 'is-error');
  }
  if (text) text.textContent = '';
  if (spinner) spinner.hidden = false;
  setExportModalBusy(false);
}

function setExportModalBusy(nextBusy) {
  ['exportCancel', 'exportConfirm', 'closeExport'].forEach((id) => {
    const node = $(id);
    if (node) node.disabled = nextBusy;
  });
  $('formatGrid')?.querySelectorAll('.format-card').forEach((card) => {
    card.tabIndex = nextBusy ? -1 : 0;
    card.style.pointerEvents = nextBusy ? 'none' : '';
  });
  ['exportPreviewPrev', 'exportPreviewNext'].forEach((id) => {
    const node = $(id);
    if (node) node.disabled = nextBusy;
  });
}

function setExportModalFeedback(mode, message) {
  const feedback = $('exportModalFeedback');
  const text = $('exportModalFeedbackText');
  const spinner = $('exportModalSpinner');
  if (!feedback || !text) return;
  feedback.hidden = false;
  feedback.classList.toggle('is-success', mode === 'success');
  feedback.classList.toggle('is-error', mode === 'error');
  if (spinner) spinner.hidden = mode !== 'loading';
  text.textContent = message;
}

function closeExportModal() {
  const overlay = $('exportOverlay');
  if (!overlay) return;
  overlay.classList.remove('is-visible');
  overlay.setAttribute('aria-hidden', 'true');
  resetExportModalFeedback();
}

function renderExportFormats() {
  const grid = $('formatGrid');
  if (!grid) return;
  const formats = [
    { id: 'pptx', name: 'PPTX', desc: 'Editable PowerPoint' },
    { id: 'pdf', name: 'PDF', desc: 'Universal format' },
    { id: 'html', name: 'HTML', desc: 'Interactive web deck' },
    { id: 'png', name: 'PNG', desc: 'Image sequence' },
  ];
  grid.innerHTML = formats.map((f, i) => `
    <div class="format-card ${i === 0 ? 'is-selected' : ''}" data-format="${f.id}"
      role="button" tabindex="0" aria-label="Export as ${f.name}"
    >
      <div class="format-card__icon" style="background:${exportFormatTone(f.id)}">${exportFormatIcon(f.id)}</div>
      <span class="format-card__name">${f.name}</span>
      <span class="format-card__desc">${f.desc}</span>
    </div>
  `).join('');
  grid.querySelectorAll('.format-card').forEach((card) => {
    const select = () => {
      grid.querySelectorAll('.format-card').forEach((c) => c.classList.remove('is-selected'));
      card.classList.add('is-selected');
      updateExportPreview();
    };
    card.addEventListener('click', select);
    card.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); select(); }
    });
  });
}

function mountExportPreviewSlide(frame, slide) {
  if (!frame || !slide) return;
  frame.innerHTML = '';
  const viewport = document.createElement('div');
  viewport.className = 'export-preview__viewport';
  const scaleWrap = document.createElement('div');
  scaleWrap.className = 'export-preview__scale';
  if (slide.html) {
    scaleWrap.appendChild(buildExportPreviewStage(slide.html));
  } else {
    const stage = document.createElement('div');
    stage.className = 'export-preview__element-stage';
    stage.innerHTML = slideHtml(slide);
    scaleWrap.append(stage);
  }
  viewport.append(scaleWrap);
  frame.append(viewport);
  requestAnimationFrame(() => {
    fitExportPreview();
    requestAnimationFrame(() => fitExportPreview());
  });
}

function updateExportPreview() {
  const info = $('exportPreviewInfo');
  const counter = $('exportPreviewCounter');
  const frame = $('exportPreviewFrame');
  const slides = state.slides || [];
  const format = getSelectedExportFormat().toUpperCase();
  const total = Math.max(1, slides.length);
  exportPreviewIndex = clamp(exportPreviewIndex, 0, Math.max(0, slides.length - 1));
  if (info) info.textContent = `${format} · ${slides.length} slides`;
  if (counter) counter.textContent = `${exportPreviewIndex + 1} / ${total}`;
  if (!frame) return;
  const slide = slides[exportPreviewIndex];
  if (!slide) {
    frame.innerHTML = `<div class="export-preview__empty">${escapeHtml(t('slidesEmptyHint'))}</div>`;
    return;
  }
  mountExportPreviewSlide(frame, slide);
}

async function confirmExportFromModal() {
  if (exportInFlight) return;
  if (!ensureExportableDeck()) return;
  const format = getSelectedExportFormat();
  const labels = getExportLabels(format);
  if (!labels) {
    setExportStatus(t('exportFormatUnavailable'));
    return;
  }

  exportInFlight = true;
  $('exportOverlay')?.classList.add('is-exporting');
  setExportModalBusy(true);
  setExportModalFeedback('loading', labels.working);
  const previewFrame = $('exportPreviewFrame');
  const previewSnapshot = previewFrame?.innerHTML || '';
  try {
    const { filename } = await executeExport(format);
    const savedMessage = t('exportSavedTo', { path: filename });
    $('exportOverlay')?.classList.remove('is-exporting');
    setExportModalFeedback('success', savedMessage);
    setExportStatus(savedMessage);
    await new Promise((resolve) => setTimeout(resolve, 1600));
    closeExportModal();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    runtime().log?.error?.(`PPT Live ${format} export failed`, { error: message });
    $('exportOverlay')?.classList.remove('is-exporting');
    setExportModalFeedback('error', `${labels.failed} ${message}`);
    setExportStatus(`${labels.failed} ${message}`);
  } finally {
    if (previewFrame && previewSnapshot) previewFrame.innerHTML = previewSnapshot;
    setExportModalBusy(false);
    exportInFlight = false;
  }
}

function bindExportModal() {
  $('exportPptx')?.addEventListener('click', () => openExportModal());
  $('closeExport')?.addEventListener('click', closeExportModal);
  $('exportCancel')?.addEventListener('click', closeExportModal);
  $('exportConfirm')?.addEventListener('click', () => { void confirmExportFromModal(); });
  $('exportOverlay')?.addEventListener('click', (e) => {
    if (e.target === $('exportOverlay') && !exportInFlight) closeExportModal();
  });
  $('exportPreviewPrev')?.addEventListener('click', () => {
    exportPreviewIndex = Math.max(0, exportPreviewIndex - 1);
    updateExportPreview();
    requestAnimationFrame(() => fitExportPreview());
  });
  $('exportPreviewNext')?.addEventListener('click', () => {
    const max = (state.slides || []).length - 1;
    exportPreviewIndex = Math.min(max, exportPreviewIndex + 1);
    updateExportPreview();
    requestAnimationFrame(() => fitExportPreview());
  });
  if (typeof ResizeObserver !== 'undefined') {
    const previewFrame = $('exportPreviewFrame');
    if (previewFrame) {
      new ResizeObserver(() => {
        if ($('exportOverlay')?.classList.contains('is-visible')) fitExportPreview();
      }).observe(previewFrame);
    }
  }
}

/* ============================================
   HOST THEME — follow BitFun light/dark
   ============================================ */
const THEME_STORAGE_KEY = 'pptLiveTheme';

function resolveTheme(theme) {
  if (theme === 'dark' || theme === 'light') return theme;
  if (window.matchMedia?.('(prefers-color-scheme: dark)')?.matches) return 'dark';
  return 'light';
}

function getHostTheme() {
  const attrTheme = document.documentElement.getAttribute('data-theme-type')
    || document.documentElement.getAttribute('data-theme');
  if (attrTheme === 'dark' || attrTheme === 'light') return attrTheme;
  const hostTheme = runtime().theme;
  if (hostTheme === 'dark' || hostTheme === 'light') return hostTheme;
  return resolveTheme();
}

function applyTheme(theme) {
  const resolved = resolveTheme(theme);
  const root = document.documentElement;
  root.setAttribute('data-theme', resolved);
  root.setAttribute('data-theme-type', resolved);
  root.style.colorScheme = resolved;
  ensureCanvasFitted();
  rerender();
}

function bindHostTheme() {
  try {
    localStorage.removeItem(THEME_STORAGE_KEY);
  } catch {
    memoryStorage.delete(THEME_STORAGE_KEY);
  }
  applyTheme(getHostTheme());
  runtime().onThemeChange?.((payload) => {
    const next = payload?.type === 'dark' ? 'dark' : 'light';
    applyTheme(next);
  });
}

async function recoverFromRestart() {
  deckEpoch += 1;
  backendRuns = [];
  backendRunInFlight = false;
  promptSubmitGuard = false;
  if (state.generation?.active || state.generation?.steps?.some((step) => step.status === 'running')) {
    finishGenerationUi(t('generationStopped'));
    resetGeneration();
  }
  setBusy(false);
  const host = runtime();
  if (host.backend?.cancelStaleRuns) {
    void host.backend.cancelStaleRuns().catch((error) => {
      runtime().log?.warn?.('Failed to cancel stale PPT Live backend runs', { error: String(error) });
    });
  }
}

function renderStylePresetOptions() {
  const stylePresetSelect = $('stylePresetSelect');
  if (!stylePresetSelect) return;
  const selected = stylePresetSelect.value || state.style?.stylePreset || DEFAULT_STYLE_PRESET;
  stylePresetSelect.textContent = '';
  getAllStylePresets(getLocale()).forEach(({ key, displayName, description }) => {
    const option = document.createElement('option');
    option.value = key;
    option.textContent = displayName;
    if (description) option.title = description;
    stylePresetSelect.append(option);
  });
  stylePresetSelect.value = selected;
  if (stylePresetSelect.selectedIndex < 0) stylePresetSelect.value = DEFAULT_STYLE_PRESET;
  refreshFlatSelect(stylePresetSelect);
}

function syncLocale() {
  state.generation = normalizeGeneration(state.generation);
  applyI18n();
  renderStylePresetOptions();
  const pill = $('aiStatusPill');
  if (pill) pill.textContent = busy ? t('statusPillBusy') : t('statusPillReady');
  rerender();
}

async function init() {
  syncLocale();
  try {
    await loadState();
    await recoverFromRestart();
    syncLocale();
    syncStylePanelFromState(state);
    await persist(true);
  } catch (error) {
    runtime().log?.error?.('PPT Live init failed', { error: String(error) });
    setStatus(t('ready'));
    syncLocale();
  } finally {
    ensureCanvasFitted();
  }
}

bindEvents();
observeThumbPreviews();
runtime().onLocaleChange?.(() => syncLocale());
init();
