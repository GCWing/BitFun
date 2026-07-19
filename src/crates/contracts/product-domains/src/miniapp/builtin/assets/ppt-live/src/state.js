import { translate as t } from './i18n.js';
import { normalizeStylePresetKey } from './style-presets.js';

export const STORAGE_KEY = 'pptLiveStudioStateV6';
export const HISTORY_KEY = 'pptLiveDeckHistoryV1';
export const SCHEMA_VERSION = 6;
/** Default Cowork model selector when the user has not chosen one yet. */
export const DEFAULT_PREFERRED_MODEL = 'primary';

export function normalizePreferredModel(value) {
  const raw = String(value || '').trim();
  return raw || DEFAULT_PREFERRED_MODEL;
}
export const ELEMENT_TYPES = ['text', 'list', 'shape', 'metric', 'chart', 'media'];

export const THEME_PRESETS = {
  executive: {
    name: 'Executive',
    background: '#fbfcff',
    ink: '#111827',
    muted: '#5b6575',
    primary: '#0f766e',
    accent: '#f97316',
    panel: '#ffffff',
  },
  market: {
    name: 'Market',
    background: '#fffdf7',
    ink: '#1f2937',
    muted: '#6b5f50',
    primary: '#2563eb',
    accent: '#d97706',
    panel: '#ffffff',
  },
  minimal: {
    name: 'Minimal',
    background: '#f8fafc',
    ink: '#0f172a',
    muted: '#64748b',
    primary: '#334155',
    accent: '#0f766e',
    panel: '#ffffff',
  },
  studio: {
    name: 'Studio',
    background: '#fcfbff',
    ink: '#1f1630',
    muted: '#6c607a',
    primary: '#7c3aed',
    accent: '#db2777',
    panel: '#ffffff',
  },
};

