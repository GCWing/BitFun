import { translate as t, getLocale } from './i18n.js';
import { clone, ensureState, makeSlide, normalizeSlide, uid } from './state.js';
import { STYLE_PRESETS } from './style-presets.js';

const PPT_DESIGN_SKILL_CONTEXT = [
  'You are PPT Live, a presentation design engine embedded in BitFun.',
  'The user is the final decision maker. Execute the PPT task end to end and do not impose any fixed content agenda on the topic.',
  '',
  'Production method:',
  '1. Publish assumptions: audience, page count, design style, and theme.',
  '2. Produce an assertion-led outline with one message per slide.',
  '3. Ground facts in pasted material, explicit URLs, or clearly marked assumptions.',
  '4. Apply the ppt-design anti-slop rules: no purple gradient gimmicks, no emoji icons, no generic illustration filler, and no text-heavy pages.',
  '5. Assemble the final editable deck blueprint with concise visible text and useful speaker notes.',
  '',
  'Benchmark design target:',
  '- Match the professional feel of beautiful.ai and gamma.app: smart templates, strong typographic hierarchy, generous whitespace, crisp card/grid systems, and content-aware layouts.',
  '- Select the layout from the content type: cover, assertion split, evidence cards, data/chart, process/timeline, comparison matrix, quote/transition, and closing action.',
  '- Prefer one dominant visual object per slide. Use proof objects, metric cards, compact charts, or structured text panels instead of filling space with bullets.',
  '- Keep visible copy presentation-ready: one assertion title, one support line, and up to three short proof points unless the slide is explicitly a comparison or process.',
  '',
  'Design principles from ppt-design:',
  '- Use the user order and verified material as the only content authority.',
  '- Every page carries one core message and keeps visible text concise.',
  '- Keep titles concrete, assertion-led, and connected to the actual subject.',
  '- If material is thin, clearly mark unknowns and verification notes while still producing a useful draft.',
].join('\n');

export function buildBriefFromInputs(state) {
  const brief = { ...state.brief };
  if (!brief.slideTarget) delete brief.slideTarget;
  return {
    ...brief,
    title: state.title,
    currentOutline: state.outline,
    style: state.style,
    sources: state.sources || null,
    locale: getLocale(),
  };
}

export async function planPresentationTaskWithAi(state, instruction) {
  const schema = {
    operation: 'generate_deck|revise_deck|revise_slide|insert_slide|delete_slide|update_outline',
    scope: 'deck|current_slide|slide_index',
    slideIndex: null,
    briefPatch: {
      topic: 'optional refined topic',
      audience: 'optional refined audience',
      slideTarget: null,
      intent: 'free-form inferred purpose, only if stated or strongly implied',
      tone: 'free-form tone, only if stated or strongly implied',
    },
    needsSources: true,
    reason: 'why this operation is the right next step',
    steps: [
      { stage: 'brief|research|verification|outline|visual|assembly', task: 'work to do', deliverable: 'expected output' },
    ],
    acceptanceCriteria: ['What must be true when done'],
  };
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    `Shape: ${JSON.stringify(schema)}.`,
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `User order: ${instruction || ''}.`,
    `Current deck state: ${JSON.stringify({
      title: state.title,
      brief: state.brief,
      slideCount: state.slides?.length || 0,
      activeSlideIndex: Math.max(0, state.slides?.findIndex((slide) => slide.id === state.activeSlideId) ?? 0),
      outline: state.outline,
      currentSlide: state.slides?.find((slide) => slide.id === state.activeSlideId) || state.slides?.[0] || null,
    })}.`,
    'Choose the next executable operation autonomously. Prefer deck-level work when the user asks for a presentation outcome, structural change, rewrite, expansion, deletion by theme, or ambiguous improvement.',
    'Choose current-slide revision only when the user clearly targets the current page/slide.',
    'Choose delete_slide only when the user clearly asks to remove the current slide or a numbered slide. For deleting duplicate, weak, or irrelevant content, choose revise_deck so the deck can be reorganized.',
    'Set briefPatch only for fields inferred from the user order. Keep steps short and operational.',
  ].join('\n');
  const data = await askAi(prompt, 1200);
  return normalizeAgentPlan(data, state);
}

export async function enrichSources(state) {
  const urls = extractUrls(`${state.brief.topic || ''}\n${state.brief.material || ''}`);
  const manual = stripUrls(`${state.brief.topic || ''}\n${state.brief.material || ''}`).trim();
  const sources = {
    items: [],
    facts: [],
    warnings: [],
    summary: '',
    fetchedAt: Date.now(),
  };
  if (manual.length >= 20) {
    sources.items.push({ kind: 'user-text', title: t('sourceManualTitle'), url: '', text: manual.slice(0, 5000) });
  }
  for (const url of urls.slice(0, 3)) {
    try {
      const fetched = await fetchReadableSources(url);
      sources.items.push(...fetched);
    } catch (error) {
      window.app?.log?.warn?.('PPT Live source fetch failed', { url, error: String(error) });
      sources.warnings.push(t('sourceFetchFailed', { url }));
    }
  }
  const combined = sources.items.map((item) => `${item.title}\n${item.text}`).join('\n\n').slice(0, 14000);
  sources.facts = extractFacts(combined);
  sources.summary = summarizeSource(combined, sources);
  if (!sources.items.length) sources.warnings.push(t('sourceMissingWarning'));
  state.sources = sources;
  return sources;
}

export async function generateOutlineWithAi(state) {
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    'Shape: {"title":"deck title","outline":["slide title", "..."]}.',
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `Brief: ${JSON.stringify(buildBriefFromInputs(state))}.`,
    'Generate the PPT outline as the Story academy.',
    'The outline must directly answer the user order and the fetched/pasted source. Do not substitute any preselected content agenda.',
    'Use TED 3S: Hook -> context -> core evidence -> shift -> takeaway. One concrete idea per slide.',
    'Every slide title must use concrete nouns from the user topic or source instead of abstract placeholders.',
    'Respect brief.slideTarget only when it is a positive number. Otherwise choose an appropriate outline length from the topic and material.',
  ].join('\n');
  const data = await askAi(prompt, 1000);
  if (!Array.isArray(data?.outline) || data.outline.length === 0) throw new Error('Invalid outline');
  const target = Number(state.brief.slideTarget) || 0;
  const outline = target > 0 ? data.outline.slice(0, target).map(String) : data.outline.map(String);
  return {
    title: data.title || data.outline[0] || state.title,
    outline,
  };
}

export async function generateDeckWithAi(state) {
  const schema = {
    title: 'Deck title',
    slides: [
      {
        role: 'cover|content|data|transition|closing',
        narrativeStage: 'hook|context|core|shift|takeaway',
        title: 'Source-specific slide title',
        kicker: '1-3 word slide role',
        claim: 'One concrete idea this slide communicates',
        proofObject: 'source-backed proof or visual direction for this page',
        supportNote: 'What source fact or assumption supports this slide',
        sourceNote: 'Source URL/name or verification note',
        facts: ['Source-backed fact or clearly marked assumption, using source vocabulary'],
        bullets: ['Short visible text, max 12 Chinese chars or 8 English words when possible'],
        metric: { value: 'Only if explicitly present in source', label: 'Metric label' },
        chartData: [{ label: 'Only source-backed label', value: 0 }],
        notes: 'Speaker notes',
        layout: 'cover|brief|evidence|process|comparison|quote|data|closing',
      },
    ],
  };
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    `Shape: ${JSON.stringify(schema)}.`,
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `Brief: ${JSON.stringify(buildBriefFromInputs(state))}.`,
    `Confirmed outline: ${JSON.stringify(state.outline)}.`,
    'Generate the final editable deck blueprint after internal research, verification, outline, visual planning, and assembly steps.',
    'Content fidelity is the top priority. Every slide must be about the user-requested topic/source. Do not introduce unrelated framing unless it is present in the user order or source.',
    'Use the user brief and source vocabulary aggressively: names, concepts, claims, examples, data, constraints, and domain-specific terms that actually appear in the material.',
    'Every non-cover slide must have exactly one dominant message and, when useful, a visual direction selected by the content.',
    'Visible text should be concise and presentation-ready. Speaker notes can carry explanation.',
    'Use chart/data slides only when numeric data exists in the source. Never invent precise numbers.',
    'If a source could not be read, mark that specific source as unavailable. If a source was read, do not say it was unread.',
    'Before returning JSON, self-check: (1) each title mentions the actual subject, (2) bullets are grounded in the order/source, (3) no preselected agenda, (4) requested page count is respected.',
  ].join('\n');
  const data = await askAi(prompt, 2800);
  if (!Array.isArray(data?.slides) || data.slides.length === 0) throw new Error('Invalid deck');
  return compileBlueprint({ title: data.title || state.title, slides: data.slides }, state);
}

