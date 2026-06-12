// BitFun backend adapter for PPT Live.
//
// Primary path: the MiniApp agent bridge (`app.agent.*`). Each `ppt.generate`
// call becomes one full host agent turn in a hidden session — the agent loads
// the built-in `ppt-design` skill and can use any host tools (WebSearch,
// WebFetch, Read, ...) for research.
//
// Fallback path: raw single-call LLM access (`app.ai.chat`) for hosts where
// the agent bridge or the `agent` permission is unavailable. The fallback has
// no tools or skills; prompts inline a condensed design ruleset instead.

const ACTIVE_RUNS = new Map();
const EVENT_LISTENERS = new Set();

function emitEvent(event) {
  EVENT_LISTENERS.forEach((listener) => {
    try {
      listener(event);
    } catch {
      // A UI listener must not break the host stream.
    }
  });
}

// ─── Agent prompt builder (staged plan/render/edit protocol) ─────────────────

const SLIDE_SHAPE_JSON = `{
      "slideNumber": 1,
      "role": "cover|content|data|transition|closing",
      "narrativeStage": "hook|progression|climax|landing",
      "title": "concrete slide title",
      "kicker": "short page type",
      "claim": "one core message",
      "proofObject": "source-backed proof or visual direction",
      "supportNote": "source fact, assumption, or verification note",
      "sourceNote": "source URL/name or verification note",
      "facts": ["verified fact or clearly marked assumption"],
      "bullets": ["short visible bullet"],
      "metric": { "value": "", "label": "" },
      "chartData": [],
      "notes": "speaker notes",
      "layout": "cover|brief|evidence|process|comparison|quote|data|closing",
      "visualTreatment": "typographic|grid|editorial|white-space|soft-tech|data|process|comparison",
      "html": "<!DOCTYPE html><html lang=\\"zh-CN\\"><head><meta charset=\\"UTF-8\\"><style>body{width:960pt;height:540pt;margin:0;overflow:hidden;...}</style></head><body>...</body></html>"
    }`;

function serializeInput(input) {
  try {
    return JSON.stringify(input ?? {}, null, 2);
  } catch {
    return '{}';
  }
}

function hasCurrentDeck(input) {
  return Array.isArray(input?.currentDeck?.slides) && input.currentDeck.slides.length > 0;
}

function buildOperationAppendix(input) {
  const operation = input?.operation || 'auto';
  if (!hasCurrentDeck(input)) {
    return `\n\n## Current operation\n\n- Operation: ${operation}\n- No current deck was provided. This is a first-pass deck generation run. Return a complete \`slides\` array.\n`;
  }
  return `

## Current operation

- Operation: ${operation}
- \`currentDeck\` is provided. Treat the user instruction as an incremental editing request for the existing deck unless the instruction explicitly asks for a completely new deck.
- \`currentDeck.slides[].slideIndex\` is zero-based. \`currentDeck.slides[].slideNumber\` is one-based and matches what users usually say.
- Use \`currentDeck.activeSlideIndex\` when the instruction says "current slide", "this page", "本页", "当前页", or similar.
- Decide the affected slide or slides yourself from the instruction, \`currentDeck.targetHints\`, slide titles, claims, notes, and visible text. Do not ask the user which pages to edit.
- Preserve unchanged slides exactly by returning a patch instead of regenerating them.
- Prefer \`deckPatch\` for revision, insertion, and deletion. Return a full \`slides\` array only when the user asks for a whole-deck rewrite or the requested change naturally affects most slides.

For incremental edits, return this optional patch shape instead of \`slides\`:
{
  "title": "existing or updated deck title",
  "language": "zh-CN or en-US",
  "outline": ["updated slide title list, optional"],
  "researchReport": {
    "summary": "what changed",
    "verifiedFacts": [],
    "assumptions": [],
    "warnings": []
  },
  "design": { "stylePhilosophy": "pentagram|muller-brockmann|build|kenya-hara|takram", "theme": "light|dark", "palette": {}, "layoutPrinciples": [] },
  "deckPatch": {
    "rationale": "why these slides were selected",
    "changedSlideIndexes": [0],
    "changes": [
      {
        "op": "replace_slide|insert_slide|delete_slide",
        "slideId": "existing slide id for replace/delete",
        "slideIndex": 0,
        "slideNumber": 1,
        "afterSlideId": "existing slide id for insert, optional",
        "slide": {
          "id": "reuse the existing id for replace; create a stable id only for insert",
          "role": "cover|content|data|transition|closing",
          "narrativeStage": "hook|progression|climax|landing",
          "title": "concrete slide title",
          "kicker": "short page type",
          "claim": "one core message",
          "proofObject": "source-backed proof or visual direction",
          "supportNote": "source fact, assumption, or verification note",
          "sourceNote": "source URL/name or verification note",
          "facts": ["verified fact or clearly marked assumption"],
          "bullets": ["short visible bullet"],
          "metric": { "value": "", "label": "" },
          "chartData": [],
          "notes": "speaker notes",
          "layout": "cover|brief|evidence|process|comparison|quote|data|closing",
          "visualTreatment": "typographic|grid|editorial|white-space|soft-tech|data|process|comparison",
          "html": "<!DOCTYPE html><html lang=\\"zh-CN\\"><head><meta charset=\\"UTF-8\\"><style>body{width:960pt;height:540pt;margin:0;overflow:hidden;...}</style></head><body>...</body></html>"
        }
      }
    ]
  }
}

Patch rules:
- \`replace_slide\`: include a complete replacement \`slide\` with mandatory \`html\`; reuse the original slide id.
- \`insert_slide\`: include a complete new \`slide\` with mandatory \`html\`; place it with \`afterSlideId\`, \`beforeSlideId\`, \`slideIndex\`, or \`slideNumber\`.
- \`delete_slide\`: do not include \`slide\`; target by \`slideId\` plus index/number when available.
- Never return an empty patch. If no change is needed, still make the smallest useful improvement requested by the user.
- If you return a full \`slides\` array during an edit, it must include every final slide in order. Missing unchanged slides will be treated as deleted.
`;
}

