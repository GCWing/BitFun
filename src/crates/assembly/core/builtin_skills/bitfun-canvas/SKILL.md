---
name: bitfun-canvas
description: >-
  A BitFun Canvas is a live React app that the user can open beside the chat.
  You MUST use a canvas when the agent produces a standalone analytical artifact
  — quantitative analyses, billing investigations, security audits, architecture
  reviews, data-heavy content, timelines, charts, tables, interactive
  explorations, repeatable tools, or any response that benefits from visual
  layout. Especially prefer a canvas when presenting structured tool or service
  results where the data is the deliverable — render it in a rich canvas rather
  than dumping it into a markdown table or code block. If you catch yourself
  about to write a markdown table, stop and use a canvas instead. You MUST also
  read this skill whenever you create, edit, or debug a BitFun Canvas artifact.
metadata:
  surfaces:
    - ide
---
A canvas is a single TSX source artifact that BitFun compiles so the user can open it beside the chat. Follow the workflow below in order.

## Workflow

### 1. Decide whether to use a canvas

The trigger is **user intent**, not response shape. Ask: would the user benefit from viewing this output as its **own standalone artifact**, separate from the chat? If the output is a means to an end (a drafted message, a code fix, a dashboard in another tool), skip the canvas.

**Use a canvas when the agent produces new standalone analytical output:**
- Quantitative analyses and metrics breakdowns (e.g. "send 500 requests and tell me how many fail")
- Billing or account investigations that surface structured findings from database queries
- Security audits or architecture reviews with categorized findings
- Cross-system data analyses and overlap reports
- Structured data from tools or services where the data IS the deliverable
- Financial analyses, margin decompositions, usage trend reports
- Tables with more than a handful of rows that the user asked to see

**Do NOT use a canvas when:**
- The user asks for work in a **specific tool** — "create a Datadog dashboard" means give them a Datadog dashboard, not a canvas
- The user has a **specific deliverable** — "draft a support response", "fix this code", "make this PR"
- The user is **working within an existing artifact** — improving an HTML dashboard, editing an existing file
- The user is doing **targeted debugging** or active development, even if structured findings emerge along the way
- Short factual answers, one-off file edits, or quick clarifying questions
- Tools are queried as an **intermediate step** for a different deliverable (e.g. querying Stripe to draft a support reply)

### 2. Write the canvas

**Location.** BitFun Canvases are session-scoped artifacts created with `CreateCanvas`. For a new canvas, always call `CreateCanvas` with a concise title and complete TSX source; do not stop after telling the user what the source would be or showing code in chat. For small targeted revisions, use `PatchCanvas` with exact unique text replacements. Use `ReadCanvas` first when you do not have the latest source in context. Use `UpdateCanvas` only for large rewrites that need a complete replacement TSX source.

**File rules:**
- Exactly one TSX source per canvas. Never create helper files, style files, or supporting modules.
- Import **only** from `bitfun/canvas`. No relative imports, no npm packages, no Node built-ins.
- Default-export the top-level component.
- Embed all data inline. **No `fetch()`, no network calls.**
- Do not locally redeclare names imported from `bitfun/canvas` such as `Grid`, `Code`, `Stack`, `Text`, `Row`, or `Card`. Use the SDK component directly, or choose a distinct helper name such as `LayerGrid` or `InlineCodeText`.

**Never render empty states.** A canvas exists to show real content. If a section, chart, table, or component has no data to display, **omit it** — do not render it with placeholder text ("Add header here", "TODO", "Example"), a "No data" message, an empty array, zeroed rows, or an empty chart frame. If the entire canvas would be empty because you don't have the underlying data, do not produce a canvas — tell the user what's missing and ask for it instead.

**Label every plot.** Charts and tables must be self-describing — a reader looking at the canvas alone should know exactly what they're seeing. For every plot include:
- A title naming the **specific metric** (not "Metrics" — "API error rate by service").
- **Axis labels with units** on both axes (e.g. "Date", "Latency (ms)").
- A **legend** when more than one series is shown, with the exact series names from the source data.
- The **source and time range** in a small caption (e.g. "Source: Datadog · last 7 days"). If a value is a transformation (mean, p95, normalized, smoothed), say so in the label.

**Component discovery:** prefer built-in `bitfun/canvas` components over hand-rolled markup. The full public surface (components, hooks, prop types, tokens) is declared in `sdk/index.d.ts` next to this skill and its sibling `.d.ts` files — read them when you need exact exports, prop shapes, or hook signatures rather than guessing. Referencing an export that does not exist is the most common runtime error.

Apply the Canvas generation policy below as you write, and complete its pre-delivery self-check (section 6) before returning the canvas.

## Design guidance

Be creative. The SDK gives you expressive building blocks — use them in whatever combination best serves the content. But avoid slop: no gradients, no emojis, no box-shadows, no rainbow coloring. BitFun canvases are flat, minimal, and purposeful.

### Visual hierarchy

Not everything deserves equal treatment. Primary content gets more space, larger headings, and accent color. Supporting content stays compact. Squint test: blur your eyes — can you tell what matters?

**Color.** All colors from `useHostTheme()` tokens — read its JSDoc in the SDK declarations for the return shape and usage pattern. No hardcoded hex. Use accent color deliberately, not on everything.

### Slop patterns — forbidden

These specific patterns produce low-quality output. If 2+ are present, redesign.

- **Gradients** — no `linear-gradient`, `radial-gradient`, `background-clip: text`.
- **Emojis** — no emoji as icons, status indicators, bullets, or section markers.
- **Box shadows** — no `box-shadow`. Flat surfaces only.
- **Wall of identical cards** — every section wrapped in the same card style with no variation. Mix open sections with cards.
- **Rainbow coloring** — a different color on every element. Most elements are neutral; color is used sparingly with purpose.
- **Giant text** — font sizes above H1 (24px), or bold text stuffed in CardHeader.
- **Decorative borders** — colored borders on every element. Borders are structural (subtle stroke tokens), not decorative.

### Pre-delivery self-check

Before returning canvas code, verify:
1. Does the layout have visual hierarchy? One thing should stand out.
2. Is there variety in the composition? Not just a single column of uniform blocks.
3. Slop check: scan for the forbidden patterns above.

## Introducing the canvas

Whenever you mention a canvas to the user — one you created, updated, or want them to open — **always** include the `bitfun-canvas://...` artifact reference returned by the Canvas tool. Use the artifact title or a short descriptive label near the reference; do not refer to a canvas by name alone without the reference.

When you create a canvas, add a short note in your chat response telling the user they can open it beside the chat, with that `bitfun-canvas://...` reference:

- **First canvas** — include one sentence explaining what a canvas is.
- **Unsolicited canvas** — if the user didn't ask for a canvas, include one sentence explaining why you chose it over plain text.

Both can apply at once; one or two sentences total is enough. Skip the intro for subsequent canvases unless you are mentioning that canvas again (still include the artifact reference).

## Troubleshooting

If a canvas appears blank or missing, first inspect the Canvas tool result diagnostics and runtime diagnostics. `CreateCanvas`, `PatchCanvas`, and `UpdateCanvas` save the source first, then compile it. If compilation or policy validation fails, the tool returns diagnostics and preserves the previous last-known-good compiled payload when one exists. Runtime errors are reported back to the host as Canvas diagnostics; open the source view, fix the exception, then call `PatchCanvas` for a small exact edit or `UpdateCanvas` for a full rewrite.
