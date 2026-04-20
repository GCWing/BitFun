# FlowChat Scroll Stability

This document explains the scroll-stability mechanism used by `VirtualMessageList.tsx`.

Read this before changing any of the following:

- footer height / footer rendering in `VirtualMessageList.tsx`
- scroll compensation state or refs
- anchor-lock timing
- `ResizeObserver` / `MutationObserver` / transition listeners
- `flowchat:tool-card-collapse-intent`
- `tool-card-toggle`
- `overflow-anchor` styles in `VirtualMessageList.scss`

## Problem

FlowChat uses `react-virtuoso` for virtualization. When the user is already at or near the bottom, collapsing content near the end of the list can shrink total content height.

Without compensation, the browser clamps `scrollTop` downward immediately because the previous bottom position no longer exists. That causes the visible header/content above to drop.

If we compensate too late, the user sees a flash:

1. browser clamps `scrollTop`
2. code restores `scrollTop`
3. header appears to drop and jump back

If we restore without enough compensation, the final position is still wrong.

The goal of this mechanism is:

- keep the visible header/content vertically stable
- allow temporary invisible blank space at the bottom
- avoid the collapse flash

## High-Level Strategy

The fix is a two-stage approach:

1. Pre-compensate before a known collapse starts.
2. Reconcile with the real measured height delta after layout updates.

This prevents the "drop first, restore later" behavior while still using the actual measured shrink amount to settle on the correct final compensation.

## Core Building Blocks

## 1. Bottom Reservations

The footer uses a unified bottom-reservation model. Each reservation contributes
temporary tail space, but keeps its own semantics:

- `collapse`: shrink protection for height loss near the bottom
- `pin`: viewport positioning space for "pin turn to top" navigation

The rendered footer height is the sum of all active reservations.

Important details:

- the real footer height is `MESSAGE_LIST_FOOTER_HEIGHT + totalBottomReservationPx`
- reservation space is not real content height
- reservations may define a `floorPx`
- only reservation space above the floor is consumable
- all measurements that compare old vs new content height must use:

```ts
effectiveScrollHeight = scroller.scrollHeight - getTotalBottomCompensationPx()
```

If you forget to subtract reservation space, future shrink/growth calculations become wrong.

`pin` reservations use this extra metadata:

- `targetTurnId`: which user turn the viewport should align to
- `mode: 'transient' | 'sticky-latest'`
- `floorPx`: the minimum tail space needed to keep the pinned target stable

`sticky-latest` is used for the "latest turn should stay pinned to top" behavior.
Its floor can be reconciled from live DOM measurements as content grows or shrinks.

## 2. Synchronous Footer DOM Apply

React state alone is not enough here.

`applyFooterCompensationNow()` writes footer height directly to the DOM and forces layout reads:

- `footer.style.height`
- `footer.style.minHeight`
- `footer.offsetHeight`
- `scroller.scrollHeight`

This is intentional. It ensures the browser uses the new footer height in the same turn, before we restore the anchor.

If you move compensation back to "React render only", the flash can return because the DOM may still be one frame behind when `scrollTop` is restored.

## 3. Anchor Lock

`anchorLockRef` temporarily remembers the desired `scrollTop`.

It exists for two reasons:

- immediate restore right after compensation is applied
- follow-up enforcement during scroll events while the layout is still settling

The immediate restore handles the critical path. The scroll listener is the safety net.

## 4. Collapse Intent

Some collapses are predictable before layout actually shrinks.

`flowchat:tool-card-collapse-intent` is emitted before a known collapsible UI
shrinks. `VirtualMessageList` uses that event to:

- capture the pre-collapse anchor `scrollTop`
- capture the bottom distance before collapse
- estimate required compensation from current card height
- apply provisional compensation immediately

This pre-compensation is what avoids the flash.

If the list waits until `ResizeObserver` sees the shrink, the browser may already have clamped `scrollTop`.

## Runtime Flow

## A. Known Tool Card Collapse

When a helper-backed card or region is about to collapse:

1. it dispatches `flowchat:tool-card-collapse-intent` before the collapse state is applied
2. `VirtualMessageList` estimates the upcoming shrink using `cardHeight`
3. `VirtualMessageList` adds provisional footer compensation immediately
4. `VirtualMessageList` activates anchor lock using the current `scrollTop`
5. actual layout shrink happens
6. `ResizeObserver` / `MutationObserver` / transition listeners trigger `measureHeightChange()`
7. measured shrink reconciles the compensation to the real final value
8. anchor lock restores / enforces the final `scrollTop`

Common examples:

- `FileOperationToolCard`
- `ModelThinkingDisplay`
- `TerminalToolCard`
- `ExploreGroupRenderer`

## B. Unknown or Unsignaled Shrink

If a shrink happens without a collapse intent:

1. `measureHeightChange()` detects the negative height delta
2. compensation falls back to `shrinkAmount - distanceFromBottom`
3. anchor lock uses the previously known scroll position

