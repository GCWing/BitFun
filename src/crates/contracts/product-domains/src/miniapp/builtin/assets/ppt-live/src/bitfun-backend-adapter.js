// BitFun backend adapter for PPT Live.
//
// The MiniApp agent bridge (`app.agent.*`) is the only generation path. The
// planning turn loads BitFun's pinned built-in `ppt-design` skill, reads the
// required references, and writes a durable generation contract. Serial render
// and edit turns reuse the deck session and project directory.

const EVENT_LISTENERS = new Set();
export const PPT_DESIGN_SKILL_KEY = 'user::bitfun-system::ppt-design';
export const PPT_DESIGN_REQUIRED_REFERENCES = [
  'references/editable-pptx.md',
  'references/slide-decks.md',
  'references/content-guidelines.md',
];

const EDITABLE_PPTX_HARD_RULES = `
- Text must never be a direct child of a DIV. Put all visible text in p, h1-h6, or li; span is inline-only inside those text elements.
- Do not use CSS gradients. Use solid fills and discrete solid-color shapes.
- Backgrounds, borders, and shadows belong on DIV shapes, never on p, h1-h6, li, span, em, or strong.
- Do not use background-image on DIV. Use an img element for images. Inline span/em/strong must not carry margin, padding, background, border, or shadow.
`;

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
  const colorMode = style.colorMode || style.theme || 'light';
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

// Prefixed to any turn that re-runs after an interrupted attempt, so the model
// treats the rerun as a continuation instead of a contradictory new task.
const CONTINUE_AFTER_INTERRUPTION_PREFIX = `Your previous response in this session was interrupted before it finished. Continue the task now. If your deliverable is a project file, first inspect what you already wrote (Read it) and rewrite that file completely if it could be incomplete. If your deliverable is a final JSON message, re-emit the complete JSON from scratch — do not assume any part of the interrupted output was received.

`;

function buildCompletionRecoveryPrefix(recovery) {
  const issues = Array.isArray(recovery?.issues) ? recovery.issues.map(String).filter(Boolean) : [];
  const previousFailure = String(recovery?.previousFailure || '').trim();
  return `AUTOMATIC COMPLETION CONTINUATION ${recovery?.attempt || 1}/${recovery?.maxAttempts || 1}.

The host has verified that the "${recovery?.stage || 'current'}" stage is still incomplete after its normal retry budget. Continue the unfinished task in this user turn; do not restart completed work and do not merely explain what remains.
- Inspect the durable project files first and reuse every valid artifact already present.
- Perform the missing tool calls or rewrites now.
- Do not claim completion until the required artifact exists and satisfies the host checks.
${previousFailure ? `- Previous verified failure: ${previousFailure}\n` : ''}${
  issues.length ? `- Host verification issues:\n  - ${issues.join('\n  - ')}\n` : ''
}
`;
}

function schemaPaletteBlock(input) {
  const palette = input?.style?.palette || {};
  return JSON.stringify(palette, null, 2)
    .split('\n')
    .map((line) => `      ${line}`)
    .join('\n');
}