export async function applySlideInstructionWithAi(state, action, instruction) {
  const current = state.slides.find((slide) => slide.id === state.activeSlideId) || state.slides[0];
  if (!current) return null;
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    'Return one slide using the same editable JSON format as the current slide.',
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `Action: ${action}.`,
    `User instruction: ${instruction || ''}.`,
    `Deck brief: ${JSON.stringify(buildBriefFromInputs(state))}.`,
    `Current slide: ${JSON.stringify(current)}.`,
    'Preserve the core message, but improve content, layout, hierarchy, speaker notes, and visual clarity.',
    'Maintain a slide role, claim, proofObject, supportNote, and sourceNote. Strengthen the proof object before adding more text.',
  ].join('\n');
  const data = await askAi(prompt, 1800);
  if (!data?.elements?.length) throw new Error('Invalid slide');
  return normalizeSlide({ ...current, ...data, id: current.id }, state.slides.indexOf(current), state);
}

export async function applyDeckInstructionWithAi(state, instruction) {
  const schema = {
    title: 'Deck title',
    slides: [
      {
        role: 'cover|content|data|transition|closing',
        narrativeStage: 'hook|context|core|shift|takeaway',
        title: 'Source-specific slide title',
        kicker: '1-3 word slide role',
        claim: 'One concrete idea this slide communicates',
        proofObject: 'source-backed proof or visual direction for this page',
        supportNote: 'What source fact or assumption supports this slide',
        sourceNote: 'Source or verification note',
        facts: ['Source-backed fact or clearly marked assumption, using source vocabulary'],
        bullets: ['Short visible text'],
        metric: { value: 'Only if explicitly present in source', label: 'Metric label' },
        chartData: [{ label: 'Only source-backed label', value: 0 }],
        notes: 'Speaker notes',
        layout: 'cover|brief|evidence|process|comparison|quote|data|closing',
      },
    ],
  };
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    `Shape: ${JSON.stringify(schema)}.`,
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `User revision request: ${instruction || ''}.`,
    `Deck brief: ${JSON.stringify(buildBriefFromInputs(state))}.`,
    `Current editable deck: ${JSON.stringify({ title: state.title, outline: state.outline, slides: state.slides })}.`,
    'Act as an end-to-end presentation agent, not a single-slide editor.',
    'Revise the whole deck as a coherent presentation while staying loyal to the user order and source material.',
    'You may generate a complete new deck, add slides, delete slides, reorder slides, merge duplicate slides, or rewrite existing slides when the user request calls for it.',
    'Preserve source constraints and never invent precise facts. Do not introduce a generic content formula unless the source or user asks for it.',
    'Keep the same approximate slide count only when the user asks for a style/content rewrite without structural change.',
    'Make every slide title source-specific and every slide revolve around one core message.',
  ].join('\n');
  const data = await askAi(prompt, 3200);
  if (!Array.isArray(data?.slides) || data.slides.length === 0) throw new Error('Invalid deck revision');
  return compileBlueprint({ title: data.title || state.title, slides: data.slides }, state, { respectSlideTarget: false });
}

export async function insertSlideWithAi(state, instruction) {
  const index = Math.min(state.slides.length, Math.max(0, state.slides.findIndex((slide) => slide.id === state.activeSlideId) + 1));
  const prompt = [
    'Return strict JSON only, no markdown fences.',
    'Return one slide using the same editable JSON format as the surrounding slides.',
    `Locale: ${getLocale()}.`,
    PPT_DESIGN_SKILL_CONTEXT,
    `Insertion request: ${instruction || ''}.`,
    `Deck brief: ${JSON.stringify(buildBriefFromInputs(state))}.`,
    `Insert after slide index: ${index}.`,
    `Deck outline: ${JSON.stringify(state.outline)}.`,
    `Previous slide: ${JSON.stringify(state.slides[index - 1] || null)}.`,
    `Next slide: ${JSON.stringify(state.slides[index] || null)}.`,
    'The inserted page must advance the story, not duplicate neighboring pages.',
    'Maintain a slide role, claim, proofObject, supportNote, sourceNote, speaker notes, and editable elements.',
  ].join('\n');
  const data = await askAi(prompt, 1800);
  if (!data?.elements?.length) throw new Error('Invalid inserted slide');
  return normalizeSlide(data, index, { ...state, slides: [...state.slides, data] });
}

export function localOutline(state) {
  const topic = displayTopic(state.brief.topic || t('defaultDeckTitle'));
  const facts = sourceFallbackFacts(state).filter(Boolean);
  const base = [
    topic,
    `${topic} 是什么，以及为什么值得关注`,
    facts[0] || `${topic} 的核心能力来自已有素材`,
    facts[1] || `${topic} 的工作方式需要用一个流程讲清楚`,
    facts[2] || `${topic} 的典型场景决定它的价值`,
    `${topic} 的证据和待验证问题`,
    `${topic} 适合谁，以及不适合谁`,
    `${topic} 的最终落点`,
  ];
  return (state.brief.slideTarget > 0 ? base.slice(0, state.brief.slideTarget) : base).map(cleanTitle);
}

export function localDeck(state) {
  const outline = state.outline?.length ? state.outline : localOutline(state);
  const next = ensureState({ ...clone(state), outline });
  const blueprint = localBlueprint(next, outline);
  const compiled = compileBlueprint(blueprint, next);
  next.slides = compiled.slides;
  next.title = compiled.title;
  next.activeSlideId = next.slides[0]?.id || '';
  next.selectedElementId = next.slides[0]?.elements[0]?.id || '';
  return next;
}

export function compileBlueprint(blueprint, state, options = {}) {
  const sourceCount = state.sources?.items?.length || 0;
  const requestedSlides = blueprint.slides || [];
  const fromAgentPayload = options.fromAgentPayload === true;
  const deckDesign = resolveDeckDesign(blueprint, state);
  const hasExplicitTarget = Number(state.brief.slideTarget) > 0 && options.respectSlideTarget !== false;
  const slideSource = hasExplicitTarget
    ? requestedSlides.slice(0, state.brief.slideTarget)
    : requestedSlides;
  const slides = slideSource.map((item, index, all) => {
    const safeItem = normalizeBlueprintDataIntegrity(item, state);
    const role = safeItem.role || roleForIndex(index, all.length);
    const layout = safeItem.layout || layoutForRole(role, index, all.length);
    const visualTreatment = normalizeVisualTreatment(safeItem.visualTreatment || safeItem.visual || safeItem.designIntent || layout, role, index, all.length);
    const slide = {
      id: uid('slide'),
      title: cleanTitle(safeItem.title || safeItem.claim || state.outline[index] || t('newSlideTitle')),
      subtitle: '',
      kicker: displayKicker(safeItem.kicker || role),
      claim: safeItem.claim || safeItem.title || '',
      proofObject: safeItem.proofObject || proofForRole(role, sourceCount),
      supportNote: safeItem.supportNote || supportForBlueprint(safeItem, state, fromAgentPayload),
      sourceNote: safeItem.sourceNote || sourceNoteForBlueprint(state),
      notes: safeItem.notes || t('defaultSpeakerNote', { title: safeItem.title || safeItem.claim || '' }),
      layout: `${layout}-${deckDesign.styleKey}-${visualTreatment}`,
      theme: themeFor(state, index, deckDesign, safeItem),
      elements: elementsForBlueprint(safeItem, role, index, all.length, state, fromAgentPayload, deckDesign, visualTreatment),
    };
    slide.quality = evaluateSlideQuality(slide, state, index);
    const repairedSlide = repairSlideLayout(slide, state, index, role, all.length, deckDesign);
    repairedSlide.quality = evaluateSlideQuality(repairedSlide, state, index);
    return normalizeSlide(repairedSlide, index, state);
  });
  return {
    title: cleanTitle(blueprint.title || state.brief.topic || slides[0]?.title || state.title),
    slides,
  };
}

function clampSlideCount(value) {
  return Math.max(1, Math.min(24, Number(value) || 1));
}

