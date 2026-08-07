# HarmonyOS App Instructions

These rules apply to all changes under `src/apps/mobile/harmonyos`.

## Visual reference fidelity

- Before drawing a system glyph, text approximation, or new bitmap, search the existing HarmonyOS media resources and the approved desktop reference images. Reuse the established asset when one exists.
- Conversation header controls must use the approved `remote_ref_back` and `remote_ref_more` assets. Do not replace them with a system chevron or text such as `...` / bullet characters.
- Render monochrome reference assets in template mode and tint them with semantic theme colors such as `INK`. Never rely on the bitmap's original black or white pixels; the same control must remain legible in light and dark themes.
- Keep paired header controls on the same fixed touch-target size and optical alignment. A responsive layout may reposition a control, but must not silently change its icon geometry or visual weight.
- Keep `SymbolGlyph` geometry separate from its touch target. When a glyph is clickable or sits in a decorated control, wrap it in `Stack({ alignContent: Alignment.Center })` (or use a centered `Button`) and give the glyph its visual size; do not stretch the glyph itself to the full 32vp/40vp/44vp target, because font metrics can make the icon look off-center.

## Responsive interaction semantics

- Wide and compact layouts must keep the same interaction meaning. Responsive presentation may change spacing and available width, but it must not turn a lightweight anchored action menu into a bottom sheet by default.
- Conversation-header overflow actions open from the top-right trigger as an anchored popover on both compact and wide layouts. Use a bottom sheet only when the content is a genuinely large or multi-step mobile workflow and the design explicitly calls for it.
- Anchor popovers to their actual trigger with `bindPopup` or the equivalent platform API. Do not emulate the anchor with unrelated page-level absolute positioning.
- Preserve auto-dismiss, outside-tap handling, accessibility labels, and a short enter/exit transition for anchored menus.

## Theme and device verification

- Use existing semantic colors from `Theme.ets`; do not hard-code a light-only foreground or surface color.
- For changes to navigation controls, menus, or responsive presentation, verify compact and wide behavior, light and dark theme legibility, and capture a real-device screenshot before completion when a device is connected.
- Run the smallest matching HarmonyOS build/check plus `pnpm run theme:color-audit:all` for theme or color-related changes.