function buildPlanPrompt(input) {
  const stylePreset = input?.style?.stylePreset || '';
  const colorMode = input?.style?.colorMode || input?.style?.theme || 'light';
  const presetReference = stylePreset
    ? `references/style-presets/${stylePreset}.md`
    : '';
  const complianceRepair = Array.isArray(input?.complianceIssues) && input.complianceIssues.length
    ? `The host rejected the previous plan because required Skill evidence was missing. Complete these exact missing actions before accepting or rewriting project.json:\n- ${input.complianceIssues.join('\n- ')}\n\n`
    : '';
  const body = `Plan a PPT Live deck. This is the PLANNING phase of a staged pipeline running in a dedicated deck project directory: the workspace root of this session is the deck's \`{{ppt_project_dir}}\`. Research the topic, lock the narrative, design the visual system, write a per-slide brief, and SAVE the complete plan to \`project.json\` at the workspace root. Later turns run in THIS SAME session, but must be able to recover from context compression using the self-contained generation contract in that file.

1. Call \`Skill('${PPT_DESIGN_SKILL_KEY}')\`. This exact stable key is mandatory. Check the tool result reports \`skill_key="${PPT_DESIGN_SKILL_KEY}"\`; if it does not, stop with an error. Never invoke or substitute a different PPT/presentation skill.
2. From the skill directory returned by Skill, \`Read\` all mandatory references: \`${PPT_DESIGN_REQUIRED_REFERENCES.join('`, `')}\`.
${presetReference ? `3. \`Read\` the selected style preset \`${presetReference}\` and apply its visual rules when planning.` : '3. No named style preset was supplied; derive one coherent visual system from the user intent and skill.'}
4. If the deck is data-heavy, analytical, explanatory, or structurally complex, \`Read\` \`references/data-information-visualization.md\` before drafting slide plans.
5. Use any BitFun research tools needed by the user's prompt. All external research happens NOW; render runs must not re-research.
6. When the plan is final, \`Write\` it to \`project.json\` as one strict JSON object matching the schema below. Do not generate slide HTML in this phase.
7. For a deck with at least 5 pages, choose two visually different \`showcaseSlideNumbers\` that later render first and establish the grammar for the remaining pages.
8. End with one short status line such as "PLAN READY: N slides". Do not paste the JSON into the reply.

\`project.json\` schema:
{
  "title": "deck title",
  "language": "zh-CN or en-US",
  "outline": [
    { "id": "slide-01", "title": "slide title", "bullets": [], "slide_id": "slide-01" }
  ],
  "slide_order": ["slide-01"],
  "style": {
    "stylePreset": "${stylePreset}",
    "fontFamily": "exact input value",
    "density": "exact input value",
    "colorMode": "exact input value",
    "palette": {}
  },
  "assumptions": ["one-shot assumptions"],
  "generationContract": {
    "version": 1,
    "skillKey": "${PPT_DESIGN_SKILL_KEY}",
    "skillName": "ppt-design",
    "requiredReferences": [
      ${PPT_DESIGN_REQUIRED_REFERENCES.map((ref) => `"${ref}"`).join(',\n      ')}${presetReference ? `,\n      "${presetReference}"` : ''}
    ],
    "deliveryTarget": "editable-pptx",
    "userPrompt": "copy input.instruction exactly",
    "userBrief": {},
    "userStyle": {},
    "hardRules": ["compact exact rules distilled from editable-pptx.md"],
    "visualGrammar": {
      "designRead": "one-line audience, task, visual language, structural grammar",
      "visualThesis": "why this brief should look this way; not generic adjectives",
      "signatureMove": "one theme-specific device used sparingly across the deck",
      "typography": "hierarchy via weight, size, spacing — not decoration",
      "paletteRoles": "semantic role of every supplied color",
      "composition": "grid, margins, edge discipline, and dominant visual masses",
      "layoutFamilies": ["deck-specific families with different silhouettes"],
      "objectStyles": "tables, charts, diagrams, images, quotes, annotations, and sources",
      "surfaces": "lines and whitespace first; cards and shadow only when hierarchy requires",
      "dataAndDiagrams": "semantics over template shapes",
      "densityCurve": "where the deck opens, builds, peaks, releases, and closes",
      "copyRegister": "language tone, sentence style, label discipline, and evidence standard",
      "antiDefaults": ["brief-specific defaults that would make this deck generic"],
      "pageRhythm": "showcase grammar, adjacent-page contrast, and accent discipline"
    }
  },
  "showcaseSlideNumbers": [1, 3],
  "researchReport": {
    "summary": "short internal summary safe to show as a product status detail",
    "verifiedFacts": ["fact with source note when available"],
    "assumptions": ["clearly marked assumption"],
    "warnings": ["source or verification warning"]
  },
  "design": {
    "stylePhilosophy": "pentagram|muller-brockmann|build|kenya-hara|takram",
    "theme": "${colorMode === 'dark' ? 'dark' : 'light'}",
    "palette": {
${schemaPaletteBlock(input)}
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
- \`outline[].slide_id\`, \`slide_order[]\`, and \`slides/slide-XX.html\` numbering must agree.
- Copy the user's original instruction, brief, and all style settings into \`generationContract\`; do not paraphrase away explicit requirements.
- Every \`contentBrief\` must be concrete enough that a render run with no research access can produce an audience-ready slide from it. Put real numbers, names, and source notes into the briefs, not vague directions.
- \`design.layoutPrinciples\` and \`design.palette\` are the consistency contract across render runs — make them specific.
- Distill the skill, mandatory references, selected preset, and user style into \`generationContract.hardRules\` and \`generationContract.visualGrammar\` so later turns can recover after context compression.
- The editable PPTX rules below are non-negotiable and must appear compactly in \`generationContract.hardRules\`:
${EDITABLE_PPTX_HARD_RULES}

Plan density (quality bar, not a protocol limit):
- Write dense, telegraphic notes, never prose paragraphs. Pack facts, numbers, and names; drop filler words.
- \`contentBrief\`: at most ~400 characters per slide.
- \`facts\`: at most 4 items; \`bullets\`: at most 4 items; each item one short line.
- \`proofObject\`, \`supportNote\`, \`sourceNote\`, \`notes\`: one short sentence each.
- \`researchReport.summary\`: at most ~600 characters; \`verifiedFacts\`/\`assumptions\`/\`warnings\`: at most 12 short items combined.

Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;
  return complianceRepair + body + buildStyleAppendix(input);
}

/** Two-digit slide file name per the ppt-design skill convention. */
function slideFileName(slideNumber) {
  return `slides/slide-${String(slideNumber).padStart(2, '0')}.html`;
}

/**
 * Render prompt for a turn that runs INSIDE the planning session. The turn
 * receives a compact self-contained contract even though it reuses
 * the planning session. This keeps page quality stable after compression.
 */
function buildSessionSlidePrompt(input) {
  const slidePlan = (input?.assignedSlides || [])[0] || {};
  const assigned = slidePlan?.slideNumber ?? '?';
  const file = slideFileName(slidePlan?.slideNumber || 0);
  return `Render PPT Live slide ${assigned}. This turn runs in the SAME deck Agent Session as planning, but you must rely on the durable project contract rather than assuming old context is still present.