function resolveDeckDesign(blueprint, state) {
  // Priority 1: explicit style preset from user selection
  const presetKey = state.style?.stylePreset;
  if (presetKey && STYLE_PRESETS[presetKey]) {
    const preset = STYLE_PRESETS[presetKey];
    return {
      styleKey: preset.styleKey,
      palette: preset.palette || {},
      principles: ensureArray(blueprint.design?.layoutPrinciples).map(String),
    };
  }

  // Priority 2: AI-detected style from blueprint
  const raw = [
    blueprint.design?.stylePhilosophy,
    blueprint.design?.style,
    blueprint.stylePhilosophy,
    blueprint.style,
    state.style?.theme,
    state.brief?.topic,
  ].filter(Boolean).join(' ').toLowerCase();
  let styleKey = 'pentagram';
  if (/m[üu]ller|brockmann|grid|swiss|academic|技术|学术|严谨/.test(raw)) styleKey = 'muller';
  else if (/\bbuild\b|minimal|luxury|premium|高端|极简|品牌|宣言/.test(raw)) styleKey = 'build';
  else if (/kenya|hara|white|space|东方|留白|文化|艺术/.test(raw)) styleKey = 'hara';
  else if (/takram|soft|tech|research|柔和|科技人文|设计研究/.test(raw)) styleKey = 'takram';

  const palette = blueprint.design?.palette || blueprint.palette || {};
  return {
    styleKey,
    palette,
    principles: ensureArray(blueprint.design?.layoutPrinciples).map(String),
  };
}

function normalizeBlueprintDataIntegrity(item, state) {
  const next = { ...(item || {}) };
  if (Array.isArray(next.chartData) && !hasGroundedChartData(next, state)) {
    delete next.chartData;
    if (next.layout === 'data' && !hasSourceNumbers(state)) next.layout = 'evidence';
    next.supportNote = next.supportNote || t('bpSupportMissing');
    next.sourceNote = next.sourceNote || sourceNoteForBlueprint(state);
    next.dataIntegrityWarning = 'chart_data_removed_without_source_numbers';
  }
  if (next.metric?.value && !hasMetricSource(next.metric.value, state, next)) {
    next.metric = null;
    next.dataIntegrityWarning = next.dataIntegrityWarning || 'metric_removed_without_source';
  }
  return next;
}

function hasGroundedChartData(item, state) {
  const data = ensureArray(item.chartData);
  if (data.length < 2) return false;
  if (!hasSourceNumbers(state)) return false;
  const sourceText = sourceEvidenceText(state, item);
  return data.every((point) => sourceText.includes(String(point?.label || '').trim()) || sourceText.includes(String(point?.value ?? '').trim()));
}

function hasMetricSource(value, state, item = {}) {
  if (!/\d/.test(String(value || ''))) return true;
  return sourceEvidenceText(state, item).includes(String(value).trim());
}

function sourceEvidenceText(state, item = {}) {
  return [
    ...(state.sources?.facts || []),
    ...(state.sources?.items || []).map((source) => source.text || source.title || ''),
    ...(ensureArray(item.facts)),
    item.supportNote || '',
  ].join('\n');
}

function normalizeVisualTreatment(value, role, index, total) {
  const raw = String(value || '').toLowerCase();
  if (/process|workflow|timeline|flow|步骤|流程/.test(raw) || role === 'workflow') return 'process';
  if (/compare|versus|matrix|before|after|对比|比较/.test(raw) || role === 'comparison') return 'comparison';
  if (/data|chart|metric|number|数据|指标/.test(raw) || role === 'data') return 'data';
  if (/quote|transition|statement|宣言|引用/.test(raw) || role === 'transition') return 'editorial';
  if (/white|space|quiet|留白/.test(raw)) return 'white-space';
  if (/soft|tech|system|柔和|科技/.test(raw)) return 'soft-tech';
  if (index === 0 || index === total - 1) return 'typographic';
  return ['grid', 'editorial', 'data', 'process', 'comparison'][index % 5];
}

function displayKicker(value) {
  const raw = String(value || '').replace(/[-_]/g, ' ').trim();
  const normalized = raw.toLowerCase();
  if (getLocale().startsWith('zh')) {
    const zh = {
      cover: '开场',
      content: '核心',
      data: '数据',
      transition: '转场',
      closing: '落点',
      hook: '开场',
      context: '背景',
      finding: '发现',
      takeaway: '结论',
    };
    return zh[normalized] || raw;
  }
  return raw.toUpperCase();
}

function evaluateSlideQuality(slide, state, index) {
  const issues = [];
  const elements = ensureArray(slide.elements);
  elements.forEach((element) => {
    if (Number(element.x) + Number(element.w) > 100 || Number(element.y) + Number(element.h) > 100) {
      issues.push(qualityIssue('high', 'bounds', t('qualityOutOfBounds')));
    }
    if ((element.type === 'text' || element.type === 'list') && estimateTextLoad(element) > estimateTextCapacity(element)) {
      issues.push(qualityIssue('medium', 'text_density', t('qualityTextDense')));
    }
    if (element.type === 'chart' && !hasGroundedChartElement(element, state, slide)) {
      issues.push(qualityIssue('high', 'chart_source', t('qualityChartUngrounded')));
    }
  });
  for (let a = 0; a < elements.length; a += 1) {
    for (let b = a + 1; b < elements.length; b += 1) {
      if (elementsOverlap(elements[a], elements[b])) {
        issues.push(qualityIssue('medium', 'overlap', t('qualityOverlap')));
        a = elements.length;
        break;
      }
    }
  }
  if (index > 0 && !cleanTitle(slide.claim || slide.title)) {
    issues.push(qualityIssue('low', 'claim', t('qualityMissingClaim')));
  }
  const penalty = issues.reduce((sum, issue) => sum + (issue.severity === 'high' ? 30 : issue.severity === 'medium' ? 15 : 6), 0);
  return { score: Math.max(0, 100 - penalty), issues: issues.slice(0, 8) };
}

function qualityIssue(severity, type, message) {
  return { id: uid('quality'), severity, type, message };
}

function repairSlideLayout(slide, state, index, role, total, deckDesign) {
  const highRisk = ensureArray(slide.quality?.issues).some((issue) => issue.type === 'bounds' || issue.type === 'overlap' || issue.type === 'text_density');
  if (!highRisk) return slide;
  const item = {
    title: slide.title,
    claim: slide.claim,
    kicker: slide.kicker,
    role,
    layout: role === 'cover' ? 'cover' : role === 'closing' ? 'closing' : 'evidence',
    proofObject: slide.proofObject,
    supportNote: slide.supportNote,
    sourceNote: slide.sourceNote,
    facts: [slide.supportNote, slide.sourceNote].filter(Boolean),
    bullets: compactElementLines(slide.elements),
  };
  const repairedElements = benchmarkElements(
    item,
    role,
    index,
    total,
    slide.title,
    slide.claim || slide.title,
    item.facts,
    item.bullets,
    index === 0 ? 'cover' : index === total - 1 ? 'closing' : 'cards',
    deckDesign.styleKey,
  );
  return { ...slide, elements: repairedElements.length ? repairedElements : slide.elements };
}

function compactElementLines(elements) {
  return ensureArray(elements)
    .flatMap((element) => element.type === 'list' ? ensureArray(element.items) : [element.text, element.label])
    .map((item) => cleanTitle(item))
    .filter(Boolean)
    .slice(0, 4);
}

function estimateTextLoad(element) {
  return String(element.text || '').length + ensureArray(element.items).join('').length;
}

function estimateTextCapacity(element) {
  const fontSize = Number(element.style?.fontSize || 18);
  return Math.max(24, (Number(element.w || 1) * Number(element.h || 1) * 2.4) / Math.max(0.7, fontSize / 18));
}

function elementsOverlap(a, b) {
  if (a.type === 'shape' || b.type === 'shape') return false;
  const ax2 = Number(a.x) + Number(a.w);
  const ay2 = Number(a.y) + Number(a.h);
  const bx2 = Number(b.x) + Number(b.w);
  const by2 = Number(b.y) + Number(b.h);
  const overlapX = Math.max(0, Math.min(ax2, bx2) - Math.max(Number(a.x), Number(b.x)));
  const overlapY = Math.max(0, Math.min(ay2, by2) - Math.max(Number(a.y), Number(b.y)));
  return overlapX > 2 && overlapY > 2;
}

function hasGroundedChartElement(element, state, slide) {
  if (!ensureArray(element.data).length) return false;
  return hasGroundedChartData({ chartData: element.data, facts: [slide.supportNote, slide.sourceNote] }, state);
}