function buildStyleAppendix(input) {
  const style = input?.style || {};
  const font = style.fontFamily || 'sans';
  const densityRaw = style.density || 'standard';
  const density = densityRaw === 'loose' ? 'spacious' : densityRaw;
  const colorMode = style.colorMode || 'light';
  const stylePreset = style.stylePreset || '';
  const palette = style.palette;

  const fontRule = font === 'serif'
    ? 'serif — use serif typography in every slide HTML (for example Georgia, "Songti SC", "Times New Roman", Cambria). Avoid sans-serif body copy.'
    : 'sans-serif — use clean sans-serif typography in every slide HTML (for example system-ui, "PingFang SC", "Microsoft YaHei", Arial, Helvetica). Avoid serif body copy.';

  let densityRule;
  if (density === 'compact') {
    densityRule = 'compact — information-forward: body padding 24-32px, line-height 1.2-1.28, and 4-6 concise bullets, metrics, or a two-column grid when the content supports it. Prefer readable tightness over decorative whitespace; never overflow the slide.';
  } else if (density === 'spacious') {
    densityRule = 'spacious — the loosest tier, still content-rich: body padding 44-52px, line-height 1.32-1.4, and 2-4 concise bullets or 2-3 short content blocks per slide. Keep clear hierarchy without leaving large empty regions.';
  } else {
    densityRule = 'standard — balanced professional density: body padding 34-42px, line-height 1.26-1.34, and 3-5 bullets, metrics, or paired columns when useful. Use whitespace to separate sections, not to leave half the slide blank.';
  }

  const colorRule = colorMode === 'dark'
    ? 'dark — use dark slide backgrounds with light text, high-contrast panels, and a keynote-style atmosphere. Set design.theme to dark and reflect it in every slides[].html background, text, and panel colors.'
    : 'light — use light slide backgrounds with dark text, clean readable contrast, and a professional presentation look. Set design.theme to light and reflect it in every slides[].html background, text, and panel colors.';

  let styleRules = `

## Presentation style preferences (must follow in slides[].html)

- Font family: ${fontRule}
- Information density: ${densityRule}
- Slide color mode: ${colorRule}

## Hard layout rules (apply to every slides[].html, any style)

- Zero overflow, enforced by budget: before writing each slide, budget the vertical space — title block 70-95pt + footer 20-25pt + a mandatory >=36pt (0.5in) bottom safety margin leaves only ~390-420pt for body content. Estimate every block as \`lines x font-size x line-height + paddings\` (tables as \`rows x row-height\`); if the sum exceeds the body budget, cut rows, merge columns, or split the slide. Never shrink fonts below 10px to force-fit content.
- Structural clipping fallback: set \`body { overflow: hidden; }\`, make the root a \`display:flex; flex-direction:column; height:540pt;\` container, and give the stretchable content area \`flex:1; min-height:0; overflow:hidden;\` so a misestimate clips inside its container instead of overflowing the canvas. Every text box larger than 12px must end >=0.5in above the canvas bottom.
- Choose the representation by content shape, judged per slide by which form communicates fastest: comparisons -> tables/matrices, rankings -> CSS horizontal bar charts, trends -> CSS column charts, composition -> \`conic-gradient\` pie/donut, strategy -> SWOT/2x2 grids, processes -> flow diagrams with CSS arrows, milestones -> timelines, single KPIs -> big-number callouts; qualitative reasoning or narrative stays as structured text. Do not write paragraphs where a visual is clearly faster, and do not force decorative charts onto purely qualitative content. Pure HTML/CSS only, label every bar/segment with its value, and pair each visual with a one-line takeaway.
`;

  // Style preset guidance routes through the ppt-design skill so the run stays
  // anchored to the skill's quality system.
  if (stylePreset) {
    styleRules += `\n- Style preset: \`${stylePreset}\`. After loading the ppt-design skill, \`Read\` its \`references/style-presets/${stylePreset}.md\` (the path is relative to the skill directory reported by the Skill tool) and apply that file in full to every slides[].html: visual identity (palette, typography mood, decorative language, recommended layouts) plus any information-density, language, and page-structure rules the preset defines. When the preset's density or structure rules conflict with the generic density preference above, the preset wins.\n`;
    if (palette) {
      try {
        styleRules += `- Style palette (matches the preset; use these exact colors for backgrounds, text, accents, and panels in every slide HTML): ${JSON.stringify(palette)}\n`;
      } catch {
        // Ignore unserializable palettes.
      }
    }
    styleRules += "- The preset does not suspend the ppt-design core rules: assertion-led titles, one core message per slide, anti-AI-slop rules, the 960pt x 540pt canvas, editable-PPTX constraints, and zero content overflow all still apply.\n- Pick the closest of the skill's five design philosophies as the structural grammar for layout, then skin it with the preset. If the preset file cannot be read, keep the palette above and fall back to that philosophy.\n";
  }

  return styleRules;
}

