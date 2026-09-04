# Cinematic Animated Wallpaper Playbook

## Style identity

Use one animated character wallpaper as the visual source of truth:

- complete motion in the main workspace;
- a tighter static character crop in floating chat;
- one expressive still frame for every sidebar;
- darkened face, weapon, clothing, energy, or environmental details in cards and dialogs;
- source-derived deep surfaces, a distinct primary accent, and separate semantic status colors;
- no obvious borders and no layout changes.

Prefer a frame near the visual climax for still artwork. Keep source composition intact for the main background. Use the bundled cold palette only as a fallback; artwork with a strong red, gold, green, or neutral identity should drive its own semantic palette.

## Asset roles

| Asset | Motion | Placement | Treatment |
| --- | --- | --- | --- |
| `background.webm` | complete animation | top-level `backgroundMedia` | host-managed, source-faithful, cover |
| `floating.webp` | one frame | `toolbar-mode.root`, `floating-mini-chat.panel` | centered character crop |
| `sidebar.webp` | one frame | all registered sidebar owners | portrait crop, moderately darkened |
| `card-detail.webp` | one frame | compact cards, messages, terminal strip | lower/detail crop, strongly darkened |
| `card-portrait.webp` | one frame | hero cards, MiniApp/Agent cards | face/character crop, strongly darkened |
| `dialog.webp` | one frame | registered dedicated dialog surfaces | wide crop, strongly darkened |
| `preview.webp` | one frame | package preview | representative full frame |
| `asset-preview-sheet.png` | generated report | outside the package | labeled overview of all six static roles |

Keep `background.webm` at or below 64 MiB, every ordinary image at or below 16 MiB, `preview.webp` at or below 4 MiB, and the archive at or below 96 MiB. The host also limits background video to 60 seconds, 4096 pixels per side, and 9 million pixels. Prefer WebM for cross-platform codec availability; final OpenBitFun import verifies actual media metadata and host playback support.

## Surface matrix

Re-query every id before use.

| Purpose | Registered owners |
| --- | --- |
| complete animation | top-level `backgroundMedia`; keep scene `workbench.workspace` transparent |
| expanded/static navigation | scene `workbench.navArea`, component `nav-panel.root` |
| collapsed navigation control | transparent scene `workbench.collapsedNav`, translucent collapsed state on `nav-bar.root` |
| Skills sidebar | scene `skills.sidebar` |
| Settings sidebar | component `settings-nav.root` |
| terminal navigation | component `shell-nav.root` |
| file/auxiliary sidebars | `files-panel.root`, `flexible-panel.root`, `btw-session-panel.root`, `aux-pane.root`, scene `session.auxiliary` |
| bottom terminal art | `bottom-terminal-pane.root` with `card-detail.webp` |
| detached Toolbar Mode still artwork | `toolbar-mode.root` |
| detached Toolbar Mode inner veil | `toolbar-mode.content` |
| in-app floating mini-chat still artwork | `floating-mini-chat.panel` |
| shared floating chat transparency | `floating-mini-chat.body`, `modern-flow-chat.root`, `modern-flow-chat.messages`, `virtual-message-list.root` |
| conversation turn navigation | `flow-chat-turn-rail.root`, `.list`, `.item`, `.bar`, and tooltip Parts |
| copyable command text | `copyable-text-preview.root`, `.empty`, tooltip Parts, and `.copyAction` |
| settings transparency | `config.root`, `config.content`, `config.contentInner`, `archived-sessions-config.root`, `keyboard-shortcuts.root`, `keyboard-shortcuts.content` |
| translucent settings cards | `config.sectionBody` |
| Insights transparency | scene `insights.root`, `insights.content` |
| image-led cards | `mini-app-card.root`, `agent-card.root`, `core-agent-card.root`, `skill-card.root`, `user-message-item.content` |
| dialogs | registered dialog content, root, or body Parts returned by the current contract |

## Cascade rules

Host styles often use the `background` shorthand. A normal Appearance `backgroundColor` or `backgroundImage` declaration can be reset by that shorthand depending on runtime order and specificity.

Use `cascade: "override"` only on the registered Part that owns the conflicting paint. Typical owners for this style:

- `toolbar-mode.root` and `.content`;
- `nav-bar.root` when its host background shorthand hides the expanded navigation art or the collapsed glass state;
- `workbench.collapsedNav` when an earlier package applied sidebar artwork to the small floating owner;
- `floating-mini-chat.panel` and `.body`;
- `modern-flow-chat.root` and `.messages`;
- `virtual-message-list.root`;
- `config.root`, `.content`, `.contentInner`, and `.sectionBody`;
- `archived-sessions-config.root`;
- `keyboard-shortcuts.root` and `.content`;
- `insights.content`;
- `chat-pane.root` when the host scene background remains opaque.

