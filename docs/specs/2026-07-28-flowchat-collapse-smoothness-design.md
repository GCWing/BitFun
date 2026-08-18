# FlowChat Collapse Smoothness & Flash Fix

**Date:** 2026-07-28  
**Status:** Approved for implementation (user authorized full design + implement; approach C)

## Problem

During FlowChat streaming, collapsible UI (thinking, explore groups, tool cards,
subagent, terminal, etc.) often expands then collapses. Two user-visible defects:

1. Collapse feels abrupt — content vanishes instead of easing away.
2. The whole pane can jump / flash as if reloaded (also mid-stream).

## Root Causes

1. **Rule Zero instant auto-collapse.** Automatic expand/collapse was forced to
   0ms (`--instant` / `disableAnimation`) so scroll compensation would not chase
   a multi-frame height change. That removed jitter at the cost of abrupt UX.
2. **Opacity leads height.** `SmoothHeightCollapse` animates height ~260ms but
   opacity ~180ms, so content fades out before the box finishes closing.
3. **Hard shell swaps.** Terminal / ExecProcess / Git toggle between
   `BaseToolCard` and `CompactToolCard`, unmounting expanded UI with no height
   transition.
4. **Intent settles too early for animation.** Auto collapse-intent finalizes
   after ~4 rAF frames (~64ms), far shorter than a real height transition.
5. **Projection identity churn.** `hasActiveStreamingNarrative` defers
   explore-group projection until the narrative settles, swapping Virtuoso keys
   (`model-round` → `explore-group`) and remounting visible content.

## Goals

- Auto-collapse uses a single smooth height animation (~300ms) with opacity and
  transform on the same duration / easing.
- Scroll compensation tracks the full animation window; no drop-then-snap.
- Thinking / Explore / FileOp / Task+Subagent / Terminal / ExecProcess / Git
  share one collapse contract.
- Live explore projection identity stays stable from first explore-capable
  render through completion.
- Prefer-reduced-motion still disables animation.

## Non-Goals

- No framer-motion.
- No change to when content *should* collapse (`isLastItem`, `wasCutByCritical`, etc.).
- No mount / `--streaming`→`--complete` enter animations (virtualization remount risk).
- No Rust / mobile-web changes.

## Solution

### 1. Shared collapse timing contract

New module `src/web-ui/src/flow_chat/components/modern/flowChatCollapseMotion.ts`:

| Constant | Value | Role |
|---|---|---|
| `FLOWCHAT_COLLAPSE_DURATION_MS` | `300` | Height / opacity / transform duration |
| `FLOWCHAT_COLLAPSE_EASING` | `cubic-bezier(0.4, 0, 0.2, 1)` | Shared easing |
| `FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES` | `4` | Extra rAF after animation before intent finalize |

### 2. `SmoothHeightCollapse`

- Default `durationMs = FLOWCHAT_COLLAPSE_DURATION_MS`.
- Inline + SCSS transition durations: height, opacity, and transform all use
  `durationMs` (no shorter opacity channel).
- Keep reverse-from-current-height behavior and `--instant` for reduced-motion /
  explicit `disableAnimation`.

### 3. Enable animated auto-collapse (revise Rule Zero §4)

Update `FLOWCHAT_SCROLL_STABILITY.md`:

- Automatic collapse **may** animate when a collapse-intent is active for the
  full `FLOWCHAT_COLLAPSE_DURATION_MS` (+ settle frames).
- Instant collapse remains only for `prefers-reduced-motion` or explicit
  `disableAnimation` during live open growth where needed.
- Remove the “auto = one frame only” requirement from Thinking / Explore /
  FileOperation / BaseToolCard call sites.

Concrete call-site changes:

- `ModelThinkingDisplay`: stop applying `--instant` on auto toggles; use the
  shared 300ms grid transition.
- `ExploreGroupRenderer`: animate auto cut; do not gate on `animateToggle`;
  only skip animation while the group is open and still streaming content growth
  if measurement requires it — collapse itself always animates.
- `FileOperationToolCard`: stop setting `disableExpandAnimation` for auto.
- Task / Subagent already animate; align `durationMs` to the shared constant.

### 4. Collapse-intent lifetime tracks animation

In `VirtualMessageList.scheduleCollapseIntentFinalization`:

- For `reason === 'auto'`, wait `FLOWCHAT_COLLAPSE_DURATION_MS`, then run the
  existing settle-frame finalizer (not settle-only).
- Keep TTL (1000ms) as hard backup.
- When a new intent arrives while one is active: **coalesce** — extend TTL,
  add provisional shrink, update/preserve semantic anchor — instead of
  finalizing the previous intent (which can briefly drop protection).

### 5. Stable explore projection identity

Remove `hasActiveStreamingNarrative` deferral from:

- `sessionToVirtualItems` / `isExploreOnlyRound`
- `buildModelRoundItemGroups` (`deferExploreGrouping` only from
  `disableExploreGrouping`)

Keep `isActiveToolItem` so *running* explore tools remain critical / visible
until they complete, then merge without a virtual-item type swap for the parent
round. Typewriter remount risk remains covered by `replayOnMount: false`.

### 6. Eliminate hard shell swaps

`TerminalToolCard`, `ExecProcessToolCardView`, and `GitToolDisplay` always render
`BaseToolCard` and collapse body via `SmoothHeightCollapse` (already inside
`BaseToolCard`). Do not conditional-mount `CompactToolCard` for expand/collapse
transitions. Compact visual cues may remain as CSS modifiers on the same shell.

### 7. Tests

- `SmoothHeightCollapse`: opacity/height share duration; auto path animates.
- Store / grouping: streaming narrative + explore tools keep explore-group
  identity (no mid-settle type flip).
- Collapse-intent scheduling helpers / scroll stability: auto intent protects
  for at least `FLOWCHAT_COLLAPSE_DURATION_MS`.
- Existing session-boundary and store projection tests updated if expectations
  change.

## Verification

```bash
pnpm run type-check:web
pnpm --dir src/web-ui run test:run \
  src/flow_chat/components/modern/SmoothHeightCollapse.test.tsx \
  src/flow_chat/store/modernFlowChatStore.test.ts \
  src/flow_chat/components/modern/modelRoundItemGrouping.test.ts \
  src/flow_chat/components/modern/VirtualMessageList.session-boundary.test.tsx \
  src/flow_chat/tool-cards/useToolCardHeightContract.test.tsx
```

Manual: stream a turn with thinking → explore tools → write/edit → task/subagent
→ terminal; confirm smooth auto-collapse and no whole-pane flash/jump.

## Related files

- `src/web-ui/src/flow_chat/components/modern/FLOWCHAT_SCROLL_STABILITY.md`
- `src/web-ui/src/flow_chat/components/modern/SmoothHeightCollapse.{tsx,scss}`
- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.tsx`
- `src/web-ui/src/flow_chat/components/modern/ExploreGroupRenderer.tsx`
- `src/web-ui/src/flow_chat/components/modern/modelRoundItemGrouping.ts`
- `src/web-ui/src/flow_chat/store/modernFlowChatStore.ts`
- `src/web-ui/src/flow_chat/tool-cards/ModelThinkingDisplay.{tsx,scss}`
- `src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx`
- `src/web-ui/src/flow_chat/tool-cards/TerminalToolCard.tsx`
- `src/web-ui/src/flow_chat/tool-cards/ExecProcessToolCardView.tsx`
- `src/web-ui/src/flow_chat/tool-cards/GitToolDisplay.tsx`
- `src/web-ui/src/flow_chat/components/subagent/SubagentProjectionView.tsx`