export function uid(prefix = 'id') {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

export function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

export function defaultBrief() {
  return {
    topic: '',
    slideTarget: 0,
  };
}

export function methodologyFor(deckType = 'strategy') {
  const profiles = {
    strategy: {
      profile: 'strategy-leadership',
      thesis: 'Decision-led transformation narrative',
      proofObjects: ['market map', 'operating model', 'risk bridge', 'decision table'],
      arc: ['thesis', 'context', 'friction', 'strategic bet', 'operating model', 'proof', 'risks', 'decision'],
    },
    sales: {
      profile: 'gtm-growth',
      thesis: 'Buyer pain to differentiated value narrative',
      proofObjects: ['before/after workflow', 'value bridge', 'customer proof', 'implementation plan'],
      arc: ['outcome', 'market shift', 'pain', 'solution', 'proof', 'commercial case', 'rollout', 'call to action'],
    },
    report: {
      profile: 'finance-ir',
      thesis: 'Executive performance narrative with decisions attached',
      proofObjects: ['metric bridge', 'trend chart', 'variance table', 'risk register'],
      arc: ['summary', 'scorecard', 'movement', 'root cause', 'metric proof', 'risk', 'plan', 'decision'],
    },
    teaching: {
      profile: 'education',
      thesis: 'Concept to application learning journey',
      proofObjects: ['concept map', 'worked example', 'comparison', 'practice prompt'],
      arc: ['goal', 'map', 'concept', 'example', 'mistakes', 'practice', 'summary', 'next step'],
    },
    fundraising: {
      profile: 'fundraising',
      thesis: 'Venture-scale opportunity supported by traction evidence',
      proofObjects: ['market wedge', 'product diagram', 'traction chart', 'milestone plan'],
      arc: ['thesis', 'problem', 'solution', 'market', 'product', 'traction', 'model', 'ask'],
    },
  };
  return profiles[deckType] || profiles.strategy;
}

export function normalizeDensity(value = 'standard') {
  const raw = String(value || 'standard');
  if (raw === 'loose') return 'spacious';
  if (['compact', 'standard', 'spacious'].includes(raw)) return raw;
  return 'standard';
}

export const DENSITY_LEVELS = ['spacious', 'standard', 'compact'];

export function densityToIndex(value = 'standard') {
  const normalized = normalizeDensity(value);
  const index = DENSITY_LEVELS.indexOf(normalized);
  return index >= 0 ? index : 1;
}

export function indexToDensity(index = 1) {
  const clamped = Math.min(Math.max(Math.round(Number(index)), 0), DENSITY_LEVELS.length - 1);
  return DENSITY_LEVELS[clamped] || 'standard';
}

export function densityProfile(density = 'standard') {
  const normalized = normalizeDensity(density);
  const profiles = {
    spacious: { bulletLimit: 4, cardColumns: 3, cardGap: 2 },
    standard: { bulletLimit: 5, cardColumns: 4, cardGap: 1.8 },
    compact: { bulletLimit: 6, cardColumns: 4, cardGap: 1.2 },
  };
  return profiles[normalized] || profiles.standard;
}

export function normalizeSlideTarget(value = 0) {
  const num = Number(value);
  if (!Number.isFinite(num) || num <= 0) return 0;
  return clamp(num, 3, 24);
}

export function defaultStyle() {
  return {
    theme: 'executive',
    density: 'standard',
    fontFamily: 'sans',
    colorMode: 'light',
    stylePreset: 'clean-business',
  };
}

export function defaultOutline() {
  return [
    t('defaultDeckTitle'),
    'Why now',
    'Current friction',
    'Strategic answer',
    'Core workflow',
    'Proof and impact',
    'Rollout plan',
    'Decision and next steps',
  ];
}

export function createInitialState() {
  const state = {
    schemaVersion: SCHEMA_VERSION,
    sessionId: uid('deck'),
    title: t('blankDeckTitle'),
    brief: defaultBrief(),
    promptDraft: '',
    lastSubmittedPrompt: '',
    agentSession: {
      id: '',
      workspaceSubdir: '',
      runId: '',
      skillKey: '',
    },
    preferredModel: DEFAULT_PREFERRED_MODEL,
    style: defaultStyle(),
    outline: [],
    sources: { items: [], facts: [], warnings: [], summary: '', fetchedAt: 0 },
    slides: [],
    activeSlideId: '',
    selectedElementId: '',
    mode: 'edit',
    presentIndex: 0,
    status: 'ready',
    generation: {
      active: false,
      current: 'idle',
      steps: generationSteps().map((step) => ({ ...step, status: 'pending' })),
      events: [],
    },
    chatMessages: [{ role: 'assistant', text: t('assistantHello') }],
    updatedAt: Date.now(),
  };
  return state;
}

export function ensureState(value) {
  const state = {
    ...createInitialState(),
    ...(value || {}),
  };
  state.schemaVersion = SCHEMA_VERSION;
  const legacyBrief = state.brief || {};
  state.brief = {
    ...defaultBrief(),
    topic: String(legacyBrief.topic || state.promptDraft || '').trim(),
    slideTarget: normalizeSlideTarget(legacyBrief.slideTarget),
  };
  state.promptDraft = typeof state.promptDraft === 'string' ? state.promptDraft : '';
  state.lastSubmittedPrompt = typeof state.lastSubmittedPrompt === 'string' ? state.lastSubmittedPrompt : '';
  state.agentSession = {
    id: String(state.agentSession?.id || ''),
    workspaceSubdir: String(state.agentSession?.workspaceSubdir || ''),
    runId: String(state.agentSession?.runId || ''),
    skillKey: String(state.agentSession?.skillKey || ''),
  };
  state.preferredModel = normalizePreferredModel(state.preferredModel);
  state.style = { ...defaultStyle(), ...(state.style || {}) };
  delete state.style.brandPrimary;
  delete state.style.brandAccent;
  if (!Object.keys(THEME_PRESETS).includes(state.style.theme)) state.style.theme = 'executive';
  if (!['compact', 'standard', 'spacious', 'loose'].includes(state.style.density)) state.style.density = 'standard';
  state.style.density = normalizeDensity(state.style.density);
  if (!['sans', 'serif'].includes(state.style.fontFamily)) {
    state.style.fontFamily = state.style.fontFamily === 'serif' ? 'serif' : 'sans';
  }
  if (!['light', 'dark'].includes(state.style.colorMode)) state.style.colorMode = 'light';
  state.style.stylePreset = normalizeStylePresetKey(
    typeof state.style.stylePreset === 'string' ? state.style.stylePreset : '',
  );
  state.generation = normalizeGeneration(state.generation);
  state.sources = normalizeSources(state.sources);
  state.brief.slideTarget = normalizeSlideTarget(state.brief.slideTarget);
  const keepEmptyGeneratingDeck = state.generation.active
    && Array.isArray(state.slides)
    && state.slides.length === 0;
  state.outline = keepEmptyGeneratingDeck
    ? []
    : Array.isArray(state.outline)
    ? state.outline.map((item) => String(item || t('newSlideTitle')))
    : [];
  state.slides = keepEmptyGeneratingDeck
    ? []
    : Array.isArray(state.slides) && state.slides.length > 0
    ? state.slides.map((slide, index) => normalizeSlide(slide, index, state))
    : state.outline.length > 0
    ? state.outline.map((title, index) => makeSlide(title, index, state.outline.length, state))
    : [];
  if (!state.slides.some((slide) => slide.id === state.activeSlideId)) {
    state.activeSlideId = state.slides[0]?.id || '';
  }
  const active = getActiveSlide(state);
  if (!active?.elements.some((element) => element.id === state.selectedElementId)) {
    state.selectedElementId = active?.elements[0]?.id || '';
  }
  state.title = state.title || state.slides[0]?.title || t('defaultDeckTitle');
  state.updatedAt = Date.now();
  return state;
}

export function normalizeSources(value = {}) {
  return {
    items: Array.isArray(value.items) ? value.items : [],
    facts: Array.isArray(value.facts) ? value.facts : [],
    warnings: Array.isArray(value.warnings) ? value.warnings : [],
    summary: typeof value.summary === 'string' ? value.summary : '',
    fetchedAt: Number(value.fetchedAt || 0),
  };
}

/** File-protocol phases shown in the Process panel (not the legacy planning spine). */
export const GENERATION_PHASE_ORDER = ['skill', 'outline', 'slides', 'verify'];

export function generationSteps() {
  return [
    { id: 'skill', label: t('generationStepSkill'), detail: t('generationStepSkillDetail') },
    { id: 'outline', label: t('generationStepOutline'), detail: t('generationStepOutlineDetail') },
    { id: 'slides', label: t('generationStepSlides'), detail: t('generationStepSlidesDetail') },
    { id: 'verify', label: t('generationStepVerify'), detail: t('generationStepVerifyDetail') },
  ];
}

const GENERATION_EVENT_LIMIT = 80;
const GENERATION_STREAM_LIMIT = 200;

function normalizeGenerationEvent(event = {}) {
  const source = typeof event === 'string' ? { title: event } : event || {};
  const title = String(source.title || source.label || source.message || t('processEventUnknown')).trim()
    || t('processEventUnknown');
  const kind = String(source.kind || 'info').toLowerCase().replace(/[^a-z0-9-]/g, '') || 'info';
  const timestamp = Number(source.timestamp || source.time || 0) || Date.now();
  return {
    id: String(source.id || uid('generation-event')),
    seq: Number(source.seq) || 0,
    title,
    detail: String(source.detail || source.description || '').trim(),
    kind,
    timestamp,
  };
}

export function normalizeGeneration(value = {}) {
  const known = new Map((Array.isArray(value.steps) ? value.steps : []).map((step) => [step.id, step]));
  const events = Array.isArray(value.events)
    ? value.events.map(normalizeGenerationEvent).slice(-GENERATION_EVENT_LIMIT)
    : [];
  const maxEventSeq = events.reduce((max, event) => Math.max(max, Number(event.seq) || 0), 0);
  const stream = Array.isArray(value.agentStream)
    ? value.agentStream.slice(-GENERATION_STREAM_LIMIT)
    : [];
  return {
    active: Boolean(value.active),
    current: value.current || 'idle',
    draftedCount: Number(value.draftedCount) || 0,
    slideTarget: Number(value.slideTarget) || 0,
    eventSeq: Math.max(Number(value.eventSeq) || 0, maxEventSeq),
    steps: generationSteps().map((step) => ({
      ...step,
      status: known.get(step.id)?.status || 'pending',
    })),
    events,
    agentStream: stream,
  };
}

export function getActiveSlide(state) {
  return state.slides.find((slide) => slide.id === state.activeSlideId) || state.slides[0];
}

export function getActiveIndex(state) {
  return Math.max(0, state.slides.findIndex((slide) => slide.id === state.activeSlideId));
}

export function getSelectedElement(state) {
  const slide = getActiveSlide(state);
  return slide?.elements.find((element) => element.id === state.selectedElementId) || null;
}

export function makeSlide(title, index, total, state = { brief: defaultBrief(), style: defaultStyle(), slides: [] }) {
  const theme = resolveDeckTheme(state, index);
  const slide = {
    id: uid('slide'),
    title: title || `${t('newSlideTitle')} ${index + 1}`,
    subtitle: '',
    kicker: kickerForIndex(index, state),
    claim: claimFor(title, index, state),
    proofObject: proofObjectForIndex(index, state),
    supportNote: supportNoteFor(title, index, state),
    sourceNote: sourceNoteFor(state),
    notes: t('defaultSpeakerNote', { title }),
    layout: layoutForIndex(index, total),
    theme,
    elements: [],
  };
  slide.elements = elementsForLayout(slide, index, total, state);
  return normalizeSlide(slide, index, state);
}

export function normalizeSlide(slide, index, state) {
  const title = slide?.title || `${t('newSlideTitle')} ${index + 1}`;
  const normalized = {
    id: slide?.id || uid('slide'),
    title,
    subtitle: slide?.subtitle || '',
    kicker: String(slide?.kicker || kickerForIndex(index, state)),
    claim: String(slide?.claim || claimFor(title, index, state)),
    proofObject: String(slide?.proofObject || proofObjectForIndex(index, state)),
    supportNote: String(slide?.supportNote || supportNoteFor(title, index, state)),
    sourceNote: String(slide?.sourceNote || sourceNoteFor(state)),
    notes: slide?.notes || '',
    layout: slide?.layout || layoutForIndex(index, state?.slides?.length || 1),
    theme: { ...resolveDeckTheme(state, index), ...(slide?.theme || slide?.style || {}) },
    html: typeof slide?.html === 'string' ? slide.html : '',
    quality: normalizeSlideQuality(slide?.quality),
    elements: [],
  };
  const source = Array.isArray(slide?.elements) && slide.elements.length > 0
    ? slide.elements
    : elementsForLayout(normalized, index, state?.slides?.length || 1, state);
  normalized.elements = source.map((element) => normalizeElement(element));
  if (normalized.html) {
    const extracted = extractHtmlSlideBackground(normalized.html);
    if (extracted) normalized.theme.background = extracted;
  }
  return normalized;
}

export function normalizeSlideQuality(value = {}) {
  const issues = Array.isArray(value?.issues) ? value.issues : [];
  return {
    score: clamp(Number(value?.score ?? 100), 0, 100),
    issues: issues.slice(0, 12).map((issue) => ({
      id: String(issue?.id || uid('quality')),
      severity: ['high', 'medium', 'low'].includes(issue?.severity) ? issue.severity : 'low',
      type: String(issue?.type || 'quality'),
      message: String(issue?.message || ''),
    })).filter((issue) => issue.message),
  };
}

export function normalizeElement(element = {}) {
  const type = ELEMENT_TYPES.includes(element.type) ? element.type : 'text';
  const defaults = defaultElement(type);
  return {
    ...defaults,
    ...element,
    id: element.id || uid('el'),
    type,
    x: clamp(Number(element.x ?? defaults.x), 0, 98),
    y: clamp(Number(element.y ?? defaults.y), 0, 98),
    w: clamp(Number(element.w ?? defaults.w), 3, 100),
    h: clamp(Number(element.h ?? defaults.h), 3, 100),
    text: typeof element.text === 'string' ? element.text : defaults.text,
    label: typeof element.label === 'string' ? element.label : defaults.label,
    items: Array.isArray(element.items) ? element.items.map(String) : defaults.items,
    data: Array.isArray(element.data) ? element.data.map(normalizeChartPoint) : defaults.data,
    style: normalizeStyle({ ...defaults.style, ...(element.style || {}) }),
  };
}

function normalizeChartPoint(point, index) {
  if (typeof point === 'number') return { label: `Q${index + 1}`, value: point };
  return {
    label: String(point?.label || `Item ${index + 1}`),
    value: Number(point?.value || 0),
  };
}

export function normalizeStyle(style = {}) {
  return {
    fontSize: clamp(Number(style.fontSize || 24), 8, 88),
    fontWeight: clamp(Number(style.fontWeight || 600), 100, 900),
    color: style.color || 'ink',
    background: style.background || 'transparent',
    opacity: clamp(Number(style.opacity ?? 1), 0, 1),
    borderRadius: clamp(Number(style.borderRadius || 0), 0, 99),
    align: style.align || 'left',
  };
}

export function defaultElement(type) {
  const map = {
    text: {
      text: 'Key message',
      label: '',
      items: [],
      data: [],
      x: 8,
      y: 12,
      w: 60,
      h: 16,
      style: { fontSize: 38, fontWeight: 780, color: 'ink', background: 'transparent', borderRadius: 0, opacity: 1, align: 'left' },
    },
    list: {
      text: '',
      label: '',
      items: ['First point', 'Second point', 'Third point'],
      data: [],
      x: 9,
      y: 36,
      w: 48,
      h: 40,
      style: { fontSize: 20, fontWeight: 500, color: 'ink', background: 'transparent', borderRadius: 8, opacity: 1, align: 'left' },
    },
    shape: {
      text: '',
      label: '',
      items: [],
      data: [],
      x: 66,
      y: 14,
      w: 24,
      h: 62,
      style: { fontSize: 18, fontWeight: 600, color: 'accent', background: 'primary', borderRadius: 22, opacity: 0.12, align: 'center' },
    },
    metric: {
      text: '3x',
      label: 'Faster first draft',
      items: [],
      data: [],
      x: 63,
      y: 42,
      w: 26,
      h: 26,
      style: { fontSize: 44, fontWeight: 820, color: 'primary', background: 'panel', borderRadius: 14, opacity: 1, align: 'left' },
    },
    chart: {
      text: 'Signal trend',
      label: '',
      items: [],
      data: [{ label: 'Now', value: 42 }, { label: 'Next', value: 68 }, { label: 'Target', value: 86 }],
      x: 52,
      y: 36,
      w: 36,
      h: 32,
      style: { fontSize: 18, fontWeight: 700, color: 'ink', background: 'panel', borderRadius: 14, opacity: 1, align: 'left' },
    },
    media: {
      text: t('mediaPlaceholder'),
      label: '',
      items: [],
      data: [],
      x: 58,
      y: 18,
      w: 32,
      h: 42,
      style: { fontSize: 16, fontWeight: 650, color: 'muted', background: 'soft', borderRadius: 16, opacity: 1, align: 'center' },
    },
  };
  return { ...clone(map[type] || map.text), type: map[type] ? type : 'text' };
}

function resolveDeckTheme(state, index = 0) {
  const deckPalette = state?.deckPalette;
  if (deckPalette && typeof deckPalette === 'object') {
    const primary = deckPalette.primary || '#111111';
    const accent = deckPalette.accent || '#c84b31';
    return ensureThemeContrast({
      name: 'deck',
      background: deckPalette.background || '#111111',
      ink: deckPalette.ink || '#f8fafc',
      muted: deckPalette.muted || '#cbd5e1',
      primary: index % 2 ? accent : primary,
      accent: index % 2 ? primary : accent,
      panel: deckPalette.panel || '#1f2937',
    });
  }
  const preset = THEME_PRESETS[state?.style?.theme || 'executive'] || THEME_PRESETS.executive;
  const primary = preset.primary;
  const accent = preset.accent;
  return ensureThemeContrast({
    ...preset,
    primary: index % 2 ? accent : primary,
    accent: index % 2 ? primary : accent,
  });
}

export function extractHtmlSlideBackground(html) {
  const source = String(html || '');
  const patterns = [
    /body\s*\{[^}]*background(?:-color)?\s*:\s*([^;}\n]+)/i,
    /<body[^>]*style="[^"]*background(?:-color)?\s*:\s*([^;"']+)/i,
    /html\s*\{[^}]*background(?:-color)?\s*:\s*([^;}\n]+)/i,
    /:root\s*\{[^}]*background(?:-color)?\s*:\s*([^;}\n]+)/i,
    /background(?:-color)?\s*:\s*(#[0-9a-f]{3,8}|rgb[a]?\([^)]+\)|hsl[a]?\([^)]+\)|black|white)/i,
  ];
  for (const pattern of patterns) {
    const match = source.match(pattern);
    if (!match) continue;
    const color = normalizeCssColor(match[1]);
    if (color) return color;
  }
  return null;
}