1. \`Read\` \`project.json\` on every page turn. Verify \`generationContract.skillKey\` is exactly \`${PPT_DESIGN_SKILL_KEY}\`, then follow its userPrompt, userBrief, userStyle, hardRules, visualGrammar, design system, and assigned slide plan together.
2. If \`showcaseSlideNumbers\` contains already-rendered pages, \`Read\` those slide files before rendering a non-showcase page and reuse their visual grammar without copying their layout mechanically.
3. Do not re-run research or change the planned title, claim, layout, or narrative role. Apply \`plan.design\` and the assigned slide plan together.
4. \`Write\` exactly one complete document to \`${file}\`. Do not modify any other slide.
5. End with "SLIDE ${assigned} READY"; do not paste HTML into the reply.

Hard HTML/PPTX constraints:
- The file must be a complete self-contained 960pt × 540pt document with inline CSS only and no remote assets.
- \`body { overflow: hidden; }\`, flex-column root with \`height: 540pt\`, stretchable areas \`flex:1; min-height:0; overflow:hidden;\`.
- Budget the vertical space before writing; body text >= 10px; keep a >=36pt bottom safety margin; never overflow the canvas.
${EDITABLE_PPTX_HARD_RULES}
- Slide copy must be audience-ready, never placeholder instructions.
${buildStyleAppendix(input)}

Current page contract:
\`\`\`json
${serializeInput({
    generationContract: input?.generationContract || {},
    design: input?.design || {},
    style: input?.style || {},
    showcaseSlideNumbers: input?.showcaseSlideNumbers || [],
    assignedSlide: slidePlan,
  })}
\`\`\``;
}

/**
 * Self-contained render prompt for a turn WITHOUT the planning session (the
 * session was lost, e.g. after a webview reload or backend restart). The turn
 * reloads the skill itself and receives the full plan in the input JSON, then
 * writes the slide file exactly like the in-session variant.
 */