function normalizeAgentPlan(value, state) {
  const allowed = new Set(['generate_deck', 'revise_deck', 'revise_slide', 'insert_slide', 'delete_slide', 'update_outline']);
  const operation = allowed.has(value?.operation) ? value.operation : (state.slides?.length ? 'revise_deck' : 'generate_deck');
  const slideCount = state.slides?.length || 0;
  const slideIndex = value?.slideIndex === null || value?.slideIndex === undefined
    ? null
    : Math.max(0, Math.min(slideCount - 1, Number(value.slideIndex) || 0));
  const scope = ['deck', 'current_slide', 'slide_index'].includes(value?.scope) ? value.scope : (operation === 'revise_slide' ? 'current_slide' : 'deck');
  return {
    operation,
    scope,
    slideIndex,
    briefPatch: normalizeBriefPatch(value?.briefPatch),
    needsSources: Boolean(value?.needsSources),
    reason: String(value?.reason || ''),
    steps: Array.isArray(value?.steps) ? value.steps.slice(0, 8).map(normalizePlanStep) : [],
    acceptanceCriteria: Array.isArray(value?.acceptanceCriteria) ? value.acceptanceCriteria.map(String).slice(0, 6) : [],
  };
}

function normalizePlanStep(step) {
  return {
    agent: String(step?.agent || step?.stage || 'brief'),
    task: String(step?.task || ''),
    deliverable: String(step?.deliverable || ''),
  };
}

function normalizeBriefPatch(value = {}) {
  const patch = {};
  const topic = cleanPatchText(value.topic);
  const audience = cleanPatchText(value.audience);
  const intent = cleanPatchText(value.intent);
  const tone = cleanPatchText(value.tone);
  if (topic) patch.topic = topic;
  if (audience) patch.audience = audience;
  if (intent) patch.deckType = intent;
  if (tone) patch.tone = tone;
  const slideTarget = Number(value.slideTarget);
  if (Number.isFinite(slideTarget) && slideTarget > 0) patch.slideTarget = Math.max(3, Math.min(24, slideTarget));
  return patch;
}

function cleanPatchText(value) {
  const text = typeof value === 'string' ? value.trim() : '';
  if (!text || /^optional\b/i.test(text)) return '';
  return text;
}

function localBlueprint(state, outline) {
  const topic = displayTopic(state.brief.topic || outline[0] || t('defaultDeckTitle'));
  const facts = state.sources?.facts?.length ? state.sources.facts : sourceFallbackFacts(state);
  const hasSource = hasGroundedSource(state);
  const roles = ['cover', 'content', 'content', 'content', 'content', 'data', 'transition', 'closing'];
  const titles = [
    topic,
    hasSource ? `${topic} 的源材料提供了第一组线索` : `${topic} 需要先补充可验证材料`,
    facts[0] || `${topic} 的定义和边界需要讲清楚`,
    facts[1] || `${topic} 的结构可以拆成几个关键模块`,
    facts[2] || `${topic} 的使用流程决定理解速度`,
    facts[3] || `${topic} 需要用例来证明价值`,
    facts[4] || `${topic} 需要把已知事实和未知问题分开呈现`,
    `${topic} 的结尾要留下一个清晰记忆点`,
  ];
  const slideCount = state.brief.slideTarget > 0 ? state.brief.slideTarget : roles.length;
  return {
    title: topic,
    slides: roles.slice(0, slideCount).map((role, index) => ({
      role,
      title: titles[index] || outline[index] || t('newSlideTitle'),
      kicker: role,
      claim: titles[index] || outline[index] || '',
      proofObject: proofForRole(role, hasSource ? 1 : 0),
      supportNote: hasSource ? t('bpSupportSource') : t('bpSupportMissing'),
      sourceNote: sourceNoteForBlueprint(state),
      facts: rotateFacts(facts, index, 3),
      bullets: rotateFacts(facts, index + 1, 3),
      notes: t('defaultSpeakerNote', { title: titles[index] || topic }),
      layout: layoutForRole(role, index, roles.length),
    })),
  };
}

export function localSlideUpdate(state, action, instruction) {
  const slide = clone(state.slides.find((item) => item.id === state.activeSlideId) || state.slides[0]);
  if (!slide) return null;
  const titleElement = slide.elements.find((element) => element.type === 'text');
  const listElement = slide.elements.find((element) => element.type === 'list');
  if (action === 'condense' && listElement) {
    listElement.items = listElement.items.slice(0, 2).map((item) => item.replace(/\s+and\s+/i, ' / '));
  } else if (action === 'professional' && titleElement) {
    titleElement.text = titleElement.text.replace(/\bmake\b/gi, 'deliver').replace(/\buse\b/gi, 'apply');
  } else if (action === 'notes') {
    slide.notes = `Takeaway: ${slide.claim || slide.title}\nProof: Walk through the ${slide.proofObject || 'dominant proof object'} and call out the strongest evidence.\nDecision: Close by naming the owner, timing, or next action. ${instruction || ''}`.trim();
  } else if (action === 'visual' || action === 'redesign') {
    const index = state.slides.findIndex((item) => item.id === slide.id);
    const replacement = makeSlide(slide.title, index + 1, state.slides.length + 1, state);
    replacement.id = slide.id;
    replacement.notes = slide.notes;
    return replacement;
  } else if (titleElement) {
    titleElement.text = instruction || titleElement.text;
  }
  return normalizeSlide(slide, state.slides.findIndex((item) => item.id === slide.id), state);
}

export function localDeckUpdate(state, instruction) {
  const next = clone(state);
  const suffix = instruction ? ` ${instruction}` : '';
  next.slides = next.slides.map((slide, index) => normalizeSlide({
    ...slide,
    supportNote: `${slide.supportNote || ''}${suffix}`.trim(),
    notes: `${slide.notes || ''}\nRevision request: ${instruction || 'Improve clarity and flow.'}`.trim(),
  }, index, next));
  next.outline = next.slides.map((slide) => slide.title);
  return { title: next.title, slides: next.slides };
}

export function localInsertedSlide(state, instruction) {
  const index = Math.min(state.slides.length, Math.max(0, state.slides.findIndex((slide) => slide.id === state.activeSlideId) + 1));
  const title = instruction || t('newSlideTitle');
  return makeSlide(title, index, state.slides.length + 1, state);
}

function agentSlideLines(item) {
  const facts = ensureArray(item.facts).map((line) => String(line).trim()).filter(Boolean);
  const bullets = ensureArray(item.bullets).map((line) => String(line).trim()).filter(Boolean);
  return { facts, bullets };
}

function resolveSlideLines(item, state, fromAgentPayload) {
  const { facts: agentFacts, bullets: agentBullets } = agentSlideLines(item);
  const claim = cleanTitle(item.claim || item.title || '');
  const support = cleanTitle(item.supportNote || '');
  if (fromAgentPayload) {
    const facts = agentFacts.length ? agentFacts : agentBullets;
    const bullets = agentBullets.length ? agentBullets : facts;
    if (bullets.length || facts.length) {
      return { facts, bullets };
    }
    if (claim || support) {
      const line = claim || support;
      return { facts: [line], bullets: [line] };
    }
    return { facts: [], bullets: [] };
  }
  const trusted = hasGroundedSource(state);
  const facts = trusted ? ensureBullets(item.facts?.length ? item.facts : item.bullets, state) : sourceFallbackFacts(state);
  const bullets = trusted ? ensureBullets(item.bullets?.length ? item.bullets : facts, state) : sourceFallbackFacts(state);
  return { facts, bullets };
}

function elementsForBlueprint(item, role, index, total, state, fromAgentPayload = false, deckDesign = resolveDeckDesign({}, state), visualTreatment = 'grid') {
  const explicit = normalizeExplicitElements(item, deckDesign);
  if (explicit.length) return explicit;

  const title = cleanTitle(item.title || item.claim || state.outline[index] || t('newSlideTitle'));
  const claim = cleanTitle(item.claim || title);
  const { facts, bullets } = resolveSlideLines(item, state, fromAgentPayload);
  const styleKey = deckDesign.styleKey;
  const pattern = smartLayoutPattern(item, role, index, total, facts, bullets, visualTreatment);
  const benchmarkElements = benchmarkElements(item, role, index, total, title, claim, facts, bullets, pattern, styleKey);
  if (benchmarkElements.length) return benchmarkElements;

  if (styleKey === 'build') return buildElements(item, role, index, total, title, claim, facts, bullets, visualTreatment);
  if (styleKey === 'hara') return haraElements(item, role, index, total, title, claim, facts, bullets, visualTreatment);
  if (styleKey === 'muller') return mullerElements(item, role, index, total, title, claim, facts, bullets, visualTreatment);
  if (styleKey === 'takram') return takramElements(item, role, index, total, title, claim, facts, bullets, visualTreatment);
  return pentagramElements(item, role, index, total, title, claim, facts, bullets, visualTreatment);
}

