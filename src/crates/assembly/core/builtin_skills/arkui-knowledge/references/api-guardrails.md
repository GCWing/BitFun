# ArkUI API Guardrails

Use these rules before writing ArkUI component constructors, modifiers, enum values, or callbacks.

## Component constructors

- `Tabs(options?)` accepts options including `barPosition`, `index`, and `controller`.
- `TabContent()` takes no object parameter; set the label with `.tabBar(...)`.
- `List({ space })` and `Grid()` contain their required item child components.
- `Row({ space })` and `Column({ space })` take spacing in the constructor.
- `Flex()` does not take a `space` option; use margins on children when needed.
- `Stack({ alignContent })` controls stack alignment; do not use row/column alignment methods on it.

## Modifier names

Use full ArkUI modifier names:

- `.backgroundColor(...)`
- `.borderRadius(...)`
- `.fontSize(...)`
- `.fontColor(...)`
- `.fontWeight(...)`
- `.textAlign(...)`
- `.objectFit(...)`
- `.textOverflow(...)`

Do not use shorthand names from other UI systems, including shortened background, radius, text size, or text color modifiers. Do not use Android layout constants for width or height.

## Modifier ownership

- `Text` owns text modifiers including `.fontSize(...)`, `.fontColor(...)`, `.fontWeight(...)`, `.textAlign(...)`, `.maxLines(...)`, and `.textOverflow(...)`.
- `Image` owns image modifiers including `.objectFit(...)`.
- `Column` and `Row` own layout alignment, but their alignment enum types differ.
- `Button` label styling is often clearer by placing styled `Text` inside `Button` when project style requires custom text styling.
- `List` owns list-level direction, edge, divider, and scroll behavior; `ListItem` owns per-row content.

## Parameters and callbacks

- `margin` and `padding` use a number, string, or object with edge names. Do not use multi-argument CSS-style shorthand.
- `AlertDialog` button entries use `{ value: '确定', action: () => {} }`.
- `ActionSheet` entries use `sheets`.
- Popup state-change callbacks receive an event object with visibility state.
- Event callback parameter types should be explicit when inference is unclear.

## Enums and resources

- Use ArkUI enum names with exact casing, for example `TextAlign.Center`, `FontWeight.Bold`, and `BarPosition.End`.
- Do not pass string values where an enum is expected.
- Do not guess system symbol or system color names. Use existing project resources or verified names.

## UIContext-sensitive APIs

When a project already uses UIContext for UI operations, follow it for:

- Toast prompts through `this.getUIContext().getPromptAction()`.
- Alert dialogs through `this.getUIContext().showAlertDialog(...)`.
- Router access through `this.getUIContext().getRouter()`.
- Animation through `this.getUIContext().animateTo(...)`.

If the current project has a wrapper for routing, dialog, toast, or logging, use the wrapper instead of introducing a new pattern.
