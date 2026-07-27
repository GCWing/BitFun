---
name: arkui-knowledge
description: Load this skill when answering ArkUI UI questions or when writing/modifying ArkUI components, layouts, state-driven UI, rendering control, navigation, dialogs, interactions, component APIs, or ArkUI declarative UI in .ets files.
---

# ArkUI Knowledge

Use this skill for ArkUI UI knowledge and implementation guidance. It helps answer ArkUI questions and helps agents write correct, polished ArkUI code on the first pass.

## When to load

Load this skill when the task involves:

- ArkUI components, component modifiers, component nesting, or declarative UI structure.
- Layout with `Column`, `Row`, `Stack`, `Flex`, `Grid`, `List`, `Scroll`, `Tabs`, or `TabContent`.
- UI state refresh with `@State`, `@Prop`, `@Link`, `@Local`, `@Param`, `@Provide`, `@Consume`, or related decorators.
- Rendering control with `ForEach`, `LazyForEach`, conditional UI, builders, or reusable UI blocks.
- Navigation, dialogs, toast prompts, menus, gestures, animation, visual styling, or UI quality.
- Writing or modifying `.ets` files that render visible ArkUI surfaces.

Do not load this skill for:

- Plain ArkTS syntax restrictions with no UI component concern; use `arkts-grammar-standards`.
- Build or type errors after compilation fails; use `arkts-error-fixes`.
- Runtime crashes, white screens, jscrash logs, or uncaught exceptions; use `arkts-runtime-fix`.
- New project creation or empty project initialization; use `deveco-create-project`.

## Responsibilities

- Explain ArkUI concepts, APIs, component choices, and correct usage.
- Guide page and component structure while preserving the current project style.
- Prevent high-frequency ArkUI mistakes before code is written.
- Improve UI quality: visible required text, clickable required controls, stable layout, state refresh, and minimal unrelated edits.
- Keep ArkUI guidance separate from ArkTS language restrictions and post-build error repair.

## Before answering or coding

1. Identify the ArkUI topic: component, layout, state, rendering, navigation, dialog, interaction, animation, or visual quality.
2. For questions, answer directly, then add the correct usage, common trap, and applicable boundary.
3. For code changes, read the target `.ets` file first. Keep the existing state-management style, navigation style, directory style, and business flow.
4. Check the relevant reference before using a high-risk API:
   - `references/component-cookbook.md`
   - `references/api-guardrails.md`
   - `references/common-mistakes.md`
   - `references/ui-quality-checklist.md`
5. If a component signature, enum, callback parameter, or modifier owner is unclear and the local references do not cover it, prefer `devecocli docs search` + `devecocli docs read`, then inspect existing project usage before writing code.

## ArkUI component guardrails

- `Tabs` can contain `TabContent` directly. Build tabs with `Tabs(...) { TabContent() { ... }.tabBar(...) }`.
- Do not pass a `builder` object into `TabContent`; use `TabContent()` and set the label with `.tabBar(...)`.
- `ForEach` and `LazyForEach` key generators should return a stable string key from the item. Avoid `void` keys and index keys for business data.
- Place ArkUI state decorators only on component member declarations with the correct V1 or V2 decorator family. Do not mix V1 and V2 decorators in one component.
- Do not invent modifier names. Use full ArkUI names including `.backgroundColor()`, `.borderRadius()`, `.fontSize()`, and `.fontColor()`.
- Match modifiers to component owners. For example, text modifiers belong on `Text`, image fitting belongs on `Image`, and layout alignment differs by container.
- Prefer the existing navigation approach in the project. Do not replace router, `Navigation`, or custom app routers without a clear requirement.
- For dialogs, toast prompts, navigation, and animation, prefer valid UI context usage when the current project already follows that pattern.

## Common mistakes

Read `references/common-mistakes.md` before implementing UI with tabs, lists, decorators, dialogs, navigation, or custom builders.

High-risk mistakes to avoid:

- `TabContent` with a fake object parameter.
- `Tabs` containing direct non-`TabContent` children.
- `ForEach` key generator with a block body that does not return a string.
- `@State` on top-level variables, local variables, plain classes, or component inputs.
- `@ComponentV2` using V1 decorators including `@State`.
- Component modifiers borrowed from web, Android, other UI frameworks, or CSS shorthand.
- Dialog button fields with the wrong key names.
- Required UI text hidden by layout, overlay, tiny size, or unreachable navigation.

## UI quality checklist

Use `references/ui-quality-checklist.md` before finalizing UI work. At minimum:

- Required labels, buttons, cards, tabs, and dialog text are visible on the target screen.
- Required clicks update state, open the dialog, switch the tab, navigate, or show the expected response.
- New UI fits the current page density, spacing, color, and component style.
- Layout has stable dimensions where dynamic content could otherwise shift or overlap.
- The change is limited to files needed by the UI request.

## Boundaries with other skills

- Use `arkts-grammar-standards` for ArkTS language rules, TypeScript-to-ArkTS differences, template literals, dynamic property access, object literal typing, and syntax compliance.
- Use `arkts-error-fixes` only after compilation reports errors or when directly fixing build/type failures.
- Use `arkts-runtime-fix` for runtime stack traces, white screens, uncaught exceptions, and jscrash logs.
- Use `deveco-create-project` for project initialization.
- Do not perform state-management migration unless the user explicitly asks for that migration.