function smartLayoutPattern(item, role, index, total, facts, bullets, visualTreatment) {
  const raw = [role, item.layout, item.visualTreatment, item.visual, item.designIntent, item.proofObject, visualTreatment].filter(Boolean).join(' ').toLowerCase();
  if (role === 'cover' || index === 0) return 'cover';
  if (role === 'closing' || role === 'takeaway' || role === 'decision' || index === total - 1) return 'closing';
  if (/quote|transition|statement|宣言|引用/.test(raw)) return 'quote';
  if (/process|workflow|timeline|roadmap|journey|steps|architecture|flow|流程|步骤|路线|架构/.test(raw)) return 'process';
  if (/compare|versus|matrix|before|after|tradeoff|对比|比较|矩阵/.test(raw)) return 'comparison';
  if (/data|chart|metric|number|kpi|scorecard|数据|指标/.test(raw) || ensureArray(item.chartData).length >= 2 || facts.some((fact) => /\d/.test(String(fact)))) return 'data';
  if (ensureArray(bullets).length >= 3) return 'cards';
  return index % 3 === 1 ? 'split' : 'spotlight';
}

function benchmarkElements(item, role, index, total, title, claim, facts, bullets, pattern, styleKey) {
  const titleSize = displayTitleSize(title, pattern, styleKey);
  const body = compactLines(bullets, pattern === 'process' || pattern === 'comparison' ? 4 : 3);
  const proof = facts[0] || item.supportNote || body[0] || claim;
  if (pattern === 'cover') {
    return [
      shape(6, 9, 88, 76, 'soft', 1, styleKey === 'muller' ? 0 : 28),
      shape(9, 15, 1.2, 55, 'primary', 1, 99),
      text(displayKicker(item.kicker || 'deck'), 13, 15, 22, 5, 10, 760, 'primary'),
      text(title, 13, 23, 58, 25, titleSize, 840, 'ink'),
      text(claim, 14, 55, 45, 11, 18, 520, 'muted'),
      metric(String(total), t('slidesUnit'), 75, 54, 14, 17, 34),
    ];
  }
  if (pattern === 'closing') {
    return [
      text(title, 9, 15, 65, 15, titleSize, 820, 'ink'),
      text(claim, 10, 33, 46, 9, 17, 540, 'muted'),
      ...cardGrid([t('closeConfirm'), t('closeOwner'), t('closeIteration')], 10, 50, 52, 22, 3, 18),
      text(proof, 67, 48, 22, 20, 18, 720, 'primary', 'soft', 20),
    ];
  }
  if (pattern === 'quote') {
    return [
      shape(11, 16, 10, 0.6, 'primary', 1, 99),
      text(title, 13, 22, 64, 23, titleSize, 780, 'ink'),
      text(body[0] || claim, 16, 53, 48, 11, 18, 520, 'muted'),
      text(item.sourceNote || '', 70, 72, 18, 5, 10, 500, 'muted'),
    ];
  }
  if (pattern === 'process') {
    return [
      text(title, 8, 10, 68, 12, titleSize, 820, 'ink'),
      text(claim, 9, 25, 54, 7, 15, 520, 'muted'),
      shape(10, 50, 78, 1.2, 'primary', 0.25, 99),
      ...cardGrid(body.slice(0, 4), 10, 39, 78, 25, Math.min(4, body.length || 1), 16).map((element, pointIndex) => ({ ...element, text: `0${pointIndex + 1}  ${element.text}` })),
    ];
  }
  if (pattern === 'comparison') {
    const columns = body.length >= 4 ? body.slice(0, 4) : [body[0] || claim, body[1] || proof, body[2] || item.supportNote || '', body[3] || item.sourceNote || ''];
    return [
      text(title, 7, 10, 72, 12, titleSize, 820, 'ink'),
      text(claim, 8, 25, 48, 7, 15, 520, 'muted'),
      ...cardGrid(columns, 8, 39, 82, 28, 2, 18),
    ];
  }
  if (pattern === 'data') {
    const safeChartData = ensureArray(item.chartData).length >= 2 ? item.chartData : factsToChartData(facts);
    const hasChart = safeChartData.length >= 2;
    return [
      text(title, 8, 10, 66, 12, titleSize, 820, 'ink'),
      text(claim, 9, 25, 47, 7, 15, 520, 'muted'),
      hasChart
        ? chart(item.proofObject || t('proofTrendChart'), safeChartData, 9, 39, 55, 31)
        : metric(item.metric?.value || facts.find((fact) => /\d/.test(fact)) || String(index).padStart(2, '0'), item.metric?.label || item.proofObject || claim, 10, 40, 34, 28, 44),
      text(proof, 69, 41, 20, 24, 17, 700, 'primary', 'soft', 18),
    ];
  }
  if (pattern === 'cards') {
    const cards = body.length ? body : [claim];
    return [
      text(title, 8, 10, 68, 12, titleSize, 820, 'ink'),
      text(claim, 9, 25, 51, 8, 15, 520, 'muted'),
      ...cardGrid(cards, 9, 42, 78, 25, Math.min(3, cards.length), 17),
    ];
  }
  if (pattern === 'spotlight') {
    return [
      text(title, 10, 15, 62, 15, titleSize, 820, 'ink'),
      text(claim, 11, 34, 42, 10, 17, 520, 'muted'),
      text(proof, 58, 38, 28, 24, 22, 760, 'primary', 'soft', 22),
      shape(10, 72, 18, 0.6, 'primary', 1, 99),
    ];
  }
  return [
    text(title, 8, 10, 62, 12, titleSize, 820, 'ink'),
    text(claim, 9, 26, 40, 9, 16, 520, 'muted'),
    list(body, 9, 44, 36, 25, 17),
    text(proof, 55, 39, 33, 27, 19, 720, 'primary', 'soft', 18),
  ];
}

function cardGrid(items, x, y, w, h, columns, fontSize) {
  const safeItems = ensureArray(items).filter(Boolean);
  const safeColumns = Math.max(1, Math.min(columns || 1, safeItems.length || 1));
  const gap = 2.5;
  const rows = Math.max(1, Math.ceil((safeItems.length || 1) / safeColumns));
  const cardW = (w - gap * (safeColumns - 1)) / safeColumns;
  const cardH = (h - gap * (rows - 1)) / rows;
  return safeItems.map((item, itemIndex) => text(item, x + (itemIndex % safeColumns) * (cardW + gap), y + Math.floor(itemIndex / safeColumns) * (cardH + gap), cardW, cardH, fontSize, itemIndex === 0 ? 760 : 620, itemIndex === 0 ? 'primary' : 'ink', itemIndex === 0 ? 'soft' : 'panel', 18));
}

function compactLines(items, maxCount) {
  return ensureArray(items)
    .map((item) => cleanTitle(item))
    .filter(Boolean)
    .slice(0, maxCount);
}

function displayTitleSize(title, pattern, styleKey) {
  const length = String(title || '').length;
  const base = pattern === 'cover' ? 44 : pattern === 'quote' || pattern === 'spotlight' ? 38 : 32;
  const styleOffset = styleKey === 'build' ? 4 : styleKey === 'hara' ? -2 : 0;
  if (length > 68) return Math.max(24, base - 10 + styleOffset);
  if (length > 48) return Math.max(26, base - 6 + styleOffset);
  return base + styleOffset;
}