function normalizeCssColor(value) {
  const raw = String(value || '').trim().replace(/\s+!important$/i, '');
  if (!raw || /^transparent$/i.test(raw)) return null;
  if (/^#[0-9a-f]{3,8}$/i.test(raw)) return normalizeHex(raw, raw);
  if (/^rgb/i.test(raw) || /^hsl/i.test(raw)) return raw;
  const named = {
    black: '#000000',
    white: '#ffffff',
    transparent: null,
  };
  if (Object.prototype.hasOwnProperty.call(named, raw.toLowerCase())) {
    return named[raw.toLowerCase()];
  }
  return raw;
}

function ensureThemeContrast(theme) {
  const background = normalizeHex(theme.background, '#ffffff');
  const panel = normalizeHex(theme.panel, '#ffffff');
  return {
    ...theme,
    background,
    panel,
    ink: readableOn(background, theme.ink, '#111827', '#f8fafc', 7),
    muted: readableOn(background, theme.muted, '#4b5563', '#cbd5e1', 4.5),
    primary: readableOn(panel, theme.primary, '#0f766e', '#5eead4', 4.5),
    accent: readableOn(panel, theme.accent, '#c2410c', '#fdba74', 4.5),
  };
}

function readableOn(background, candidate, darkFallback, lightFallback, minRatio) {
  const bg = normalizeHex(background, '#ffffff');
  const color = normalizeHex(candidate, darkFallback);
  if (contrastRatio(bg, color) >= minRatio) return color;
  const dark = normalizeHex(darkFallback, '#111827');
  const light = normalizeHex(lightFallback, '#f8fafc');
  return contrastRatio(bg, dark) >= contrastRatio(bg, light) ? dark : light;
}

function contrastRatio(a, b) {
  const l1 = relativeLuminance(a);
  const l2 = relativeLuminance(b);
  const light = Math.max(l1, l2);
  const dark = Math.min(l1, l2);
  return (light + 0.05) / (dark + 0.05);
}

function relativeLuminance(hex) {
  const { r, g, b } = hexToRgb(hex);
  return [r, g, b]
    .map((value) => {
      const channel = value / 255;
      return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
    })
    .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
}

function normalizeHex(value, fallback) {
  const raw = String(value || '').trim();
  const short = raw.match(/^#([0-9a-f]{3})$/i);
  if (short) return `#${short[1].split('').map((part) => part + part).join('')}`.toLowerCase();
  if (/^#[0-9a-f]{6}$/i.test(raw)) return raw.toLowerCase();
  return fallback;
}

function hexToRgb(hex) {
  const raw = normalizeHex(hex, '#000000').slice(1);
  const value = parseInt(raw, 16);
  return { r: (value >> 16) & 255, g: (value >> 8) & 255, b: value & 255 };
}

function layoutForIndex(index, total) {
  if (index === 0) return 'cover';
  if (index === total - 1) return 'closing';
  return ['split', 'metric', 'process', 'comparison'][index % 4];
}

function kickerForIndex(index, state) {
  const method = methodologyFor();
  const role = method.arc[index % method.arc.length] || 'proof';
  return role.replace(/[-_]/g, ' ').toUpperCase();
}

function proofObjectForIndex(index, state) {
  const method = methodologyFor();
  const proof = method.proofObjects[index % method.proofObjects.length] || 'visual proof';
  const labels = {
    'market map': t('proofMarketMap'),
    'operating model': t('proofOperatingModel'),
    'risk bridge': t('proofRiskBridge'),
    'decision table': t('proofDecisionTable'),
    'before/after workflow': t('proofBeforeAfter'),
    'value bridge': t('proofValueBridge'),
    'customer proof': t('proofCustomerProof'),
    'implementation plan': t('proofImplementationPlan'),
    'metric bridge': t('proofMetricBridge'),
    'trend chart': t('proofTrendChart'),
    'variance table': t('proofVarianceTable'),
    'risk register': t('proofRiskRegister'),
    'concept map': t('proofConceptMap'),
    'worked example': t('proofWorkedExample'),
    comparison: t('proofComparison'),
    'practice prompt': t('proofPracticePrompt'),
    'market wedge': t('proofMarketWedge'),
    'product diagram': t('proofProductDiagram'),
    'traction chart': t('proofTractionChart'),
    'milestone plan': t('proofMilestonePlan'),
    'visual proof': t('proofVisualProof'),
  };
  return labels[proof] || proof;
}

function claimFor(title, index, state) {
  const topic = state?.brief?.topic || state?.title || title;
  if (index === 0) return t('claimCover', { topic });
  if (title && /[.!?。！？]$/.test(title.trim())) return title;
  const stems = [
    t('claimPressure', { title }),
    t('claimDecision', { title }),
    t('claimProof', { title }),
    t('claimAction', { title }),
  ];
  return stems[index % stems.length];
}

function supportNoteFor(title, index, state) {
  const proof = proofObjectForIndex(index, state);
  return t('supportWithAssumption', { proof });
}

function sourceNoteFor(state) {
  return t('sourceDraftAssumption');
}

function elementsForLayout(slide, index, total, state) {
  const title = slide.title;
  const profile = densityProfile(state?.style?.density);
  const points = pointsFor(title, index, state)
    .slice(0, profile.bulletLimit)
    .map((point) => String(point).slice(0, 90));
  const pattern = fallbackPatternForSlide(slide, index, total);
  if (pattern === 'cover') {
    return [
      element('shape', { x: 6, y: 9, w: 88, h: 76, style: { background: 'soft', opacity: 1, borderRadius: 28 } }),
      element('shape', { x: 9, y: 15, w: 1.2, h: 55, style: { background: 'primary', opacity: 1, borderRadius: 99 } }),
      element('text', { text: slide.kicker, x: 13, y: 15, w: 22, h: 5, style: { fontSize: 10, fontWeight: 760, color: 'primary' } }),
      element('text', { text: title, x: 13, y: 23, w: 58, h: 25, style: { fontSize: title.length > 48 ? 34 : 44, fontWeight: 840 } }),
      element('text', { text: slide.claim, x: 14, y: 55, w: 45, h: 11, style: { fontSize: 18, fontWeight: 520, color: 'muted' } }),
      element('metric', { text: String(total), label: t('slidesUnit'), x: 75, y: 54, w: 14, h: 17, style: { fontSize: 34 } }),
    ];
  }
  if (pattern === 'closing') {
    return [
      element('text', { text: title, x: 9, y: 15, w: 65, h: 15, style: { fontSize: title.length > 48 ? 30 : 38, fontWeight: 820 } }),
      element('text', { text: slide.claim, x: 10, y: 33, w: 46, h: 9, style: { fontSize: 17, fontWeight: 540, color: 'muted' } }),
      ...fallbackCards([t('closeConfirm'), t('closeOwner'), t('closeIteration')], 10, 50, 52, 22, 3),
      element('text', { text: points[0] || slide.supportNote, x: 67, y: 48, w: 22, h: 20, style: { fontSize: 18, fontWeight: 720, color: 'primary', background: 'soft', borderRadius: 20 } }),
    ];
  }
  if (pattern === 'process') {
    return [
      element('text', { text: title, x: 8, y: 10, w: 68, h: 12, style: { fontSize: 32, fontWeight: 820 } }),
      element('text', { text: slide.claim, x: 9, y: 25, w: 54, h: 7, style: { fontSize: 15, fontWeight: 520, color: 'muted' } }),
      element('shape', { x: 10, y: 50, w: 78, h: 1.2, style: { background: 'primary', opacity: 0.25, borderRadius: 99 } }),
      ...fallbackCards(
        points.map((point, pointIndex) => `0${pointIndex + 1}  ${point}`),
        10,
        37,
        78,
        28,
        Math.min(profile.cardColumns, Math.max(2, points.length)),
        profile.cardGap,
      ),
    ];
  }
  if (pattern === 'comparison') {
    return [
      element('text', { text: title, x: 7, y: 10, w: 72, h: 12, style: { fontSize: 32, fontWeight: 820 } }),
      element('text', { text: slide.claim, x: 8, y: 25, w: 48, h: 7, style: { fontSize: 15, fontWeight: 520, color: 'muted' } }),
      ...fallbackCards(points, 8, 37, 82, 30, 2, profile.cardGap),
    ];
  }
  if (pattern === 'data') {
    return [
      element('text', { text: title, x: 8, y: 10, w: 66, h: 12, style: { fontSize: 32, fontWeight: 820 } }),
      element('text', { text: slide.claim, x: 9, y: 25, w: 47, h: 7, style: { fontSize: 15, fontWeight: 520, color: 'muted' } }),
      element('metric', { text: String(index).padStart(2, '0'), label: slide.proofObject, x: 10, y: 40, w: 34, h: 28, style: { fontSize: 44 } }),
      element('text', { text: points[0] || slide.supportNote, x: 69, y: 41, w: 20, h: 24, style: { fontSize: 17, fontWeight: 700, color: 'primary', background: 'soft', borderRadius: 18 } }),
    ];
  }
  if (pattern === 'cards') {
    return [
      element('text', { text: title, x: 8, y: 10, w: 68, h: 12, style: { fontSize: 32, fontWeight: 820 } }),
      element('text', { text: slide.claim, x: 9, y: 25, w: 51, h: 8, style: { fontSize: 15, fontWeight: 520, color: 'muted' } }),
      ...fallbackCards(points, 9, 38, 78, 28, profile.cardColumns, profile.cardGap),
    ];
  }
  return [
    element('text', { text: title, x: 10, y: 15, w: 62, h: 15, style: { fontSize: title.length > 48 ? 30 : 38, fontWeight: 820 } }),
    element('text', { text: slide.claim, x: 11, y: 34, w: 42, h: 10, style: { fontSize: 17, fontWeight: 520, color: 'muted' } }),
    element('text', { text: points[0] || slide.supportNote, x: 58, y: 38, w: 28, h: 24, style: { fontSize: 22, fontWeight: 760, color: 'primary', background: 'soft', borderRadius: 22 } }),
    element('shape', { x: 10, y: 72, w: 18, h: 0.6, style: { background: 'primary', opacity: 1, borderRadius: 99 } }),
  ];
}

function fallbackPatternForSlide(slide, index, total) {
  const raw = [slide.layout, slide.kicker, slide.proofObject, slide.claim, slide.title].join(' ').toLowerCase();
  if (index === 0 || slide.layout === 'cover') return 'cover';
  if (index === total - 1 || slide.layout === 'closing') return 'closing';
  if (/process|workflow|timeline|roadmap|journey|steps|architecture|flow|流程|步骤|路线|架构/.test(raw)) return 'process';
  if (/compare|comparison|versus|matrix|before|after|risk|对比|比较|矩阵|风险/.test(raw)) return 'comparison';
  if (/data|metric|trend|scorecard|chart|number|数据|指标|趋势/.test(raw)) return 'data';
  return index % 3 === 1 ? 'cards' : 'spotlight';
}

function fallbackCards(items, x, y, w, h, columns, gap = 2.5) {
  const safeItems = items.filter(Boolean);
  const safeColumns = Math.max(1, Math.min(columns || 1, safeItems.length || 1));
  const safeGap = Number.isFinite(gap) ? gap : 2.5;
  const rows = Math.max(1, Math.ceil((safeItems.length || 1) / safeColumns));
  const cardW = (w - safeGap * (safeColumns - 1)) / safeColumns;
  const cardH = (h - safeGap * (rows - 1)) / rows;
  return safeItems.map((item, itemIndex) => element('text', {
    text: item,
    x: x + (itemIndex % safeColumns) * (cardW + safeGap),
    y: y + Math.floor(itemIndex / safeColumns) * (cardH + safeGap),
    w: cardW,
    h: cardH,
    style: {
      fontSize: 17,
      fontWeight: itemIndex === 0 ? 760 : 620,
      color: itemIndex === 0 ? 'primary' : 'ink',
      background: itemIndex === 0 ? 'soft' : 'panel',
      borderRadius: 18,
    },
  }));
}

function element(type, overrides) {
  const base = defaultElement(type);
  return { ...base, ...overrides, style: { ...base.style, ...(overrides.style || {}) } };
}

function pointsFor(title, index, state) {
  const topic = state?.brief?.topic || title;
  const proof = proofObjectForIndex(index, state);
  const pool = [
    `${t('pointClaimPrefix')} ${claimFor(title, index, state)}`,
    `${t('pointProofPrefix')} ${proof}`,
    `${t('pointAudiencePrefix')} ${topic}`,
    t('pointEvidenceRule'),
    t('pointDesignRule'),
    t('pointCloseRule'),
  ];
  const limit = densityProfile(state?.style?.density).bulletLimit;
  const picks = [];
  for (let offset = 0; offset < limit; offset += 1) {
    picks.push(pool[(index + offset) % pool.length]);
  }
  return picks;
}