function buildSlidesPrompt(input) {
  const slidePlan = (input?.assignedSlides || [])[0] || {};
  const assigned = slidePlan?.slideNumber ?? '?';
  const file = slideFileName(slidePlan?.slideNumber || 0);
  const body = `Render one PPT Live slide. This is the RENDER phase of a staged pipeline running in a dedicated deck project directory: the workspace root of this session is the deck's \`{{ppt_project_dir}}\`. The plan (research, outline, design system, render guide, and per-slide briefs) is already final and is provided in the input JSON as \`plan\`. This turn must render ONLY slide ${assigned}.

1. Call \`Skill('${PPT_DESIGN_SKILL_KEY}')\`; verify the result reports the exact stable key. Never substitute another skill.
2. \`Read\` every path in \`plan.generationContract.requiredReferences\`, including mandatory references \`${PPT_DESIGN_REQUIRED_REFERENCES.join('`, `')}\`, plus the selected style preset when listed.
3. \`Read\` \`project.json\`, then follow \`generationContract\`, \`plan.design\`, and the assigned slide together. Do not re-research or change the planned title, claim, layout, or narrative role.
4. \`Write\` exactly one complete document to \`${file}\`: self-contained 960pt × 540pt HTML with inline CSS and audience-ready copy.
5. End with "SLIDE ${assigned} READY"; do not paste HTML into the reply.

Render rules:
- Apply every explicit user style setting from \`generationContract.userStyle\`.
- Keep the HTML compact: no HTML comments, no unused CSS rules, minimal whitespace and indentation. Density of CONTENT is good; padding of MARKUP is not.
${EDITABLE_PPTX_HARD_RULES}

Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;
  return body + buildStyleAppendix(input);
}

function buildLegacyPrompt(input) {
  const body = `Generate or revise a PPT Live deck. The user only sees the PPT Live app UI.

1. Call \`Skill('${PPT_DESIGN_SKILL_KEY}')\` and verify the returned stable key. Never substitute a user or project skill.
2. \`Read\` every mandatory reference \`${PPT_DESIGN_REQUIRED_REFERENCES.join('`, `')}\` and the selected style preset reference. If this deck has a project file, \`Read\` \`project.json\` and preserve its generationContract.
3. Use research tools only when the edit introduces factual claims not already grounded by the deck.
4. Finish with only one strict JSON object.

Every slide must include complete \`slides[].html\`: self-contained 960pt × 540pt HTML with inline CSS (ppt-design editable PPTX rules). Slide copy must be audience-ready, never placeholder instructions.
${EDITABLE_PPTX_HARD_RULES}

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
 * - "slides": render the assigned slide. With `input.inSession` the turn runs
 *   inside the planning session and relies on its context; otherwise it gets a
 *   self-contained prompt that reloads the skill and reads the plan JSON.
 * - absent: legacy single-shot protocol (full deck or incremental patch).
 * `input.continueAfterInterruption` marks reruns of an interrupted turn.
 * `input.completionRecovery` is a stronger host-verified continuation after
 * the normal stage retry budget has been exhausted.
 */
function buildAgentPrompt(input) {
  let prompt;
  if (input?.phase === 'plan') prompt = buildPlanPrompt(input);
  else if (input?.phase === 'slides') {
    prompt = input?.inSession ? buildSessionSlidePrompt(input) : buildSlidesPrompt(input);
  } else prompt = buildLegacyPrompt(input);
  if (input?.completionRecovery) {
    prompt = buildCompletionRecoveryPrefix(input.completionRecovery) + prompt;
  } else if (input?.continueAfterInterruption) {
    prompt = CONTINUE_AFTER_INTERRUPTION_PREFIX + prompt;
  }
  return prompt;
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
    // Staged generation delivers through project files written by the agent
    // (ppt-design's native workflow); 'files' tells ui.js to read them back.
    protocol: 'files',
    async call(action, input, options = {}) {
      if (action !== 'ppt.generate') {
        throw new Error(`Unsupported PPT Live action: ${action}`);
      }
      ensureAgentEvents();
      const prompt = buildAgentPrompt(input);
      const result = await app.agent.run(prompt, {
        runId: options.idempotencyKey,
        sessionName: 'PPT Live',
        // Reuse the planning session when the caller carries one, so render
        // and continue turns see the loaded skill/preset/research context.
        sessionId: options.sessionId,
        // Staged turns work inside a dedicated deck project directory under
        // the app's own appdata storage (never the user's workspace).
        appDataWorkspace: options.appDataWorkspace,
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

// ─── Install ─────────────────────────────────────────────────────────────────

export function installBitFunBackendAdapter(app = window.app) {
  if (!app || app.backend?.call) return;
  if (app.agent?.run) {
    installAgentBackend(app);
  }
}