function pentagramElements(item, role, index, total, title, claim, facts, bullets, visualTreatment) {
  if (role === 'cover' || index === 0) {
    const coverTitleSize = title.length > 58 ? 25 : title.length > 42 ? 29 : 40;
    return [
      shape(7, 15, 4, 56, 'primary', 1, 99),
      text(title, 14, 18, 62, 28, coverTitleSize, 840, 'ink'),
      text(claim, 15, 50, 56, 12, 18, 540, 'muted'),
      metric(String(total), t('slidesUnit'), 75, 53, 15, 18),
    ];
  }
  if (visualTreatment === 'process' || role === 'workflow' || item.layout === 'process') {
    return [
      text(title, 8, 13, 68, 13, 31, 810, 'ink'),
      text(claim, 9, 29, 54, 9, 15, 540, 'muted'),
      shape(10, 51, 78, 2, 'primary', 0.18, 99),
      ...bullets.slice(0, 3).map((point, pointIndex) => metric(`0${pointIndex + 1}`, point, 10 + pointIndex * 27, 39, 22, 27, 28)),
    ];
  }
  if (visualTreatment === 'data' && Array.isArray(item.chartData) && item.chartData.length >= 2) {
    return [
      text(title, 8, 12, 68, 13, 31, 810, 'ink'),
      chart(item.proofObject || t('proofTrendChart'), item.chartData, 10, 34, 54, 36),
      text(item.supportNote || bullets[0], 69, 38, 20, 25, 17, 700, 'primary', 'soft', 14),
    ];
  }
  if (visualTreatment === 'comparison') {
    return [
      text(title, 7, 11, 70, 12, 30, 820, 'ink'),
      text(bullets[0] || claim, 9, 35, 35, 28, 22, 760, 'ink', 'panel', 8),
      text(bullets[1] || facts[0] || item.supportNote || '', 52, 35, 35, 28, 22, 760, 'primary', 'soft', 8),
      shape(48, 30, 1, 44, 'primary', 1, 99),
    ];
  }
  if (role === 'risk' || item.layout === 'risk') {
    return [
      text(title, 8, 12, 68, 13, 31, 810, 'ink'),
      list(bullets.slice(0, 4), 9, 34, 47, 38, 19, 'panel'),
      text(item.supportNote || t('bpSupportMissing'), 61, 36, 28, 30, 17, 650, 'muted', 'soft', 14),
    ];
  }
  if (role === 'decision' || index === total - 1) {
    return [
      text(title, 9, 15, 70, 15, 38, 820, 'ink'),
      list([t('closeConfirm'), t('closeOwner'), t('closeIteration')], 12, 42, 45, 32, 22),
      text(bullets[0] || item.supportNote || '', 63, 42, 27, 24, 18, 720, 'primary', 'soft', 16),
    ];
  }
  return [
    text(title, 8, 12, 68, 13, 31, 810, 'ink'),
    text(claim, 9, 29, 52, 9, 15, 540, 'muted'),
    list(bullets.slice(0, 3), 9, 42, 39, 31, 18),
    text(facts[0] || item.supportNote || '', 55, 38, 34, 29, 18, 700, 'primary', 'soft', 14),
  ];
}

function mullerElements(item, role, index, total, title, claim, facts, bullets, visualTreatment) {
  if (index === 0) {
    return [
      text(String(total).padStart(2, '0'), 8, 10, 10, 10, 34, 820, 'primary'),
      shape(8, 23, 82, 1, 'ink', 1, 0),
      text(title, 8, 31, 72, 20, title.length > 48 ? 30 : 40, 760, 'ink'),
      text(claim, 8, 59, 50, 10, 17, 500, 'muted'),
      text(item.sourceNote || '', 67, 76, 22, 6, 11, 500, 'muted'),
    ];
  }
  if (visualTreatment === 'process') {
    return [
      text(`0${index}`, 8, 10, 8, 8, 24, 780, 'primary'),
      text(title, 20, 10, 62, 12, 28, 760, 'ink'),
      shape(8, 27, 82, 1, 'ink', 1, 0),
      ...bullets.slice(0, 4).map((point, pointIndex) => text(point, 10 + pointIndex * 21, 44, 17, 18, 17, 620, pointIndex === 0 ? 'primary' : 'ink')),
    ];
  }
  if (visualTreatment === 'data') {
    const safeChartData = item.chartData?.length ? item.chartData : factsToChartData(facts);
    return [
      text(`0${index}`, 8, 10, 8, 8, 24, 780, 'primary'),
      text(title, 20, 10, 62, 12, 28, 760, 'ink'),
      safeChartData.length >= 2
        ? chart(item.proofObject || t('proofTrendChart'), safeChartData, 15, 35, 62, 34)
        : text(item.supportNote || facts[0] || bullets[0] || claim, 20, 38, 58, 24, 21, 650, 'primary', 'soft', 8),
    ];
  }
  return [
    text(`0${index}`, 8, 10, 8, 8, 24, 780, 'primary'),
    text(title, 20, 10, 62, 12, 28, 760, 'ink'),
    shape(8, 27, 82, 1, 'ink', 1, 0),
    text(claim, 20, 36, 36, 18, 21, 650, 'ink'),
    list(bullets.slice(0, 3), 61, 36, 25, 30, 15, 'transparent'),
  ];
}

function buildElements(item, role, index, total, title, claim, facts, bullets, visualTreatment) {
  if (index === 0) {
    return [
      text(title, 9, 20, 72, 28, title.length > 46 ? 34 : 48, 500, 'ink'),
      text(claim, 10, 62, 42, 10, 17, 420, 'muted'),
      text(String(total), 83, 74, 8, 8, 20, 500, 'primary'),
    ];
  }
  if (visualTreatment === 'data') {
    return [
      text(title, 9, 13, 72, 14, 32, 500, 'ink'),
      metric(item.metric?.value || facts.find((fact) => /\d/.test(fact)) || `${index}`, item.metric?.label || item.proofObject || claim, 10, 42, 34, 26, 50),
      text(bullets[0] || item.supportNote || '', 52, 47, 33, 14, 18, 420, 'muted'),
    ];
  }
  if (visualTreatment === 'white-space' || visualTreatment === 'editorial') {
    return [
      text(title, 13, 24, 62, 24, title.length > 50 ? 30 : 42, 500, 'ink'),
      text(bullets[0] || claim, 14, 58, 36, 12, 18, 420, 'muted'),
    ];
  }
  return [
    text(title, 9, 14, 68, 16, 34, 500, 'ink'),
    text(claim, 10, 39, 48, 12, 19, 420, 'muted'),
    list(bullets.slice(0, 2), 10, 60, 48, 18, 17, 'transparent'),
    shape(76, 14, 1, 64, 'primary', 1, 0),
  ];
}

function haraElements(item, role, index, total, title, claim, facts, bullets, visualTreatment) {
  if (index === 0) {
    return [
      text(title, 16, 28, 60, 20, title.length > 50 ? 28 : 38, 430, 'ink'),
      text(claim, 17, 57, 34, 10, 15, 360, 'muted'),
      shape(16, 78, 12, 0.4, 'primary', 1, 0),
    ];
  }
  return [
    text(title, 16, 18, 56, 16, 30, 420, 'ink'),
    text(bullets[0] || claim, 18, 45, 38, 12, 17, 360, 'muted'),
    text(bullets[1] || facts[0] || item.supportNote || '', 62, 64, 22, 10, 13, 340, 'muted'),
    shape(16, 77, 10, 0.4, 'primary', 1, 0),
  ];
}

function takramElements(item, role, index, total, title, claim, facts, bullets, visualTreatment) {
  if (index === 0) {
    return [
      shape(62, 12, 26, 55, 'soft', 1, 28),
      text(title, 9, 17, 56, 22, title.length > 50 ? 30 : 40, 760, 'ink'),
      text(claim, 10, 48, 42, 12, 17, 500, 'muted'),
      metric(String(total), t('slidesUnit'), 68, 45, 16, 17, 36),
    ];
  }
  if (visualTreatment === 'process') {
    return [
      text(title, 8, 12, 62, 12, 30, 740, 'ink'),
      shape(10, 36, 78, 30, 'soft', 1, 24),
      ...bullets.slice(0, 3).map((point, pointIndex) => metric(`0${pointIndex + 1}`, point, 14 + pointIndex * 24, 41, 18, 18, 24)),
    ];
  }
  if (visualTreatment === 'data') {
    const safeChartData = item.chartData?.length ? item.chartData : factsToChartData(facts);
    return [
      text(title, 8, 12, 62, 12, 30, 740, 'ink'),
      safeChartData.length >= 2
        ? chart(item.proofObject || t('proofTrendChart'), safeChartData, 10, 34, 50, 34)
        : text(item.supportNote || facts[0] || bullets[0] || claim, 10, 38, 48, 24, 20, 650, 'primary', 'soft', 18),
      text(bullets[0] || claim, 65, 39, 22, 20, 17, 600, 'primary', 'soft', 18),
    ];
  }
  return [
    text(title, 8, 12, 64, 12, 30, 740, 'ink'),
    text(claim, 9, 30, 44, 10, 16, 520, 'muted'),
    list(bullets.slice(0, 3), 10, 49, 42, 24, 17, 'transparent'),
    text(facts[0] || item.supportNote || '', 60, 40, 28, 24, 17, 620, 'primary', 'soft', 18),
  ];
}