Do not use override for layout properties. The compiler only marks descriptor-approved forceable paint properties as important.

## Material recipe

Use reusable materials instead of repeating paint blocks:

- `panel`: translucent deep surface plus 4-8 px backdrop blur;
- `card`: translucent surface plus a soft shadow;
- `control`: slightly denser surface, small existing radius, restrained glow;
- `detail-card`: `card-detail.webp`, cover/center, dark surface, existing radius;
- `portrait-card`: `card-portrait.webp`, cover/center, dark surface, existing radius;
- `dialog-art`: `dialog.webp`, cover/center, dense surface, existing dialog radius;
- `popup`: near-opaque surface for menus that need reliable readability;
- `code`: opaque surface for code and terminal text.

Do not add explicit border widths. When a host border remains visually strong, override only `borderColor` with transparent or a very low-alpha accepted color.

## Renderer palette

Define source-specific semantic colors using [palette-contract.md](palette-contract.md). The scaffold keeps text and accent tokens opaque while deriving translucent scene, glass, card, and border values.

For the default cold cinematic fallback, representative generated values are:

```json
{
  "--openbitfun-color-surface-scene": "rgba(8, 17, 31, 0.08)",
  "--openbitfun-color-surface-raised": "rgba(24, 43, 60, 0.72)",
  "--openbitfun-color-surface-subtle": "rgba(11, 25, 40, 0.52)",
  "--openbitfun-color-action-quiet-hover": "rgba(16, 34, 53, 0.60)",
  "--openbitfun-color-action-neutral-surface": "rgba(22, 43, 61, 0.66)",
  "--openbitfun-color-action-neutral-surface-pressed": "rgba(23, 54, 74, 0.74)",
  "--openbitfun-color-border-subtle": "rgba(82, 229, 245, 0.08)",
  "--openbitfun-color-border-default": "rgba(82, 229, 245, 0.12)",
  "--openbitfun-color-border-strong": "rgba(82, 229, 245, 0.18)"
}
```

Tune the RGB values to the source palette while preserving the alpha hierarchy.

Insights report cards currently derive their local surfaces from `--openbitfun-color-surface-raised` and canonical action/border tokens. They do not expose individual registered Parts. Keep `insights.content` transparent and use these tokens for glass-card behavior.

## Runtime failure diagnosis

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| detached floating window has no artwork | the package styled `floating-mini-chat`, but the window is `toolbar-mode` | apply static `floating.webp` to `toolbar-mode.root` with override and keep `.content` translucent |
| in-app floating mini-chat has no artwork | host `background` shorthand reset the asset | add override to the panel and transparent inner chat owners |
| main video is hidden | `workbench.workspace` or another scene owner still paints an opaque background | make the registered workspace and structural scene owners transparent; keep the video only in top-level `backgroundMedia` |
| main video shows a still image | reduced motion is enabled or video playback/codec validation failed | confirm OS/browser motion preference, inspect import errors, and use WebM VP9 with `preview.webp` as poster |
| collapsed navigation has an opaque 80x40 block | child `nav-bar.root` paints `--openbitfun-color-surface-canvas` | force the base root transparent and use a low-alpha collapsed-state glass material |
| collapsed navigation looks like a pasted image tile | `sidebar.webp` was applied to the small `workbench.collapsedNav` owner | remove the image, keep the scene owner transparent, and let the collapsed `nav-bar.root` provide quiet glass contrast |
| whole Insights history has an image | artwork was applied to `insights.content` | make content transparent; style cards through tokens |
| archived sessions or shortcuts are opaque | specialized ConfigPage root replaced the generic `config.root` attribute | override the specialized root Part directly |
| background disappears only inside chat | `modern-flow-chat.root` still paints `color-bg-scene` | make token translucent and/or force root transparent |
| chat input has a second rounded outer frame | `control` or card material was applied to the structural `chat-input.container` wrapper | remove wrapper decoration and leave `chat-input.box` as the single visual input surface |
| project validates but runtime is wrong | validator checked structure, not cascade outcome | import and inspect screenshots; then add the smallest paint override |

## Runtime acceptance checklist

- Main workspace animation remains visible during calm and bright frames.
- Reduced-motion mode shows the poster without autoplaying video.
- Text remains readable without making every surface opaque.
- Sidebars use still artwork, never independent animation.
- Expanded navigation remains transparent over its sidebar artwork; collapsed navigation uses quiet glass without a cropped image tile.
- Detached Toolbar Mode and in-app floating mini-chat use the intentional static character crop behind chat content and input chrome.
- Insights page background is transparent; individual cards remain distinct.
- Archived sessions and keyboard shortcuts expose the workspace through page gaps.
- No component changes size or position when the skin is applied.
- No visible one-pixel frame is introduced by the skin.
- Chat input has one visual surface; structural wrappers remain undecorated.