This path is safer than doing nothing, but it is more likely to show visible movement than the pre-compensation path.

## Why Transition Tracking Exists

Some collapsible UI uses animated layout properties such as:

- `grid-template-rows`
- `height`
- `max-height`

During those transitions, the DOM may report intermediate sizes for multiple frames.

`layoutTransitionCountRef` prevents us from consuming compensation too early while the layout is still animating. If you remove this guard, compensation can disappear mid-transition and reintroduce vertical drift.

## C. Follow-Output Mode (continuous tail)

When the viewport is in follow-output mode and the latest turn is still
streaming, the user's intent is "keep the tail visible", which is the
opposite of "preserve the upper anchor". To avoid the visible
"stutter then jump" behavior caused by collapse pre-compensation
freezing the viewport mid-animation, follow mode short-circuits the
protection path:

1. `handleToolCardCollapseIntent` returns early without writing
   `pendingCollapseIntent`, without adding `collapse` reservation, and
   without activating anchor lock.
2. The shrink branch of `measureHeightChange` returns early without
   adding fallback footer compensation.
3. A continuous RAF loop in `useFlowChatFollowOutput` runs every frame
   while `isFollowing && isStreaming`, calling `performAutoFollowScroll`
   to chase the bottom and `reconcileStickyPinReservation` to keep the
   sticky-latest pin floor aligned with the live DOM.
4. The loop is cancelled as soon as follow exits (user upward scroll,
   session change, streaming ends, or an explicit navigation).

This branch coexists with the legacy collapse compensation path. Outside
follow mode (user reading older content), all original protections still
apply unchanged.

## Why `overflow-anchor: none` Must Stay

`VirtualMessageList.scss` disables native browser scroll anchoring on:

- `[data-virtuoso-scroller]`
- `.message-list-footer`

This is required because the browser's built-in anchoring fights the manual compensation logic.

If you remove `overflow-anchor: none`, the browser may apply its own anchor correction on top of our compensation and produce unstable or inconsistent results.

## Required Event Contract

`tool-card-toggle`

- dispatch after a generic expand/collapse action that changes height
- purpose: schedule a follow-up measurement

`flowchat:tool-card-collapse-intent`

- dispatch before a collapse that can reduce list height near the bottom
- include `cardHeight` when possible
- purpose: pre-compensate before the browser clamps scroll position

Current producer:

- `useToolCardHeightContract.ts`
- `ModelThinkingDisplay.tsx`
- `ExploreGroupRenderer.tsx`

Most tool cards now emit these events through `useToolCardHeightContract`.
Components that need more accurate collapse estimation can pass a custom
`getCardHeight` function to the helper.

If a future collapsible component shows the same "header drops" or "flash on collapse" symptom, it should likely emit `flowchat:tool-card-collapse-intent` before collapsing.

## Invariants To Preserve

- Footer compensation must remain additive temporary space, not real content.
- Effective height comparisons must subtract current compensation.
- Footer DOM compensation must be applied synchronously before anchor restore.
- Anchor restore must clamp against current `maxScrollTop`.
- Pre-collapse intent must capture the anchor before the component shrinks.
- Compensation must not be consumed too early during active layout transitions.
- Session changes and empty-list resets must clear compensation and anchor state.

## Common Ways To Break This

- Replacing `applyFooterCompensationNow()` with state-only rendering.
- Measuring raw `scrollHeight` deltas without subtracting existing compensation.
- Removing `flowchat:tool-card-collapse-intent` from a helper-backed collapsible component.
- Dispatching collapse intent after `setState` instead of before it.
- Removing `overflow-anchor: none`.
- Removing transition-aware delayed measurement.
- Simplifying anchor restore to a one-shot restore without the scroll listener fallback.
- Removing the follow-mode short-circuit in `handleToolCardCollapseIntent` /
  `measureHeightChange`. Without it, follow-output streaming will visibly stall
  during collapse animations and then snap to the latest token.
- Removing the continuous RAF follow loop. Event-driven follow alone cannot
  keep up with collapse animations + dense token streams without visible jitter.

## If You Need To Change This Logic

Use this checklist:

1. Verify bottom collapse at the end of a conversation.
2. Verify manual collapse of a completed `Write` / `Edit` tool card.
3. Verify auto-collapse of file tool cards after streaming finishes.
4. Verify repeated expand/collapse near the bottom.
5. Verify thinking / explore / other collapsible sections still schedule measurements correctly.
6. Verify there is no visible "drop then snap back" flash.
7. Verify the final header position remains stable after collapse.

## Related Files

- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.tsx`
- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.scss`
- `src/web-ui/src/flow_chat/tool-cards/useToolCardHeightContract.ts`
- `src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx`
- `src/web-ui/src/flow_chat/tool-cards/ModelThinkingDisplay.tsx`
- `src/web-ui/src/flow_chat/tool-cards/TerminalToolCard.tsx`
- `src/web-ui/src/flow_chat/components/modern/ExploreGroupRenderer.tsx`