function normalizeExplicitElements(item, deckDesign) {
  const elements = Array.isArray(item.elements) ? item.elements : [];
  return elements
    .filter((element) => element && typeof element === 'object')
    .slice(0, 12)
    .map((element) => ({
      type: ['text', 'list', 'shape', 'metric', 'chart', 'media'].includes(element.type) ? element.type : 'text',
      text: String(element.text || ''),
      label: String(element.label || ''),
      items: ensureArray(element.items).map(String),
      data: ensureArray(element.data).map((point, pointIndex) => ({
        label: String(point?.label || `#${pointIndex + 1}`),
        value: Number(point?.value || 0),
      })),
      x: Number.isFinite(Number(element.x)) ? Number(element.x) : 8,
      y: Number.isFinite(Number(element.y)) ? Number(element.y) : 12,
      w: Number.isFinite(Number(element.w)) ? Number(element.w) : 60,
      h: Number.isFinite(Number(element.h)) ? Number(element.h) : 12,
      style: {
        fontSize: Number(element.style?.fontSize || element.fontSize || 24),
        fontWeight: Number(element.style?.fontWeight || element.fontWeight || 600),
        color: semanticColor(element.style?.color || element.color || 'ink'),
        background: semanticColor(element.style?.background || element.background || 'transparent'),
        borderRadius: Number(element.style?.borderRadius || element.borderRadius || 0),
        opacity: Number(element.style?.opacity ?? element.opacity ?? 1),
        align: element.style?.align || element.align || 'left',
      },
    }));
}

function semanticColor(value) {
  const raw = String(value || '').trim();
  if (!raw) return 'transparent';
  return raw;
}

function factsToChartData(facts) {
  const source = ensureArray(facts).filter((fact) => /\d/.test(String(fact))).slice(0, 4);
  return source.map((fact, index) => ({ label: fact.slice(0, 12), value: extractFirstNumber(fact) ?? 35 + index * 15 }));
}

function extractFirstNumber(value) {
  const match = String(value || '').match(/-?\d+(?:\.\d+)?/);
  if (!match) return null;
  const number = Number(match[0]);
  return Number.isFinite(number) ? number : null;
}

function text(value, x, y, w, h, fontSize, fontWeight, color, background = 'transparent', borderRadius = 0) {
  return {
    type: 'text',
    text: String(value || ''),
    label: '',
    items: [],
    data: [],
    x,
    y,
    w,
    h,
    style: { fontSize, fontWeight, color, background, borderRadius, opacity: 1, align: 'left' },
  };
}

function list(items, x, y, w, h, fontSize = 19, background = 'transparent') {
  return {
    type: 'list',
    text: '',
    label: '',
    items: ensureArray(items).slice(0, 5),
    data: [],
    x,
    y,
    w,
    h,
    style: { fontSize, fontWeight: 560, color: 'ink', background, borderRadius: background === 'panel' ? 14 : 0, opacity: 1, align: 'left' },
  };
}

function metric(value, label, x, y, w, h, fontSize = 40) {
  return {
    type: 'metric',
    text: String(value || ''),
    label: String(label || ''),
    items: [],
    data: [],
    x,
    y,
    w,
    h,
    style: { fontSize, fontWeight: 820, color: 'primary', background: 'panel', borderRadius: 14, opacity: 1, align: 'left' },
  };
}

function chart(title, data, x, y, w, h) {
  return {
    type: 'chart',
    text: String(title || t('proofTrendChart')),
    label: '',
    items: [],
    data: ensureArray(data).map((point, index) => ({
      label: String(point.label || `#${index + 1}`),
      value: Number(point.value || 0),
    })),
    x,
    y,
    w,
    h,
    style: { fontSize: 18, fontWeight: 700, color: 'ink', background: 'panel', borderRadius: 14, opacity: 1, align: 'left' },
  };
}

function shape(x, y, w, h, background, opacity, borderRadius) {
  return {
    type: 'shape',
    text: '',
    label: '',
    items: [],
    data: [],
    x,
    y,
    w,
    h,
    style: { fontSize: 18, fontWeight: 600, color: 'accent', background, borderRadius, opacity, align: 'center' },
  };
}

function themeFor(state, index, deckDesign = resolveDeckDesign({}, state), item = {}) {
  const preset = DESIGN_THEMES[deckDesign.styleKey] || DESIGN_THEMES.pentagram;
  const palette = { ...preset, ...(deckDesign.palette || {}), ...(item.palette || {}) };
  const primary = palette.primary || state.style?.brandPrimary || '#111111';
  const accent = palette.accent || state.style?.brandAccent || '#c84b31';
  return {
    background: palette.background,
    ink: palette.ink,
    muted: palette.muted,
    primary: index % 2 ? accent : primary,
    accent: index % 2 ? primary : accent,
    panel: palette.panel,
  };
}

const DESIGN_THEMES = {
  pentagram: {
    background: '#fafaf7',
    ink: '#1a1a1a',
    muted: '#5f5f5a',
    primary: '#111111',
    accent: '#c84b31',
    panel: '#ffffff',
  },
  muller: {
    background: '#f5f5f2',
    ink: '#111111',
    muted: '#616161',
    primary: '#e11d2e',
    accent: '#111111',
    panel: '#ffffff',
  },
  build: {
    background: '#f8f6f1',
    ink: '#15120d',
    muted: '#746d62',
    primary: '#8b6f47',
    accent: '#1f1a14',
    panel: '#fffdf8',
  },
  hara: {
    background: '#f7f5f0',
    ink: '#26231f',
    muted: '#8b867e',
    primary: '#b8a46f',
    accent: '#4b4640',
    panel: '#fbfaf6',
  },
  takram: {
    background: '#f6f4ee',
    ink: '#18202a',
    muted: '#68717d',
    primary: '#2f7f73',
    accent: '#d88c51',
    panel: '#fffdfa',
  },
  // === PPT Live Style Presets (kept in sync with style-presets.js) ===
  'clean-business': {
    background: '#ffffff',
    ink: '#1a1a1a',
    muted: '#6b7280',
    primary: '#2563eb',
    accent: '#3b82f6',
    panel: '#f8fafc',
  },
  'insight-report': {
    background: '#ffffff',
    ink: '#1f2937',
    muted: '#64748b',
    primary: '#1e3a8a',
    accent: '#dc2626',
    panel: '#f1f5f9',
  },
  'minimal-gallery': {
    background: '#fafafa',
    ink: '#171717',
    muted: '#737373',
    primary: '#171717',
    accent: '#525252',
    panel: '#ffffff',
  },
  'bold-editorial': {
    background: '#ffffff',
    ink: '#000000',
    muted: '#525252',
    primary: '#dc2626',
    accent: '#ef4444',
    panel: '#fafafa',
  },
  'yellow-magazine': {
    background: '#facc15',
    ink: '#171717',
    muted: '#525252',
    primary: '#171717',
    accent: '#ffffff',
    panel: '#fef08a',
  },
  'pink-pop': {
    background: '#fce7f3',
    ink: '#831843',
    muted: '#be185d',
    primary: '#db2777',
    accent: '#f472b6',
    panel: '#fdf2f8',
  },
  'creative-studio': {
    background: '#ffffff',
    ink: '#171717',
    muted: '#525252',
    primary: '#ea580c',
    accent: '#c2410c',
    panel: '#fafafa',
  },
  'retro-pop': {
    background: '#fef3c7',
    ink: '#451a03',
    muted: '#92400e',
    primary: '#dc2626',
    accent: '#f59e0b',
    panel: '#fffbeb',
  },
  'dark-neon': {
    background: '#0a0a0a',
    ink: '#e5e5e5',
    muted: '#737373',
    primary: '#22d3ee',
    accent: '#f472b6',
    panel: '#171717',
  },
  'pop-infographic': {
    background: '#ffffff',
    ink: '#171717',
    muted: '#525252',
    primary: '#ec4899',
    accent: '#06b6d4',
    panel: '#f0fdfa',
  },
};

function layoutForRole(role, index, total) {
  if (role === 'cover' || index === 0) return 'cover';
  if (role === 'closing' || role === 'takeaway' || index === total - 1) return 'closing';
  if (role === 'transition') return 'quote';
  if (role === 'workflow' || role === 'architecture') return 'process';
  if (role === 'comparison' || role === 'data') return 'comparison';
  if (role === 'content' || role === 'example' || role === 'finding' || role === 'context' || role === 'hook') return 'split';
  return index % 2 ? 'metric' : 'split';
}

function proofForRole(role, sourceCount) {
  const withSource = sourceCount > 0;
  const map = {
    cover: withSource ? t('proofSourceSummary') : t('proofVerificationPlan'),
    content: withSource ? t('proofEvidenceList') : t('proofVerificationPlan'),
    data: t('proofMetricBridge'),
    transition: t('proofVisualProof'),
    closing: t('proofDecisionTable'),
    hook: withSource ? t('proofSourceSummary') : t('proofVerificationPlan'),
    context: withSource ? t('proofSourceSummary') : t('proofVerificationPlan'),
    finding: withSource ? t('proofEvidenceList') : t('proofVerificationPlan'),
    architecture: t('proofProductDiagram'),
    example: t('proofWorkedExample'),
    comparison: t('proofComparison'),
    takeaway: t('proofDecisionTable'),
    problem: t('proofComparison'),
    solution: t('proofCapabilityMatrix'),
    workflow: t('proofOperatingModel'),
    proof: withSource ? t('proofEvidenceList') : t('proofVerificationPlan'),
    risk: t('proofRiskRegister'),
    decision: t('proofDecisionTable'),
  };
  return map[role] || t('proofVisualProof');
}

