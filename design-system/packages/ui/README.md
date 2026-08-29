# @bitfun/ui

Theme-independent React primitives and components.

```tsx
import "@bitfun/theme-bitfun/default.css";
import "@bitfun/ui/styles.css";
import { Button, ThemeRoot } from "@bitfun/ui";

export function Example() {
  return (
    <ThemeRoot colorScheme="light" density="comfortable">
      <Button>Continue</Button>
    </ThemeRoot>
  );
}
```

The package owns component anatomy, behavior, accessibility, and stable variants. It does not own theme selection persistence, product state, routes, locale resources, or platform APIs.

## FlowChat tool cards

FlowChat frameworks use an attention model rather than a size or border model:

- `AmbientToolCard` keeps routine tool traces lightweight and glanceable.
- `ProminentToolCard` gives attention-worthy results a framed summary, a stable
  left content region, hover/focus-revealed right actions, and controlled detail
  disclosure.

Import these components from the dedicated product-surface entry:

```tsx
import {
  AmbientToolCard,
  AmbientToolCardHeader,
  AskUser,
  ChatComposer,
  ChatComposerContent,
  ChatComposerEndActions,
  ChatComposerStartActions,
  CommandToolCard,
  ContextCompressionToolCard,
  FileOperationToolCard,
  ProminentToolCard,
  ProminentToolCardHeader,
  ReadFileToolCard,
  ToolCardChangeSummary,
} from "@bitfun/ui/flow-chat";
```

`ChatComposer` owns the reusable 32px context band and the compact/expanded
40px/120px input-surface anatomy. Product consumers keep their editor, menus,
model selection, voice input, sending, stores, and localized copy, and supply
them through `contextBar`, `startActions`, `endActions`, or the equivalent
compound slot components. The compound form is useful when a complex consumer
needs to keep those sections adjacent in source while the package still owns
the final DOM layout.

`AskUser` is the controlled question-and-answer interaction for FlowChat. It
owns native single- and multi-selection semantics, responsive option anatomy,
custom text input, submission feedback, and the answered disclosure summary.
Consumers keep question parsing, localized copy, draft persistence, and answer
submission outside the package and provide them through typed props.

Prominent headers keep information roles stable: `action` is the static primary
label, `content` is the secondary flexible subject, `extra` is right-aligned
dynamic metadata, and `actions` contains controls revealed on hover or keyboard
focus. Use `ToolCardChangeSummary` for added/removed counts; domain icons and
interaction affordances belong in `actions`, not in the summary.

Concrete tool-card views compose those frameworks without importing product
state. The published families cover file and command execution, search and web
results, agent and session activity, Git and review summaries, page lifecycle,
code execution, todos, images, and other routine tool traces. Each view owns its
card anatomy, status presentation, disclosure behavior, and action placement.

Tool-specific data shaping, localization, host actions, stores, and heavy
renderers remain in the consuming product and enter through semantic props,
callbacks, and slots. Bespoke product workflows remain product-owned rather
than being forced into a standard package view.