function buildPlanPrompt(input) {
  const body = `Plan a PPT Live deck. This is the PLANNING phase of a staged pipeline: research the topic, lock the narrative, and write a per-slide brief. Slide HTML is produced later by separate render runs that follow your plan exactly, so the plan must be complete and self-sufficient.

1. Call \`Skill('ppt-design')\` — the BitFun built-in PPT design skill — and follow its narrative, density, and design-system rules when planning. Never substitute any other presentation or PPT skill.
2. Use any BitFun tools you need (WebFetch, WebSearch, Read, etc.) when the user's prompt requires external facts. All research happens NOW; render runs are forbidden from re-researching.
3. Finish with **only** one strict JSON object — no Markdown fences, no commentary, no tool calls in the final message.
4. Do NOT generate any slide HTML in this phase.

Return JSON matching this shape:
{
  "title": "deck title",
  "language": "zh-CN or en-US",
  "outline": ["slide title"],
  "researchReport": {
    "summary": "short internal summary safe to show as a product status detail",
    "verifiedFacts": ["fact with source note when available"],
    "assumptions": ["clearly marked assumption"],
    "warnings": ["source or verification warning"]
  },
  "design": {
    "stylePhilosophy": "pentagram|muller-brockmann|build|kenya-hara|takram",
    "theme": "light|dark",
    "palette": {
      "background": "#FAFAF7",
      "ink": "#1A1A1A",
      "muted": "#666666",
      "primary": "#111111",
      "accent": "#C84B31",
      "panel": "#FFFFFF"
    },
    "layoutPrinciples": ["specific visual rules every slide of this deck must share"]
  },
  "slidePlans": [
    {
      "slideNumber": 1,
      "role": "cover|content|data|transition|closing",
      "narrativeStage": "hook|progression|climax|landing",
      "title": "concrete slide title",
      "kicker": "short page type",
      "claim": "one core message",
      "proofObject": "source-backed proof or visual direction",
      "supportNote": "source fact, assumption, or verification note",
      "sourceNote": "source URL/name or verification note",
      "facts": ["verified fact or clearly marked assumption"],
      "bullets": ["short visible bullet"],
      "metric": { "value": "", "label": "" },
      "chartData": [],
      "notes": "speaker notes",
      "layout": "cover|brief|evidence|process|comparison|quote|data|closing",
      "visualTreatment": "typographic|grid|editorial|white-space|soft-tech|data|process|comparison",
      "contentBrief": "everything the render run needs to build this slide without asking questions: the exact copy or copy direction, the data values to visualize and the recommended visual form (table/bar/column/pie/SWOT/flow/timeline/big-number/structured text), and the layout intent"
    }
  ]
}

Plan rules:
- \`slidePlans\` must cover the full deck in final order; \`slideNumber\` is one-based and contiguous.
- Every \`contentBrief\` must be concrete enough that a render run with no research access can produce an audience-ready slide from it. Put real numbers, names, and source notes into the briefs, not vague directions.
- \`design.layoutPrinciples\` and \`design.palette\` are the consistency contract across parallel render runs — make them specific.

Output budget (hard limits — the plan JSON is streamed over a connection that gets cut after several minutes, so an oversized plan ALWAYS fails and wastes the entire run):
- Write dense, telegraphic notes, never prose paragraphs. Pack facts, numbers, and names; drop filler words.
- \`contentBrief\`: at most ~400 characters per slide.
- \`facts\`: at most 4 items; \`bullets\`: at most 4 items; each item one short line.
- \`proofObject\`, \`supportNote\`, \`sourceNote\`, \`notes\`: one short sentence each.
- \`researchReport.summary\`: at most ~600 characters; \`verifiedFacts\`/\`assumptions\`/\`warnings\`: at most 12 short items combined.
- Total plan JSON must stay under ~25,000 characters even for large decks. If the deck is big, make each brief tighter instead of dropping slides.

Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;
  return body + buildStyleAppendix(input);
}

function buildSlidesPrompt(input) {
  const assigned = (input?.assignedSlides || [])
    .map((slide) => slide?.slideNumber)
    .filter((number) => number != null)
    .join(', ');
  const body = `Render PPT Live slides. This is the RENDER phase of a staged pipeline. The plan (research, outline, design system, per-slide briefs) is already final and is provided in the input JSON as \`plan\`. Your batch must render ONLY the slides listed in \`assignedSlides\` (slide numbers: ${assigned}).

1. Call \`Skill('ppt-design')\` — the BitFun built-in PPT design skill — and follow it end-to-end for slide HTML quality. Never substitute any other presentation or PPT skill.
2. Do NOT re-research. Do not call WebSearch or WebFetch. Trust \`plan.researchReport\` and each slide's \`contentBrief\` completely; they contain all verified facts.
3. Do NOT change the plan: keep each assigned slide's \`slideNumber\`, title, claim, layout, and narrative role as planned. Apply \`plan.design\` (philosophy, theme, palette, layoutPrinciples) to every slide so parallel batches stay visually identical.
4. Finish with **only** one strict JSON object — no Markdown fences, no commentary, no tool calls in the final message.

Every slide must include complete \`html\`: self-contained 960pt × 540pt HTML with inline CSS (ppt-design editable PPTX rules). Slide copy must be audience-ready, never placeholder instructions.

Return JSON matching this shape:
{
  "slides": [
    ${SLIDE_SHAPE_JSON}
  ]
}

Render rules:
- Return exactly the slides listed in \`assignedSlides\`, in ascending \`slideNumber\` order, and no others. If \`completedSlides\` is present in the input, those slides are already done — never regenerate them.
- Emit each slide's JSON object completely before starting the next one, so partial output remains recoverable.
- Keep the HTML compact: no HTML comments, no unused CSS rules, minimal whitespace and indentation. The response is streamed over a connection that gets cut after several minutes, so wasted characters risk failing the whole batch. Density of CONTENT is good; padding of MARKUP is not.

Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;
  return body + buildStyleAppendix(input);
}

function buildLegacyPrompt(input) {
  const body = `Generate or revise a PPT Live deck. The user only sees the PPT Live app UI.

1. Call \`Skill('ppt-design')\` — the BitFun built-in PPT design skill — and follow it end-to-end. Never substitute any other presentation or PPT skill, even if one appears in the available skills list; ignore user-installed PPT design skills entirely for this run.
2. Use any BitFun tools you need (WebFetch, WebSearch, etc.) when the user's prompt requires external facts.
3. Finish with **only** one strict JSON object — no Markdown fences, no commentary, no tool calls in the final message.

Every slide must include complete \`slides[].html\`: self-contained 960pt × 540pt HTML with inline CSS (ppt-design editable PPTX rules). Slide copy must be audience-ready, never placeholder instructions.

Return JSON matching this shape:
{
  "title": "deck title",
  "language": "zh-CN or en-US",
  "outline": ["slide title"],
  "researchReport": {
    "summary": "short internal summary safe to show as a product status detail",
    "verifiedFacts": ["fact with source note when available"],
    "assumptions": ["clearly marked assumption"],
    "warnings": ["source or verification warning"]
  },
  "design": {
    "stylePhilosophy": "pentagram|muller-brockmann|build|kenya-hara|takram",
    "theme": "light|dark",
    "palette": {
      "background": "#FAFAF7",
      "ink": "#1A1A1A",
      "muted": "#666666",
      "primary": "#111111",
      "accent": "#C84B31",
      "panel": "#FFFFFF"
    },
    "layoutPrinciples": ["specific visual rules used for this deck"]
  },
  "slides": [
    ${SLIDE_SHAPE_JSON}
  ]
}

Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;
  return body + buildOperationAppendix(input) + buildStyleAppendix(input);
}

/**
 * Build the full agent user prompt for a `ppt.generate` run.
 * `input.phase` selects the staged-pipeline protocol:
 * - "plan": research + outline + design system + per-slide briefs, no HTML.
 * - "slides": render the assigned slides from a finished plan, no research.
 * - absent: legacy single-shot protocol (full deck or incremental patch).
 */
function buildAgentPrompt(input) {
  if (input?.phase === 'plan') return buildPlanPrompt(input);
  if (input?.phase === 'slides') return buildSlidesPrompt(input);
  return buildLegacyPrompt(input);
}

// ─── Agent-backed backend (primary path) ─────────────────────────────────────

function installAgentBackend(app) {
  let agentEventsHooked = false;
  const ensureAgentEvents = () => {
    if (agentEventsHooked) return;
    agentEventsHooked = true;
    // Host events already carry sessionId/turnId/sourceEvent/text/contentType/
    // toolEvent/error in the shape ui.js consumes; re-emit them as-is.
    app.agent.onEvent((event) => {
      if (!event || typeof event !== 'object') return;
      emitEvent(event);
    });
  };

  app.backend = {
    async call(action, input, options = {}) {
      if (action !== 'ppt.generate') {
        throw new Error(`Unsupported PPT Live action: ${action}`);
      }
      ensureAgentEvents();
      const prompt = buildAgentPrompt(input);
      const result = await app.agent.run(prompt, {
        runId: options.idempotencyKey,
        sessionName: 'PPT Live',
      });
      if (!result?.sessionId || !result?.turnId) {
        throw new Error('PPT Live agent backend did not return sessionId/turnId');
      }
      return {
        sessionId: result.sessionId,
        turnId: result.turnId,
        actionRunId: result.actionRunId || result.turnId,
      };
    },
    onEvent(listener) {
      EVENT_LISTENERS.add(listener);
    },
    offEvent(listener) {
      EVENT_LISTENERS.delete(listener);
    },
    async cancel(sessionId, turnId) {
      await app.agent.cancel(sessionId, turnId);
    },
    async turnText(sessionId, turnId) {
      const result = await app.agent.turnText(sessionId, turnId);
      return { text: result?.text || '' };
    },
    async cancelStaleRuns() {
      await app.agent.cancelStaleRuns();
    },
  };
}

// ─── Raw-LLM fallback backend (no tools, no skills) ──────────────────────────

const COMMON_SYSTEM_PROMPT = `You are PPT Live, a presentation design engine embedded in BitFun.
Return strict JSON only, without markdown fences or commentary.
Use only the user brief, supplied source material, and clearly marked assumptions.
Every rendered slide must be a self-contained HTML document sized for a 960pt by 540pt canvas.
Do not use remote scripts, stylesheets, fonts, images, or other network assets in slide HTML.
Prefer concise assertion-led copy, strong hierarchy, generous whitespace, and content-aware layouts.
Avoid purple gradients, decorative emoji, generic illustration filler, fake citations, and text-heavy pages.
Keep body text at least 10px and reserve a 36pt bottom safety margin.`;

function emitRunEvent(run, sourceEvent, fields = {}) {
  emitEvent({
    sessionId: run.sessionId,
    turnId: run.turnId,
    sourceEvent,
    ...fields,
  });
}

function styleRules(input) {
  const style = input?.style || {};
  return [
    `Color mode: ${style.colorMode || 'light'}.`,
    `Font family: ${style.fontFamily || 'sans'}.`,
    `Density: ${style.density || 'standard'}.`,
    `Style preset: ${style.stylePreset || 'editorial'}.`,
    `Palette: ${JSON.stringify(style.palette || {})}.`,
    'Use flex or grid with min-height:0 and overflow:hidden.',
    'Choose structure from the content: comparison, data, process, timeline, KPI, quote, or structured text.',
    'Label quantitative visuals with values.',
  ].join('\n');
}

function buildFallbackPrompt(input) {
  const serializedInput = JSON.stringify(input);
  if (input?.phase === 'plan') {
    return {
      systemPrompt: `${COMMON_SYSTEM_PROMPT}
This is the planning phase. Do not generate slide HTML.
Return:
{"title":"","language":"zh-CN|en-US","outline":[],"researchReport":{"summary":"","verifiedFacts":[],"assumptions":[],"warnings":[]},"design":{"stylePhilosophy":"pentagram|muller-brockmann|build|kenya-hara|takram","theme":"light|dark","palette":{},"layoutPrinciples":[]},"slidePlans":[{"slideNumber":1,"role":"cover|content|data|transition|closing","narrativeStage":"hook|progression|climax|landing","title":"","kicker":"","claim":"","proofObject":"","supportNote":"","sourceNote":"","facts":[],"bullets":[],"metric":{"value":"","label":""},"chartData":[],"notes":"","layout":"cover|brief|evidence|process|comparison|quote|data|closing","visualTreatment":"typographic|grid|editorial|white-space|soft-tech|data|process|comparison","contentBrief":""}]}.
slidePlans must be one-based, contiguous, complete, and in final order. Each contentBrief must contain exact copy direction, facts, visual form, and layout intent. Keep the JSON compact.`,
      prompt: `Plan this deck from the supplied brief and source material.
${styleRules(input)}
Input JSON:
${serializedInput}`,
    };
  }

  if (input?.phase === 'slides') {
    const assigned = (input.assignedSlides || [])
      .map((slide) => slide.slideNumber)
      .join(', ');
    return {
      systemPrompt: `${COMMON_SYSTEM_PROMPT}
This is the render phase. Render only assigned slide numbers ${assigned}; do not research or change the plan.
Return:
{"slides":[{"slideNumber":1,"role":"cover|content|data|transition|closing","narrativeStage":"hook|progression|climax|landing","title":"","kicker":"","claim":"","proofObject":"","supportNote":"","sourceNote":"","facts":[],"bullets":[],"metric":{"value":"","label":""},"chartData":[],"notes":"","layout":"cover|brief|evidence|process|comparison|quote|data|closing","visualTreatment":"typographic|grid|editorial|white-space|soft-tech|data|process|comparison","html":"<!DOCTYPE html>..."}]}.
Return exactly the assigned slides in ascending order. Preserve each planned title, claim, role, and slideNumber. Apply plan.design consistently and complete each slide object before starting the next.`,
      prompt: `Render the assigned slides.
${styleRules(input)}
Input JSON:
${serializedInput}`,
    };
  }

  const editRules = hasCurrentDeck(input)
    ? `The current deck is supplied. Prefer a minimal deckPatch:
{"title":"","language":"zh-CN|en-US","outline":[],"researchReport":{"summary":"","verifiedFacts":[],"assumptions":[],"warnings":[]},"design":{"stylePhilosophy":"","theme":"light|dark","palette":{},"layoutPrinciples":[]},"deckPatch":{"rationale":"","changedSlideIndexes":[0],"changes":[{"op":"replace_slide|insert_slide|delete_slide","slideId":"","slideIndex":0,"slideNumber":1,"afterSlideId":"","slide":{"id":"","title":"","claim":"","notes":"","html":"<!DOCTYPE html>..."}}]}}.
For replace_slide, reuse the existing id and return a complete replacement slide. For delete_slide, omit slide. Return a full slides array only for whole-deck rewrites.`
    : 'This is a first-pass generation. Return a complete slides array using the render-phase slide shape.';

  return {
    systemPrompt: `${COMMON_SYSTEM_PROMPT}
${editRules}`,
    prompt: `Generate or revise the deck.
${styleRules(input)}
Input JSON:
${serializedInput}`,
  };
}

function createFallbackRun() {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
  const run = {
    sessionId: `bitfun-ppt-${id}`,
    turnId: `bitfun-turn-${id}`,
    text: '',
    thinking: '',
    handle: null,
    cancelled: false,
    settled: false,
  };
  ACTIVE_RUNS.set(run.turnId, run);
  return run;
}

function startFallbackRun(app, run, input) {
  if (run.cancelled) return;
  const aiPrompt = buildFallbackPrompt(input);
  emitRunEvent(run, 'bitfun:model-round-started');
  app.ai
    .chat([{ role: 'user', content: aiPrompt.prompt }], {
      systemPrompt: aiPrompt.systemPrompt,
      model: 'primary',
      maxTokens: 16000,
      temperature: 0.35,
      onChunk: (chunk = {}) => {
        if (run.cancelled || run.settled) return;
        const text = typeof chunk === 'string' ? chunk : chunk.text;
        const reasoning = typeof chunk === 'object' ? chunk.reasoningContent : '';
        if (text) {
          run.text += String(text);
          emitRunEvent(run, 'bitfun:text-chunk', {
            text: String(text),
            contentType: 'answer',
          });
        }
        if (reasoning) {
          run.thinking += String(reasoning);
          emitRunEvent(run, 'bitfun:text-chunk', {
            text: String(reasoning),
            contentType: 'thinking',
          });
        }
      },
      onDone: (result = {}) => {
        if (run.cancelled || run.settled) return;
        const fullText = String(result.fullText || '').trim();
        if (fullText) run.text = fullText;
        run.settled = true;
        emitRunEvent(run, 'bitfun:model-round-completed');
        emitRunEvent(run, 'bitfun:dialog-turn-completed');
      },
      onError: (error = {}) => {
        if (run.cancelled || run.settled) return;
        run.settled = true;
        emitRunEvent(run, 'bitfun:dialog-turn-failed', {
          error: String(error.message || error || 'PPT Live AI request failed'),
        });
      },
    })
    .then((handle) => {
      run.handle = handle;
      if (run.cancelled) return handle?.cancel?.();
      return undefined;
    })
    .catch((error) => {
      if (run.cancelled || run.settled) return;
      run.settled = true;
      emitRunEvent(run, 'bitfun:dialog-turn-failed', {
        error: String(error?.message || error || 'PPT Live AI request failed'),
      });
    });
}

async function cancelFallbackRun(app, run) {
  if (!run || run.cancelled || run.settled) return;
  run.cancelled = true;
  try {
    if (run.handle?.cancel) {
      await run.handle.cancel();
    } else if (run.handle?.streamId && app.ai?.cancel) {
      await app.ai.cancel(run.handle.streamId);
    }
  } finally {
    run.settled = true;
    emitRunEvent(run, 'bitfun:dialog-turn-cancelled');
  }
}

function installFallbackBackend(app) {
  app.backend = {
    call(action, input) {
      if (action !== 'ppt.generate') {
        return Promise.reject(new Error(`Unsupported PPT Live action: ${action}`));
      }
      const run = createFallbackRun();
      setTimeout(() => startFallbackRun(app, run, input), 0);
      return Promise.resolve({
        sessionId: run.sessionId,
        turnId: run.turnId,
        actionRunId: run.turnId,
      });
    },
    onEvent(listener) {
      EVENT_LISTENERS.add(listener);
    },
    offEvent(listener) {
      EVENT_LISTENERS.delete(listener);
    },
    async cancel(sessionId, turnId) {
      const run = ACTIVE_RUNS.get(turnId);
      if (run?.sessionId === sessionId) await cancelFallbackRun(app, run);
    },
    async turnText(sessionId, turnId) {
      const run = ACTIVE_RUNS.get(turnId);
      if (!run || run.sessionId !== sessionId) return { text: '' };
      return { text: run.text || run.thinking || '' };
    },
    async cancelStaleRuns() {
      await Promise.all(
        [...ACTIVE_RUNS.values()]
          .filter((run) => !run.settled)
          .map((run) => cancelFallbackRun(app, run)),
      );
    },
  };
}

// ─── Install ─────────────────────────────────────────────────────────────────

export function installBitFunBackendAdapter(app = window.app) {
  if (!app || app.backend?.call) return;
  if (app.agent?.run) {
    installAgentBackend(app);
    return;
  }
  if (app.ai?.chat) {
    installFallbackBackend(app);
  }
}
