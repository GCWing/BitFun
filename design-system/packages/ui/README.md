# @openbitfun/ui

Theme-independent React primitives and components.

```tsx
import "@openbitfun/theme-openbitfun/default.css";
import "@openbitfun/ui/styles.css";
import { Button, ThemeRoot } from "@openbitfun/ui";

export function Example() {
  return (
    <ThemeRoot colorScheme="light" density="comfortable">
      <Button>Continue</Button>
    </ThemeRoot>
  );
}
```

The package owns component anatomy, behavior, accessibility, and stable variants. It does not own theme selection persistence, product state, routes, locale resources, or platform APIs.

## Mobile controls

Touch-first controls use the isolated mobile entry so compact and foldable
surfaces do not inherit desktop component geometry or ship desktop component
styles:

```tsx
import "@openbitfun/ui/mobile.css";
import {
  MobileActionSheet,
  MobileBadge,
  MobileBanner,
  MobileButton,
  MobileCard,
  MobileChoiceSheet,
  MobileConfirmSheet,
  MobileComposer,
  MobileDisclosure,
  MobileFileButton,
  MobileFloatingActions,
  MobileIconButton,
  MobileLink,
  MobileListRow,
  MobileMessage,
  MobilePageHeader,
  MobileScrim,
  MobileSection,
  MobileSegmentedControl,
  MobileSheet,
  MobileStatus,
  MobileTextField,
  MobileTextarea,
} from "@openbitfun/ui/mobile";
```

These components own mobile touch targets, pressed/focus/disabled states,
surface elevation, responsive inline sizing, composer geometry, transparent
floating action layout, and sheet accessibility. Product state, localized copy,
routing, and device or session operations stay in the consuming application.

Use `OverflowText` for single-line labels that need a treatment only when their
rendered content is actually clipped. Its default `fade` behavior softens the
inline end. `behavior="marquee"` keeps that resting cue, then reveals the full
label with a measured hover/focus marquee; reduced-motion users keep the static
fade. The primitive preserves the full text in the accessibility tree, supports
right-to-left direction, and leaves width constraints and tooltip content to
the consumer.

`Disclosure` is the shared expandable-content primitive. It owns controlled or
uncontrolled open state, trigger/region accessibility wiring, focus exclusion
while collapsed, reduced-motion behavior, and independent header actions.
Product copy and the revealed content remain consumer-owned.

Sized icon slots in buttons, tabs, menu items and fields own their glyph geometry.
Pass catalog `Icon` nodes through `leadingIcon`, `trailingIcon`, `icon` or the
matching component slot, just as for SVG icons. These slots constrain catalog
icons to the component's size; a standalone `Icon` retains its explicit size
(24px by default). Do not shrink the catalog globally to correct a slot mismatch.

`IconButton` defaults to `quiet`: its resting surface is transparent, hover and
pressed states use shared action feedback, and keyboard focus keeps a visible
focus ring. Use it for toolbar, dialog, and row utilities. `fill` and `primary`
keep an opaque backing surface for persistent emphasis. Disabled quiet actions
remain transparent and do not show hover or pressed feedback.

The catalog uses exported vectors, including their view boxes and per-path
opacity. Theme colors remain caller-owned through `currentColor`. Asset
fingerprints are reviewed with intentional resource updates so replacing a
glyph with a similarly named substitute cannot pass unnoticed.

Use `canonicalIconNames` for galleries and pickers. `iconNames` also keeps the
legacy `download`, `circle` and `turn` entries for compatibility; prefer
`arrow-down`, `unselected` and `<NumberBadge value={18} />` respectively.
`turn` is only the old empty background, not a complete numbered marker.
`NumberBadge` owns a 24px slot, a 20px surface and 11px medium text; longer
values grow horizontally. Callers supply formatted values and contextual
accessible labels. `ToolbarBadge` delegates to the same anatomy.

Use `Icon name="session"` in new consumers. `SessionIcon` retains its SVG
interface for existing integrations, with geometry checked against the same
catalog asset.

## Advanced selection and menus

Use native `Select` for simple options. `Combobox` adds searchable single
selection, grouped options, explicit custom-value creation and async loading
states. `MultiSelect` owns multiple selection, removable tags and select-all.
Controlled values are authoritative; option discovery remains host-owned.
Wrap the product once in `DesignSystemProvider` to supply translated messages,
the portal host, theme facts and the shared overlay layer stack.
The Web UI's legacy Select implementation is retired. Like retired Button and
Switch overrides, legacy `components.select` Appearance rules are ignored at
the existing read-only migration boundary; original packages are not rewritten.
Selection visuals now come from the public field/menu semantic tokens.
`FieldGroup fieldSurface="ambient"` keeps text and picker field borders while
letting their shells reuse the grouped surface. The default field surface stays
theme-owned, and portalled menus remain on the opaque panel surface.

`Menu` remains composable inline anatomy. `MenuPopover` composes it into a
controlled anchored or coordinate popup. Pass `items`, `open`, `onClose` and
either `anchorRef` or `position`. Entries can include `submenu`, `shortcut`,
`disabled`, `checked`/`role`, and `onSelect`. Activation closes the tree and
restores focus before dispatching `onSelect`; the host owns asynchronous work
and error handling. The popup flips and clamps to the viewport, keeps keyboard
navigation in the active menu, and supports safe pointer travel to either side.

Portals resolve through `DesignSystemProvider.portalHost`, then fall back to the
nearest design-system root. Stable `parts` wrappers preserve host data hooks;
they must forward all props and refs and retain public component ownership.
`useSubmenuIntent` is available for product popovers that need the same pointer
corridor behavior.

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
  ProminentToolCardSummary,
  ReadFileToolCard,
  ToolCardCopyButton,
  ToolCardChangeSummary,
} from "@openbitfun/ui/flow-chat";
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
