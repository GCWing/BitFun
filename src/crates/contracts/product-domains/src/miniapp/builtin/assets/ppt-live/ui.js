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
import { applyI18n, readInputs, renderAll, renderInspector, renderSlideCanvas, renderGeneration, renderGenerationOverlay, renderThumbs, slideHtml, fitSlideCanvas, fitHtmlSlideFrame, buildExportPreviewStage, fitExportPreviewFrame, fitThumbPreviews, normalizeSlideDocument, observeThumbPreviews, ensureCanvasFitted, syncDensitySlider } from './src/render.js';
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
  PPT_DESIGN_REQUIRED_REFERENCES,
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
    await runPptLiveBackend('auto', instruction, { includeTopic: !reviseExistingDeck });
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

function livePlanContextStyle(planContext = {}) {
  const style = buildGenerationStyle();
  return {
    ...(planContext.style || {}),
    ...style,
  };
}

function livePlanContextDesign(planContext = {}) {
  const style = buildGenerationStyle();
  return {
    ...(planContext.design || {}),
    theme: style.colorMode === 'dark' ? 'dark' : 'light',
    palette: { ...(planContext.design?.palette || {}), ...style.palette },
  };
}

function liveGenerationContract(planContext = {}) {
  const style = buildGenerationStyle();
  return {
    ...(planContext.generationContract || {}),
    userStyle: style,
  };
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

function setDensityIndex(index, { save = true } = {}) {
  const nextIndex = clamp(Math.round(Number(index)), 0, 2);
  state.style.density = indexToDensity(nextIndex);
  syncDensitySlider(state.style.density);
  rerender();
  if (save) void persist(true);
}

// Interrupted turns are retried as "continue" turns inside the same agent
// session. Each stage first gets a short retry budget for transient failures;
// only after that budget is exhausted do we send explicit completion-aware
// continuation prompts carrying host-verified missing artifacts.
const PPT_BACKEND_MAX_ATTEMPTS = 4;
const PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS = 8;
const PPT_BACKEND_TOTAL_STAGE_ATTEMPTS =
  PPT_BACKEND_MAX_ATTEMPTS + PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS;
const PPT_RETRY_DELAY_MS = 750;

function stageAttemptInfo(attempt) {
  const continuation = attempt > PPT_BACKEND_MAX_ATTEMPTS;
  return {
    continuation,
    continuationAttempt: continuation ? attempt - PPT_BACKEND_MAX_ATTEMPTS : 0,
  };
}

function completionRecoveryInput(stage, attempt, error, issues = []) {
  const info = stageAttemptInfo(attempt);
  if (!info.continuation) return null;
  return {
    stage,
    attempt: info.continuationAttempt,
    maxAttempts: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
    previousFailure: backendErrorDetail(error),
    issues: [...new Set((issues || []).map(String).filter(Boolean))],
  };
}

function recoveryExhaustedError(stageKey, lastError, failures = [], stageVars = {}) {
  const reasons = [...new Set(
    failures
      .map((failure) => backendErrorDetail(failure))
      .filter(Boolean),
  )];
  const lastReason = backendErrorDetail(lastError);
  const summary = lastReason || reasons.at(-1) || 'The Agent did not produce the required artifact.';
  const error = new Error(
    `PPT Live recovery exhausted after ${PPT_BACKEND_MAX_ATTEMPTS} stage attempts and `
      + `${PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS} continuation turns. Last verified reason: ${summary}`,
    { cause: lastError || undefined },
  );
  error.pptLiveRecoveryExhausted = {
    stageKey,
    stageVars,
    stepAttempts: PPT_BACKEND_MAX_ATTEMPTS,
    continuationAttempts: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
    reasons,
  };
  return error;
}

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

const GENERATION_HARD_RULES = [
  'All visible text must be inside p, h1-h6, or li; DIV cannot contain direct text and span is inline-only.',
  'CSS gradients are forbidden; use solid fills and discrete shapes.',
  'Backgrounds, borders, and shadows belong on DIV shapes, never on text or inline elements.',
  'DIV background-image is forbidden; use img. Inline text elements cannot carry box spacing or decorative fills.',
  'Use a complete self-contained 960pt x 540pt document with no remote assets and no canvas overflow.',
  'Text larger than 12px must keep at least a 36pt bottom safety margin.',
];

function outlineItemTitle(item) {
  return typeof item === 'string' ? item : String(item?.title || '');
}

function planOutlineTitles(plan) {
  return Array.isArray(plan?.outline)
    ? plan.outline.map(outlineItemTitle).filter(Boolean)
    : [];
}

function expectedPlanReferences(style) {
  const references = [...PPT_DESIGN_REQUIRED_REFERENCES];
  if (style?.stylePreset) {
    references.push(`references/style-presets/${style.stylePreset}.md`);
  }
  return references;
}

function finalizePlanContract(payload, instruction) {
  const slidePlans = extractValidSlidePlans(payload).map((plan, index) => ({
    ...plan,
    slideNumber: slidePlanNumber(plan, index + 1),
    slideId: String(plan?.slideId || `slide-${String(index + 1).padStart(2, '0')}`),
  }));
  const style = buildGenerationStyle();
  const brief = buildGenerationBrief();
  const outline = slidePlans.map((plan) => ({
    id: plan.slideId,
    title: String(plan.title || ''),
    bullets: Array.isArray(plan.bullets) ? plan.bullets.map(String) : [],
    slide_id: plan.slideId,
  }));
  const requestedShowcase = Array.isArray(payload?.showcaseSlideNumbers)
    ? payload.showcaseSlideNumbers.map(Number)
    : [];
  const validShowcase = [...new Set(requestedShowcase)]
    .filter((number) => Number.isInteger(number) && number >= 1 && number <= slidePlans.length)
    .slice(0, 2);
  if (slidePlans.length >= 5 && validShowcase.length < 2) {
    for (const fallback of [1, Math.min(3, slidePlans.length), slidePlans.length]) {
      if (!validShowcase.includes(fallback)) validShowcase.push(fallback);
      if (validShowcase.length === 2) break;
    }
  }

  return {
    ...payload,
    outline,
    slide_order: outline.map((item) => item.slide_id),
    style,
    design: {
      ...(payload?.design || {}),
      theme: style.colorMode === 'dark' ? 'dark' : 'light',
      palette: { ...(payload?.design?.palette || {}), ...style.palette },
    },
    assumptions: Array.isArray(payload?.assumptions)
      ? payload.assumptions.map(String)
      : (payload?.researchReport?.assumptions || []).map(String),
    generationContract: {
      ...(payload?.generationContract || {}),
      version: 1,
      skillKey: PPT_DESIGN_SKILL_KEY,
      skillName: 'ppt-design',
      requiredReferences: expectedPlanReferences(style),
      deliveryTarget: 'editable-pptx',
      userPrompt: String(instruction || ''),
      userBrief: brief,
      userStyle: style,
      hardRules: GENERATION_HARD_RULES,
      visualGrammar: {
        ...(payload?.generationContract?.visualGrammar || {}),
        ...(payload?.design?.renderGuide || {}),
      },
    },
    showcaseSlideNumbers: slidePlans.length >= 5 ? validShowcase : [],
    slidePlans,
  };
}

async function writeDeckProjectJson(project, payload) {
  const fs = runtime().fs;
  if (!project || !fs?.writeFile) return;
  await fs.writeFile(`${project.dir}/project.json`, `${JSON.stringify(payload, null, 2)}\n`);
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

function toolTraceParams(entry) {
  return entry?.params && typeof entry.params === 'object' ? entry.params : {};
}

function isPptDesignSkillCommand(command) {
  const normalized = String(command || '').trim();
  return normalized === PPT_DESIGN_SKILL_KEY || normalized === 'ppt-design';
}

function pptDesignSkillLoadSucceeded(completedEntry) {
  if (!completedEntry || String(completedEntry.toolName || '').toLowerCase() !== 'skill') return false;
  const result = completedEntry.result || {};
  const blob = JSON.stringify(result);
  if (blob.includes(PPT_DESIGN_SKILL_KEY)) return true;
  if (result?.success === true && String(result?.skill_name || '').toLowerCase() === 'ppt-design') return true;
  return false;
}

function planningEvidenceIssues(toolTrace, style) {
  const started = (toolTrace || []).filter((entry) => entry.eventType === 'Started');
  const completed = (toolTrace || []).filter((entry) => entry.eventType === 'Completed');
  const completedById = new Map(
    completed
      .filter((entry) => entry.toolId)
      .map((entry) => [entry.toolId, entry]),
  );
  const issues = [];
  const pptDesignSkillStarts = started.filter((entry) => (
    String(entry.toolName || '').toLowerCase() === 'skill'
    && isPptDesignSkillCommand(toolTraceParams(entry).command)
  ));
  const skillCompleted = pptDesignSkillStarts.some((entry) => (
    pptDesignSkillLoadSucceeded(completedById.get(entry.toolId))
  ));
  if (!pptDesignSkillStarts.length || !skillCompleted) {
    issues.push(`The exact built-in Skill key ${PPT_DESIGN_SKILL_KEY} was not successfully loaded.`);
  }

  const readPaths = started
    .filter((entry) => (
      String(entry.toolName || '').toLowerCase() === 'read'
      && completedById.has(entry.toolId)
    ))
    .map((entry) => {
      const params = toolTraceParams(entry);
      return String(params.file_path || params.path || '');
    });
  expectedPlanReferences(style).forEach((reference) => {
    if (!readPaths.some((path) => path.endsWith(reference))) {
      issues.push(`Required reference was not read: ${reference}`);
    }
  });
  return issues;
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
    const isInitialAutoDraft = operation === 'auto' && (isDefaultDraft() || isStarterDeck());
    if (isInitialAutoDraft) {
      // Fresh deck generation plans once, then renders one slide per Agent
      // turn in strict order so turns do not compete for model/tool capacity.
      await runStagedDeckGeneration(operation, instruction);
      return;
    }
    await runLegacyBackendWithRetries(operation, instruction);
  } finally {
    backendRunInFlight = false;
  }
}

async function runLegacyBackendWithRetries(operation, instruction) {
  let lastError = null;
  // After an interrupted attempt, retry as a "continue" turn inside the same
  // hidden session so the model resumes with its prior context intact.
  const project = currentDeckProject() || (backendUsesFileProtocol() ? newDeckProject() : null);
  if (project && !state.agentSession?.workspaceSubdir) {
    await pruneOldDeckProjects(project.runId);
  }
  const retrySession = {
    id: state.agentSession?.id || null,
    project,
  };
  for (let attempt = 1; attempt <= PPT_BACKEND_MAX_ATTEMPTS; attempt += 1) {
    try {
      await runPptLiveBackendAttempt(operation, instruction, attempt, retrySession);
      return;
    } catch (error) {
      lastError = error;
      if (isUnknownSessionBackendError(error)) retrySession.id = null;
      else if (error?.pptLiveSessionId) retrySession.id = error.pptLiveSessionId;
      if (!isRetryableBackendError(error) || attempt >= PPT_BACKEND_MAX_ATTEMPTS) throw error;
      runtime().log?.warn?.('PPT Live backend attempt failed, retrying', {
        attempt,
        maxAttempts: PPT_BACKEND_MAX_ATTEMPTS,
        continueInSession: Boolean(retrySession.id),
        error: String(error),
      });
      addGenerationEvent({
        title: t('generationRetrying', { attempt: attempt + 1, max: PPT_BACKEND_MAX_ATTEMPTS }),
        detail: backendErrorDetail(error),
        kind: 'error',
      });
      setStatus(t('generationRetrying', { attempt: attempt + 1, max: PPT_BACKEND_MAX_ATTEMPTS }));
      await new Promise((resolve) => setTimeout(resolve, retryDelayMs(error, attempt)));
    }
  }
  if (lastError) throw lastError;
}

/**
 * Run one `ppt.generate` backend turn and return `{ payload, sessionId }`.
 * Handles event wiring, streaming buffers, idle/absolute timeouts,
 * cancel-on-abandon, and run tracking. UI step transitions are delegated to
 * `hooks` so staged phases and the legacy path can shape progress differently:
 * - `hooks.onTextProgress(buffer)`: called as answer text streams in.
 * - `hooks.onToolPhase(kind)`: called with 'detected' | 'completed' | 'research' | 'round'.
 * `options.sessionId` submits the turn into an existing hidden agent session
 * (the staged pipeline keeps plan + render + continue turns in one session).
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
        if (event.sessionId !== sessionId) return;
        if (event.turnId && event.turnId !== turnId) return;
        activity.lastEventAt = Date.now();
        const sourceEvent = String(event.sourceEvent || '');
        if (sourceEvent.endsWith('dialog-turn-started')) {
          progressTracker.note(t('eventTurnStarted'), '', 'turn');
        } else if (sourceEvent.endsWith('model-round-started')) {
          hooks.onToolPhase?.('round');
          progressTracker.touch();
        } else if (sourceEvent.endsWith('model-round-completed')) {
          progressTracker.touch();
        } else if (sourceEvent.endsWith('tool-event')) {
          const toolEvent = normalizeToolEvent(event.toolEvent || {});
          const eventType = toolEvent.event_type || toolEvent.eventType || '';
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
          if (shouldLogToolEvent(toolEvent, loggedToolEvents)) {
            addGenerationEvent(describeToolEvent(event));
            progressTracker.touch();
          }
          if (eventType === 'EarlyDetected' || eventType === 'Started') {
            hooks.onToolPhase?.('detected');
          } else if (eventType === 'Completed') {
            const toolName = String(toolEvent.tool_name || toolEvent.toolName || '').trim().toLowerCase();
            hooks.onToolPhase?.('completed');
            if (toolName === 'skill') {
              progressTracker.note(t('eventToolSkillReady'), friendlyToolName(toolEvent.tool_name || toolEvent.toolName), 'phase');
            } else if (toolName === 'websearch' || toolName === 'webfetch') {
              hooks.onToolPhase?.('research');
            }
          }
        } else if (sourceEvent.endsWith('text-chunk')) {
          const chunk = String(event.text || '');
          const isThinking = event.contentType === 'thinking';
          if (isThinking) thinkingBuffer += chunk;
          else {
            textBuffer += chunk;
            progressTracker.touch();
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
          settled = true;
          completion = {
            success: event.success,
            finishReason: event.finishReason || event.finish_reason || '',
            partialRecoveryReason:
              event.partialRecoveryReason || event.partial_recovery_reason || '',
          };
          resolve({ answer: textBuffer, thinking: thinkingBuffer });
        } else if (sourceEvent.endsWith('dialog-turn-failed') || sourceEvent.endsWith('dialog-turn-cancelled')) {
          settled = true;
          // Final flush so checkpoint extractors see every slide that finished
          // streaming before the failure; retries resume from those slides.
          if (textBuffer) hooks.onTextProgress?.(textBuffer);
          const eventError = compactText(event.error || event.message || '');
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

function buildSlideRenderRequestBase(operation) {
  return {
    operation,
    locale: getLocale(),
    brief: buildGenerationBrief({ includeEvidence: false }),
    style: buildGenerationStyle({ includePreset: false }),
  };
}

async function runPptLiveBackendAttempt(operation, instruction, attempt = 1, retrySession = { id: null }) {
  const runEpoch = deckEpoch;
  setBusy(true, t('working'));
  resetGeneration();
  setGenerationStep('brief', 'running', t('generationReadingBrief'));
  addGenerationEvent({ title: t('processEventStarted'), detail: t('processEventWaiting'), kind: 'start' });
  if (attempt > 1) {
    addGenerationEvent({
      title: t('generationRetryAttempt', { attempt, max: PPT_BACKEND_MAX_ATTEMPTS }),
      detail: '',
      kind: 'start',
    });
  }
  prepareAgentGenerationSurface(operation, instruction);
  let completed = false;
  const lastStreamPhase = { value: '' };
  const progressShim = { touch: () => {}, note: () => {}, lastProgressLogAt: 0 };

  try {
    const { payload, sessionId } = await executeBackendTurn({
      ...buildBackendRequestBase(operation, instruction),
      title: state.title,
      outline: clone(state.outline),
      currentSlideIndex: getActiveIndex(state),
      currentDeck: buildCurrentDeckSnapshot(instruction),
      ...(retrySession?.id ? { continueAfterInterruption: true } : {}),
    }, {
      onToolPhase: (kind) => {
        if (kind === 'detected') {
          setGenerationStep('brief', 'running', t('generationReadingBrief'));
        } else if (kind === 'completed') {
          setGenerationStep('brief', 'done');
          setGenerationStep('spine', 'running', t('generationWritingClaims'));
        } else if (kind === 'research') {
          setGenerationStep('proof', 'running', t('generationChoosingProof'));
        } else if (kind === 'round') {
          setGenerationStep('spine', 'running', t('generationWritingClaims'));
        }
      },
      onTextProgress: (buffer) => noteTextStreamProgress(buffer, progressShim, lastStreamPhase),
    }, {
      sessionId: retrySession?.id || undefined,
      appDataWorkspace: retrySession?.project?.workspaceSubdir,
    });
    retrySession.id = sessionId || retrySession.id;
    state.agentSession = {
      id: retrySession.id || '',
      workspaceSubdir: retrySession?.project?.workspaceSubdir || '',
      runId: retrySession?.project?.runId || '',
      skillKey: PPT_DESIGN_SKILL_KEY,
    };
    addGenerationEvent({ title: t('generationParsingDeck'), detail: '', kind: 'parsing' });
    setStatus(t('generationParsingDeck'));
    applyDeckPayload(payload);
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

function slidePlanNumber(plan, fallback) {
  const number = Number(plan?.slideNumber);
  return Number.isFinite(number) && number > 0 ? Math.round(number) : fallback;
}

function extractValidSlidePlans(payload) {
  return Array.isArray(payload?.slidePlans)
    ? payload.slidePlans.filter((plan) => plan && typeof plan === 'object')
    : [];
}

/**
 * Plan phase. With the file protocol (`project` set) the agent saves the plan
 * to `project.json` in the deck project directory and ui.js reads it back;
 * the legacy AI fallback still returns the plan as JSON text. Interrupted
 * attempts retry as "continue" turns in the same session, and a plan file
 * that landed on disk before the failure is recovered without another turn.
 */
async function runStagedPlanPhase(operation, instruction, project = null) {
  let lastError = null;
  let planSessionId = null;
  const planningTrace = [];
  const failures = [];
  let complianceIssues = [];
  for (let attempt = 1; attempt <= PPT_BACKEND_TOTAL_STAGE_ATTEMPTS; attempt += 1) {
    try {
      const attemptInfo = stageAttemptInfo(attempt);
      if (attempt > 1) {
        addGenerationEvent({
          title: attemptInfo.continuation
            ? t('generationRecoveryContinuing', {
                stage: t('generationStagePlanning'),
                attempt: attemptInfo.continuationAttempt,
                max: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
              })
            : t('generationPlanRetry', { attempt, max: PPT_BACKEND_MAX_ATTEMPTS }),
          detail: complianceIssues.join(' ') || backendErrorDetail(lastError),
          kind: 'error',
        });
        setStatus(attemptInfo.continuation
          ? t('generationRecoveryContinuing', {
              stage: t('generationStagePlanning'),
              attempt: attemptInfo.continuationAttempt,
              max: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
            })
          : t('generationPlanRetry', { attempt, max: PPT_BACKEND_MAX_ATTEMPTS }));
        await new Promise((resolve) => setTimeout(resolve, retryDelayMs(lastError, attempt)));
      }
      const completionRecovery = completionRecoveryInput(
        'plan',
        attempt,
        lastError,
        complianceIssues,
      );
      const turn = await executeBackendTurn({
        ...buildBackendRequestBase(operation, instruction),
        phase: 'plan',
        title: state.title,
        outline: [],
        ...(complianceIssues.length ? { complianceIssues } : {}),
        ...(attempt > 1 && !completionRecovery && planSessionId
          ? { continueAfterInterruption: true }
          : {}),
        ...(completionRecovery ? { completionRecovery } : {}),
      }, {
        onToolPhase: (kind) => {
          if (kind === 'detected') {
            setGenerationStep('brief', 'running', t('generationReadingBrief'));
          } else if (kind === 'completed' || kind === 'round') {
            setGenerationStep('brief', 'done');
            setGenerationStep('spine', 'running', t('generationWritingClaims'));
          } else if (kind === 'research') {
            setGenerationStep('proof', 'running', t('generationChoosingProof'));
          }
        },
        onTextProgress: (buffer) => {
          if (!buffer.includes('"slidePlans"')) return;
          // Legacy JSON delivery streams the plan for minutes on large decks;
          // surface how many per-slide briefs appeared so the UI never looks
          // frozen. File-protocol turns reply with a status line only.
          const plannedCount = (buffer.match(/"slideNumber"/g) || []).length;
          if (plannedCount > 0) {
            setGenerationStep('proof', 'running', t('generationPlanProgress', { count: plannedCount }));
            setStatus(t('generationPlanProgress', { count: plannedCount }));
          } else {
            setGenerationStep('proof', 'running', t('generationPlanningSlides'));
          }
        },
      }, {
        sessionId: planSessionId || undefined,
        appDataWorkspace: project?.workspaceSubdir,
        resultKind: project ? 'text' : undefined,
      });
      planSessionId = turn.sessionId || planSessionId;
      planningTrace.push(...(turn.toolTrace || []));
      const payload = project ? await tryReadDeckPlanFile(project) : turn.payload;
      if (project && !payload) {
        throw new Error('PPT Live plan turn finished without a valid project.json');
      }
      complianceIssues = planningEvidenceIssues(planningTrace, buildGenerationStyle());
      if (complianceIssues.length) {
        const complianceError = new Error(`PPT Live planning compliance failed: ${complianceIssues.join(' ')}`);
        complianceError.pptLivePlanCompliance = true;
        complianceError.pptLiveSessionId = turn.sessionId || planSessionId || null;
        throw complianceError;
      }
      const finalizedPayload = finalizePlanContract(payload, instruction);
      const slidePlans = extractValidSlidePlans(finalizedPayload);
      if (!slidePlans.length) throw new Error('PPT Live plan phase returned no slidePlans');
      await writeDeckProjectJson(project, finalizedPayload);
      return {
        payload: finalizedPayload,
        slidePlans,
        sessionId: turn.sessionId || planSessionId || null,
        project,
      };
    } catch (error) {
      lastError = error;
      failures.push(error);
      if (Array.isArray(error?.pptLiveToolTrace)) planningTrace.push(...error.pptLiveToolTrace);
      if (isUnknownSessionBackendError(error)) planSessionId = null;
      else if (error?.pptLiveSessionId) planSessionId = error.pptLiveSessionId;
      if (!isRetryableBackendError(error)) throw error;
      if (attempt >= PPT_BACKEND_TOTAL_STAGE_ATTEMPTS) {
        throw recoveryExhaustedError('generationStagePlanning', error, failures);
      }
      runtime().log?.warn?.('PPT Live plan phase failed, retrying', {
        attempt,
        maxAttempts: PPT_BACKEND_TOTAL_STAGE_ATTEMPTS,
        continueInSession: Boolean(planSessionId),
        error: String(error),
      });
    }
  }
  throw lastError;
}

/**
 * Render one slide. The normal path submits a lightweight turn INTO the
 * planning session (`session.id`), where the ppt-design skill, style preset,
 * research, and plan already live.
 * With the file protocol (`project` set) the agent writes
 * `slides/slide-NN.html` in the deck project directory and ui.js reads it
 * back; a file that landed on disk before a failure is recovered without
 * another turn. If the session is lost (backend restart, stale id), the turn
 * falls back to a self-contained prompt that reloads the skill and carries
 * the plan. Interrupted attempts retry as "continue" turns in the session.
 */
async function runStagedSlide({ operation, planContext, slidePlan, slideNumber, session, project }) {
  let lastError = null;
  const failures = [];
  const slideFromHtml = (html, quality = { score: 100, issues: [] }) => ({
    ...slidePlan,
    html,
    quality,
    id: `ppt-live-slide-${slideNumber}`,
    slideNumber,
  });
  for (let attempt = 1; attempt <= PPT_BACKEND_TOTAL_STAGE_ATTEMPTS; attempt += 1) {
    try {
      const attemptInfo = stageAttemptInfo(attempt);
      if (attempt > 1) {
        if (project) {
          // The failed turn may have written the slide file before dying.
          const recovered = await tryReadDeckSlideFile(project, slideNumber);
          if (recovered) {
            return slideFromHtml(recovered);
          }
        }
        addGenerationEvent({
          title: attemptInfo.continuation
            ? t('generationRecoveryContinuing', {
                stage: t('generationStageSlide', { slide: slideNumber }),
                attempt: attemptInfo.continuationAttempt,
                max: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
              })
            : t('generationSlideRetry', {
                slide: slideNumber,
                attempt,
                max: PPT_BACKEND_MAX_ATTEMPTS,
              }),
          detail: backendErrorDetail(lastError),
          kind: 'error',
        });
        setStatus(attemptInfo.continuation
          ? t('generationRecoveryContinuing', {
              stage: t('generationStageSlide', { slide: slideNumber }),
              attempt: attemptInfo.continuationAttempt,
              max: PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS,
            })
          : t('generationSlideRetry', {
              slide: slideNumber,
              attempt,
              max: PPT_BACKEND_MAX_ATTEMPTS,
            }));
        await new Promise((resolve) => setTimeout(resolve, retryDelayMs(lastError, attempt)));
      }
      const completionRecovery = completionRecoveryInput(
        'slides',
        attempt,
        lastError,
        lastError ? [backendErrorDetail(lastError)] : [],
      );
      const inSession = Boolean(session?.id);
      const requestInput = inSession
        ? {
            operation,
            locale: getLocale(),
            phase: 'slides',
            inSession: true,
            assignedSlides: [slidePlan],
            generationContract: liveGenerationContract(planContext),
            design: livePlanContextDesign(planContext),
            style: livePlanContextStyle(planContext),
            showcaseSlideNumbers: planContext.showcaseSlideNumbers,
            ...(attempt > 1 && !completionRecovery ? { continueAfterInterruption: true } : {}),
            ...(completionRecovery ? { completionRecovery } : {}),
          }
        : {
            ...buildSlideRenderRequestBase(operation),
            phase: 'slides',
            plan: {
              ...planContext,
              style: livePlanContextStyle(planContext),
              design: livePlanContextDesign(planContext),
              generationContract: liveGenerationContract(planContext),
            },
            assignedSlides: [slidePlan],
            ...(completionRecovery ? { completionRecovery } : {}),
          };
      const turn = await executeBackendTurn(requestInput, {}, {
        sessionId: session?.id || undefined,
        appDataWorkspace: project?.workspaceSubdir,
        resultKind: project ? 'text' : undefined,
      });
      if (session && turn.sessionId) session.id = turn.sessionId;
      const { payload } = turn;
      if (project) {
        const html = await tryReadDeckSlideFile(project, slideNumber);
        if (!html) {
          throw new Error(`PPT Live slide ${slideNumber} file is missing or incomplete`);
        }
        return slideFromHtml(html);
      }
      const slides = Array.isArray(payload?.slides) ? payload.slides : [];
      const slide = slides.find((candidate) => slidePlanNumber(candidate, NaN) === slideNumber) || slides[0];
      if (!slide || typeof slide !== 'object' || !String(slide.html || '').trim()) {
        throw new Error(`PPT Live slide ${slideNumber} is missing complete HTML`);
      }
      return {
        ...slide,
        id: slide.id || `ppt-live-slide-${slideNumber}`,
        slideNumber,
      };
    } catch (error) {
      lastError = error;
      failures.push(error);
      if (session && isUnknownSessionBackendError(error)) {
        // Session is gone; render the rest of the deck with self-contained turns.
        session.id = null;
        runtime().log?.warn?.('PPT Live planning session lost, falling back to standalone render turns', {
          slide: slideNumber,
        });
      }
      if (!isRetryableBackendError(error)) throw error;
      if (attempt >= PPT_BACKEND_TOTAL_STAGE_ATTEMPTS) {
        throw recoveryExhaustedError(
          'generationStageSlide',
          error,
          failures,
          { slide: slideNumber },
        );
      }
      runtime().log?.warn?.('PPT Live slide render failed, retrying page', {
        slide: slideNumber,
        attempt,
        maxAttempts: PPT_BACKEND_TOTAL_STAGE_ATTEMPTS,
        continueInSession: Boolean(session?.id),
        error: String(error),
      });
    }
  }
  throw lastError;
}


async function runStagedDeckGeneration(operation, instruction) {
  const runEpoch = deckEpoch;
  setBusy(true, t('working'));
  resetGeneration();
  setGenerationStep('brief', 'running', t('generationReadingBrief'));
  addGenerationEvent({ title: t('processEventStarted'), detail: t('processEventWaiting'), kind: 'start' });
  addGenerationEvent({ title: t('generationPlanPhase'), detail: '', kind: 'phase' });
  prepareAgentGenerationSurface(operation, instruction);
  let completed = false;

  // File protocol (agent backend): the whole run works inside a dedicated
  // deck project directory in this app's appdata storage, following the
  // ppt-design skill's native `project.json` + `slides/slide-NN.html` layout.
  const project = backendUsesFileProtocol() ? newDeckProject() : null;
  if (project) {
    await pruneOldDeckProjects(project.runId);
  }

  try {
    // Phase 1: plan (research + outline + design + per-slide briefs).
    const { payload: planPayload, slidePlans, sessionId: planSessionId } = await runStagedPlanPhase(operation, instruction, project);
    if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');
    setGenerationStep('brief', 'done');
    setGenerationStep('spine', 'done');
    setGenerationStep('proof', 'done');
    setGenerationStep('design', 'running', t('generationDesigningLayouts'));
    state.generation.slideTarget = slidePlans.length;
    state.generation.draftedCount = 0;
    addGenerationEvent({
      title: t('generationPlanReady', { count: slidePlans.length }),
      detail: compactText(planOutlineTitles(planPayload).join(' / '), 200),
      kind: 'phase',
    });
    if (planPayload.title) {
      state.title = String(planPayload.title);
      rerender();
    }

    // Phase 2: render exactly one slide per Agent turn, in deck order. Render
    // turns run inside the planning session so the skill/preset/research load
    // once for the whole deck; `planContext` is only sent on the standalone
    // fallback path when that session is lost.
    const session = { id: planSessionId || null };
    state.agentSession = {
      id: session.id || '',
      workspaceSubdir: project?.workspaceSubdir || '',
      runId: project?.runId || '',
      skillKey: PPT_DESIGN_SKILL_KEY,
    };
    const planContext = {
      title: planPayload.title || '',
      language: planPayload.language || '',
      outline: planPayload.outline || [],
      researchReport: planPayload.researchReport || '',
      design: planPayload.design || {},
      style: planPayload.style || buildGenerationStyle(),
      generationContract: planPayload.generationContract || {},
      showcaseSlideNumbers: planPayload.showcaseSlideNumbers || [],
      slideOrder: planPayload.slide_order || [],
    };
    const normalizedPlans = slidePlans.map((plan, index) => ({
      ...plan,
      slideNumber: slidePlanNumber(plan, index + 1),
    }));
    const showcaseNumbers = new Set(planContext.showcaseSlideNumbers);
    const renderPlans = [
      ...normalizedPlans.filter((plan) => showcaseNumbers.has(plan.slideNumber)),
      ...normalizedPlans.filter((plan) => !showcaseNumbers.has(plan.slideNumber)),
    ];
    addGenerationEvent({
      title: t('generationSlidesPhase', {
        count: normalizedPlans.length,
      }),
      detail: '',
      kind: 'phase',
    });
    const readySlides = [];
    for (const slidePlan of renderPlans) {
      if (isDeckEpochStale(runEpoch)) throw new Error('Generation stopped');
      const slideNumber = slidePlan.slideNumber;
      setGenerationStep('design', 'running', t('generationRenderingSlide', {
        slide: slideNumber,
        total: normalizedPlans.length,
      }));
      setStatus(t('generationRenderingSlide', {
        slide: slideNumber,
        total: normalizedPlans.length,
      }));
      try {
        const slide = await runStagedSlide({
          operation,
          planContext,
          slidePlan,
          slideNumber,
          session,
          project,
        });
        readySlides.push(slide);
      } catch (error) {
        if (!readySlides.length) throw error;
        const orderedReadySlides = [...readySlides].sort((a, b) => a.slideNumber - b.slideNumber);
        applyDeckPayload({
          title: planPayload.title,
          language: planPayload.language,
          outline: [],
          researchReport: planPayload.researchReport,
          design: planPayload.design,
          slides: orderedReadySlides,
        });
        state.activeSlideId = state.slides[state.slides.length - 1]?.id || state.activeSlideId;
        state.selectedElementId = '';
        rerender();
        await persist(true);
        const completedNumbers = new Set(readySlides.map((slide) => slide.slideNumber));
        const missingNumbers = normalizedPlans
          .filter((plan) => !completedNumbers.has(plan.slideNumber))
          .map((plan) => plan.slideNumber);
        const partialError = new Error(t('generationPartialDeck', { missing: missingNumbers.join(', ') }));
        partialError.pptLivePartialDeck = true;
        partialError.cause = error;
        if (error?.pptLiveRecoveryExhausted) {
          partialError.pptLiveRecoveryExhausted = error.pptLiveRecoveryExhausted;
        }
        throw partialError;
      }

      state.generation.draftedCount = readySlides.length;
      setGenerationStep('design', 'running', t('generationSlideReady', {
        slide: slideNumber,
        total: normalizedPlans.length,
      }));
      addGenerationEvent({
        title: t('generationSlideReady', { slide: slideNumber, total: normalizedPlans.length }),
        detail: '',
        kind: 'slide',
      });
      const orderedReadySlides = [...readySlides].sort((a, b) => a.slideNumber - b.slideNumber);
      applyDeckPayload({
        title: planPayload.title,
        language: planPayload.language,
        outline: orderedReadySlides.map((slide) => slide.title || ''),
        researchReport: planPayload.researchReport,
        design: planPayload.design,
        slides: orderedReadySlides,
      });
      state.activeSlideId = `ppt-live-slide-${slideNumber}`;
      state.selectedElementId = '';
      rerender();
    }

    const orderedReadySlides = [...readySlides].sort((a, b) => a.slideNumber - b.slideNumber);
    const finalSlides = orderedReadySlides;
    state.agentSession.id = session.id || state.agentSession.id;

    // Phase 4: finalize the complete deck and persist once.
    addGenerationEvent({ title: t('generationParsingDeck'), detail: '', kind: 'parsing' });
    setStatus(t('generationParsingDeck'));
    applyDeckPayload({
      title: planPayload.title,
      language: planPayload.language,
      outline: planOutlineTitles(planPayload),
      researchReport: planPayload.researchReport,
      design: planPayload.design,
      slides: finalSlides,
    });
    await saveHistorySnapshot(`agent:${operation}`);
    addGenerationEvent({ title: t('processEventDone'), detail: '', kind: 'done' });
    setGenerationStep('design', 'done');
    setGenerationStep('compile', 'done', t('generationCompiled'));
    finishGenerationUi(t('deckReady'));
    completed = true;
    rerender();
    await persist(true);
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
]);

function friendlyToolName(name) {
  const raw = String(name || '').trim();
  if (!raw) return t('eventUnknownTool');
  if (/^skill$/i.test(raw)) return t('eventToolSkillName');
  if (/^websearch$/i.test(raw)) return t('eventToolWebSearchName');
  if (/^webfetch$/i.test(raw)) return t('eventToolWebFetchName');
  return raw;
}

function shouldLogToolEvent(toolEvent, loggedToolEvents) {
  const normalized = normalizeToolEvent(toolEvent);
  const eventType = normalized.event_type || normalized.eventType || '';
  if (SILENT_TOOL_EVENT_TYPES.has(eventType)) return false;
  // One user-facing row per tool invocation; Started pairs are internal noise.
  if (eventType === 'Started' || eventType === 'EarlyDetected') return false;
  const toolName = String(normalized.tool_name || normalized.toolName || 'tool').toLowerCase();
  const params = normalized.params && typeof normalized.params === 'object' ? normalized.params : {};
  const path = String(params.file_path || params.path || params.command || '').trim();
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
  const phase = inferGenerationPhaseFromBuffer(buffer);
  const title = generationPhaseMessage(phase);
  setGenerationStep(phase, 'running', title);
  updateGenerationSlideProgress(buffer, phase);
  progressTracker.touch();
  void lastPhaseRef;
}

function describeToolEvent(event) {
  const toolEvent = normalizeToolEvent(event.toolEvent || {});
  const eventType = toolEvent.event_type || toolEvent.eventType || 'ToolEvent';
  const toolName = friendlyToolName(toolEvent.tool_name || toolEvent.toolName);
  const labels = {
    EarlyDetected: t('eventToolDetected'),
    ParamsPartial: t('eventToolParams'),
    Queued: t('eventToolQueued'),
    Waiting: t('eventToolWaiting'),
    Started: t('eventToolStarted'),
    Progress: t('eventToolProgress'),
    Streaming: t('eventToolStreaming'),
    StreamChunk: t('eventToolStreamChunk'),
    ConfirmationNeeded: t('eventToolConfirmation'),
    Confirmed: t('eventToolConfirmed'),
    Rejected: t('eventToolRejected'),
    Completed: t('eventToolCompleted'),
    Failed: t('eventToolFailed'),
    Cancelled: t('eventToolCancelled'),
  };
  const namedTypes = new Set(['EarlyDetected', 'Started', 'Completed', 'Failed', 'Cancelled', 'ConfirmationNeeded']);
  return {
    title: labels[eventType] || t('processEventTool'),
    detail: namedTypes.has(eventType) ? toolName : userFacingToolDetail(eventType, toolEvent),
    kind: eventType === 'Failed' || eventType === 'Cancelled' || eventType === 'Rejected' ? 'error' : 'tool',
  };
}

function userFacingToolDetail(eventType, toolEvent) {
  if (eventType === 'Failed') return compactText(toolEvent.error || t('backendGenerationFailed'));
  if (eventType === 'Completed') return '';
  if (eventType === 'Progress') return compactText(toolEvent.message || '');
  return '';
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

function applyDeckPayload(payload) {
  if (applyDeckPatchPayload(payload)) {
    if (payload.researchReport) applyResearchReport(payload.researchReport);
    if (payload.design?.palette && typeof payload.design.palette === 'object') {
      state.deckPalette = payload.design.palette;
    }
    return;
  }
  const htmlSlides = normalizeHtmlSlides(payload);
  if (htmlSlides.length) {
    state.title = String(payload.title || state.title || t('blankDeckTitle'));
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
    state.title = String(payload.title || state.title || t('blankDeckTitle'));
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

function applyDeckPatchPayload(payload) {
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
  state.title = String(payload.deckPatch?.title || payload.patch?.title || payload.title || state.title || t('blankDeckTitle'));
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
      setDensityIndex(pickDensityIndexFromClientX(event.clientX, densityTrack), { save: false });
      densityTrack.setPointerCapture(event.pointerId);
    });
    densityTrack.addEventListener('pointermove', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      setDensityIndex(pickDensityIndexFromClientX(event.clientX, densityTrack), { save: false });
    });
    densityTrack.addEventListener('pointerup', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      densityTrack.releasePointerCapture(event.pointerId);
      void restyleDeck();
    });
    densityTrack.addEventListener('pointercancel', (event) => {
      if (!densityTrack.hasPointerCapture(event.pointerId)) return;
      densityTrack.releasePointerCapture(event.pointerId);
      void persist(true);
    });
    densitySlider.querySelectorAll('[data-density-index]').forEach((tick) => {
      tick.addEventListener('click', (event) => {
        event.stopPropagation();
        setDensityIndex(tick.dataset.densityIndex);
        void restyleDeck();
      });
    });
    densitySlider.addEventListener('keydown', (event) => {
      const currentIndex = densityToIndex(state.style.density);
      if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
        event.preventDefault();
        setDensityIndex(currentIndex - 1);
        void restyleDeck();
      } else if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
        event.preventDefault();
        setDensityIndex(currentIndex + 1);
        void restyleDeck();
      } else if (event.key === 'Home') {
        event.preventDefault();
        setDensityIndex(0);
        void restyleDeck();
      } else if (event.key === 'End') {
        event.preventDefault();
        setDensityIndex(2);
        void restyleDeck();
      }
    });
  }

  /* Font family */
  document.querySelectorAll('[data-font-family]').forEach((button) => {
    button.addEventListener('click', () => {
      state.style.fontFamily = button.dataset.fontFamily === 'serif' ? 'serif' : 'sans';
      document.querySelectorAll('[data-font-family]').forEach((node) => {
        const active = node === button;
        node.classList.toggle('is-active', active);
        node.setAttribute('aria-pressed', active ? 'true' : 'false');
      });
      void restyleDeck();
      void persist(true);
    });
  });

  /* Slide color mode */
  document.querySelectorAll('[data-color-mode]').forEach((button) => {
    button.addEventListener('click', () => {
      state.style.colorMode = button.dataset.colorMode === 'dark' ? 'dark' : 'light';
      document.querySelectorAll('[data-color-mode]').forEach((node) => {
        const active = node === button;
        node.classList.toggle('is-active', active);
        node.setAttribute('aria-pressed', active ? 'true' : 'false');
      });
      void restyleDeck();
    });
  });

  /* Style preset */
  const stylePresetSelect = $('stylePresetSelect');
  if (stylePresetSelect) {
    renderStylePresetOptions();
    enhanceFlatSelect(stylePresetSelect);
    stylePresetSelect.value = state.style?.stylePreset || DEFAULT_STYLE_PRESET;
    refreshFlatSelect(stylePresetSelect);
    stylePresetSelect.addEventListener('change', () => {
      const selected = stylePresetSelect.value;
      if (selected) {
        state.style.stylePreset = selected;
        const preset = getStylePreset(selected);
        if (preset) {
          state.style.colorMode = preset.colorMode || 'light';
          state.style.fontFamily = preset.fontFamily || 'sans';
          state.style.density = preset.density || 'standard';
          // Sync UI toggles
          document.querySelectorAll('[data-color-mode]').forEach((node) => {
            const active = node.dataset.colorMode === state.style.colorMode;
            node.classList.toggle('is-active', active);
            node.setAttribute('aria-pressed', active ? 'true' : 'false');
          });
          document.querySelectorAll('[data-font-family]').forEach((node) => {
            const active = node.dataset.fontFamily === state.style.fontFamily;
            node.classList.toggle('is-active', active);
            node.setAttribute('aria-pressed', active ? 'true' : 'false');
          });
          syncDensitySlider(state.style.density);
        }
        void restyleDeck();
        void persist(true);
      }
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
  syncDensitySlider(state.style?.density);
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