function roleForIndex(index, total) {
  if (index === 0) return 'cover';
  if (index === total - 1) return 'closing';
  return ['content', 'content', 'data', 'transition'][Math.max(0, index - 1) % 4];
}

function supportForBlueprint(item, state, fromAgentPayload = false) {
  const support = cleanTitle(item.supportNote || '');
  if (fromAgentPayload) {
    const { bullets, facts } = agentSlideLines(item);
    return support || bullets[0] || facts[0] || cleanTitle(item.claim || item.title || '');
  }
  if (!hasGroundedSource(state)) return t('bpSupportMissing');
  if (item.facts?.length) return item.facts[0];
  return state.sources?.items?.length ? t('bpSupportSource') : t('bpSupportMissing');
}

function sourceNoteForBlueprint(state) {
  const urls = state.sources?.items?.filter((item) => item.url).map((item) => item.url);
  if (urls?.length) return t('sourceFetchedNote', { count: urls.length });
  if (hasGroundedSource(state)) return t('sourceUserMaterial');
  return t('sourceDraftAssumption');
}

function hasGroundedSource(state) {
  return Boolean(state.sources?.facts?.length || state.sources?.items?.some((item) => String(item.text || '').length >= 120));
}

function hasSourceNumbers(state) {
  return Boolean(state.sources?.facts?.some((fact) => /\d/.test(String(fact))));
}

function sourceFallbackFacts(state) {
  return state.sources?.warnings?.length
    ? state.sources.warnings
    : [t('bpMissingFact1'), t('bpMissingFact2'), t('bpMissingFact3')];
}

function rotateFacts(facts, offset, count) {
  const source = ensureArray(facts).filter(Boolean);
  if (!source.length) return [];
  return Array.from({ length: Math.min(count, source.length) }, (_, index) => source[(offset + index) % source.length]);
}

function ensureBullets(items, state) {
  const source = ensureArray(items).map((item) => String(item).trim()).filter(Boolean);
  if (source.length) return source;
  return sourceFallbackFacts(state);
}

function ensureArray(value) {
  return Array.isArray(value) ? value : [];
}

function cleanTitle(value) {
  return String(value || '').replace(/\s+/g, ' ').trim().slice(0, 120);
}

function displayTopic(value) {
  const raw = cleanTitle(value);
  const withoutUrls = stripUrls(raw).trim() || raw;
  const normalized = withoutUrls
    .replace(/^create\s+(an?|the)?\s*\d+\s*[- ]?\s*(page|slide)\s+/i, '')
    .replace(/^create\s+(an?|the)?\s+/i, '')
    .replace(/^make\s+(an?|the)?\s+/i, '')
    .replace(/^build\s+(an?|the)?\s+/i, '')
    .replace(/^add\s+(an?|the)?\s*(page|slide)\s+(about|on|for)?\s*/i, '')
    .trim();
  const firstSentence = normalized.split(/[.!?。！？]/)[0]?.trim() || normalized;
  const concise = firstSentence.length > 72 ? firstSentence.slice(0, 69).trimEnd() + '...' : firstSentence;
  const urls = extractUrls(raw);
  if (!urls.length) return concise || raw;
  try {
    const parsed = new URL(urls[0]);
    if (parsed.hostname === 'github.com') {
      const [, owner, repo] = parsed.pathname.split('/');
      if (owner && repo) return `${owner}/${repo}`;
    }
    return parsed.hostname.replace(/^www\./, '');
  } catch {
    return concise || raw.replace(urls[0], '').trim() || urls[0];
  }
}

function extractUrls(value) {
  const matches = String(value || '').match(/https?:\/\/[^\s<>"'`]+/g) || [];
  return Array.from(new Set(matches.map(cleanUrlToken).filter(Boolean)));
}

function stripUrls(value) {
  return String(value || '').replace(/https?:\/\/[^\s<>"'`]+/g, '').replace(/\s+/g, ' ');
}

function cleanUrlToken(value) {
  let url = String(value || '').trim();
  while (/[.,，。;；:：!?！？、\])}\u300b\u300d\u300f]$/.test(url)) {
    url = url.slice(0, -1);
  }
  return url;
}

async function fetchReadableSources(url) {
  const host = window.app;
  if (!host?.net?.fetch) throw new Error('net unavailable');
  const targets = sourceTargets(url);
  let lastError = null;
  const found = [];
  for (const target of targets) {
    try {
      const response = await host.net.fetch(target.url, {
        headers: {
          Accept: target.accept || 'text/html,text/plain,application/json',
          'User-Agent': 'PPT-Live/1.0',
        },
      });
      if (Number(response.status) < 200 || Number(response.status) >= 300) throw new Error(`HTTP ${response.status}`);
      const text = readableText(response.body || '', target.url);
      if (text.length < 80) throw new Error('source too small');
      host.log?.info?.('PPT Live source fetched', { url: target.url, kind: target.kind, textLength: text.length });
      found.push({
        kind: target.kind,
        title: target.title || target.url,
        url: target.url,
        text: text.slice(0, 9000),
      });
    } catch (error) {
      host.log?.warn?.('PPT Live source target failed', { url: target.url, kind: target.kind, error: String(error) });
      lastError = error;
    }
  }
  if (found.length) return found;
  throw lastError || new Error('fetch failed');
}

function sourceTargets(url) {
  try {
    const parsed = new URL(cleanUrlToken(url));
    if (parsed.hostname === 'github.com') {
      const [, owner, repo] = parsed.pathname.split('/');
      if (owner && repo) {
        const cleanRepo = repo.replace(/\.git$/i, '');
        return [
          {
            kind: 'github-readme',
            title: `${owner}/${cleanRepo} README`,
            url: `https://raw.githubusercontent.com/${owner}/${cleanRepo}/HEAD/README.md`,
            accept: 'text/plain',
          },
          {
            kind: 'github-api',
            title: `${owner}/${cleanRepo}`,
            url: `https://api.github.com/repos/${owner}/${cleanRepo}`,
            accept: 'application/vnd.github+json',
          },
          { kind: 'web-page', title: cleanUrlToken(url), url: cleanUrlToken(url) },
        ];
      }
    }
  } catch {
    return [{ kind: 'web-page', title: cleanUrlToken(url), url: cleanUrlToken(url) }];
  }
  return [{ kind: 'web-page', title: cleanUrlToken(url), url: cleanUrlToken(url) }];
}

function readableText(body, url) {
  const raw = String(body || '');
  if (url.includes('api.github.com/repos/')) {
    try {
      const data = JSON.parse(raw);
      return [
        data.full_name,
        data.description,
        `Stars: ${data.stargazers_count ?? 'unknown'}`,
        `Forks: ${data.forks_count ?? 'unknown'}`,
        `Language: ${data.language || 'unknown'}`,
        `Topics: ${(data.topics || []).join(', ')}`,
        `Updated: ${data.updated_at || 'unknown'}`,
        data.homepage ? `Homepage: ${data.homepage}` : '',
      ].filter(Boolean).join('\n');
    } catch {
      return raw;
    }
  }
  return raw
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/\s+/g, ' ')
    .trim();
}

function extractFacts(text) {
  const clean = String(text || '').replace(/\s+/g, ' ').trim();
  const sentences = clean
    .split(/(?<=[。！？.!?])\s+|[\n\r]+/)
    .map((item) => item.trim())
    .filter((item) => item.length >= 16 && item.length <= 180);
  const numeric = sentences.filter((item) => /\d/.test(item));
  return [...numeric, ...sentences].slice(0, 12);
}

function summarizeSource(text, sources) {
  const facts = extractFacts(text).slice(0, 6);
  if (!facts.length) return '';
  return [t('sourceDigestTitle'), ...facts.map((fact) => `- ${fact}`), ...(sources.warnings || []).map((warning) => `- ${warning}`)].join('\n');
}

async function askAi() {
  throw new Error('PPT Live generation must use the BitFun host AI client');
}

function extractJson(value) {
  const raw = typeof value === 'string' ? value : JSON.stringify(value);
  try {
    return JSON.parse(raw);
  } catch {
    const match = raw.match(/\{[\s\S]*\}/);
    if (!match) return null;
    return JSON.parse(match[0]);
  }
}
