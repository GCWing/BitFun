# FlowChat Scroll Stability

This document explains the scroll-stability mechanism used by `VirtualMessageList.tsx`.

## Rule Zero: Do Not Create Motion For This Mechanism To Chase

Every rule below is compensation for content that changes size on its own. The
cheapest way to keep the pane stable is to not generate the movement in the
first place. Three invariants hold across the message list, and breaking any of
them reintroduces the "the chat keeps refreshing itself" report:

1. **No mount-triggered animation on anything the list renders.** The list is
   virtualized: an item that scrolls out of view unmounts and remounts, so a
   `fadeIn` / `slideInUp` keyed off mount replays on every pass. Same for an
   animation keyed off `--streaming` → `--complete`: it replays when the
   typewriter drains. `getModelRoundItemClassName` deliberately has no `--enter`
   modifier, and `.user-message-item` deliberately has no enter animation.
2. **No wall-clock input to projection or grouping.** `sessionToVirtualItems`
   and `buildModelRoundItemGroups` are pure functions of the session data. A
   time-dependent classification needs a timer to re-run it, and that timer
   restructures and remounts cards seconds after the data settled. There is no
   "transient window" for recently-completed tools any more.
3. **Automatic expand/collapse lands in one frame; only user clicks animate.**
   An automatic collapse that animates over 250–320 ms forces the compensation
   path below to track a moving target frame by frame — that tracking is the
   visible jitter. `ModelThinkingDisplay`, `FileOperationToolCard` (via
   `BaseToolCard disableExpandAnimation`) and `ExploreGroupRenderer` (via
   `SmoothHeightCollapse disableAnimation`) all animate only when the change
   came from a user click.

A fourth, related rule lives in `useTypewriter`: `replayOnMount` defaults to
false, so a still-streaming block that remounts continues from its current text
instead of resetting to an empty string and re-growing.

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

User-initiated expand/collapse still uses animated layout properties such as:

- `grid-template-rows`
- `height`
- `max-height`

(Automatic collapses no longer animate — see Rule Zero — so this path now only
covers deliberate user toggles.)

During those transitions, the DOM may report intermediate sizes for multiple frames.

The collapse intent carries a hard TTL (`expiresAtMs`, currently 1000 ms).
While the intent is alive, the grow branch of `measureHeightChange` does not
consume compensation, so a mid-animation intermediate size cannot drain it too
early. When the TTL lapses, `replayDeferredFollowIfSettled` drains residual
compensation and replays any deferred follow. There is no transition-event
listener: expiry is purely time-based.

## C. Follow-Output Mode (continuous tail)

When the viewport is in follow-output mode and the latest turn is still
streaming, the user's intent is "keep the tail visible". The continuous
RAF loop re-pins `scrollTop` toward the bottom every frame.

Collapses interact with follow mode in two mutually exclusive ways:

1. **Follow + streaming active:** `handleToolCardCollapseIntent` returns
   early and writes no intent, and the shrink branch of
   `measureHeightChange` is skipped. The RAF loop simply re-pins to the
   new bottom on the next frame, absorbing the shrink in ~16 ms. Because
   automatic collapses are now instant (Rule Zero), the shrink is a
   single-frame step — there is no multi-frame animation for the loop to
   chase, which is what previously produced the "conversation sinks
   down" drift. Not writing an intent here also means nothing
   accumulates, so issue #1176 (permanent footer whitespace) cannot
   occur in this path.
2. **Not following (user reading older content):** the intent +
   pre-compensation + anchor-lock path applies as described above, and
   `shouldSuspendAutoFollow` keeps event-driven follow scheduling
   deferred until the intent's TTL lapses.

The loop is cancelled as soon as follow exits (user upward scroll,
session change, streaming ends, or an explicit navigation).

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

- Adding a mount-triggered CSS animation to a virtualized list item, or making an
  automatic collapse animated again (see Rule Zero).
- Feeding `Date.now()` back into `sessionToVirtualItems` /
  `buildModelRoundItemGroups`, or splitting one `ModelRound` into several
  `model-round` virtual items — both swap stable Virtuoso keys for new ones and
  remount visible content.
- Replacing `applyFooterCompensationNow()` with state-only rendering.
- Measuring raw `scrollHeight` deltas without subtracting existing compensation.
- Removing `flowchat:tool-card-collapse-intent` from a helper-backed collapsible component.
- Dispatching collapse intent after `setState` instead of before it.
- Removing `overflow-anchor: none`.
- Removing the intent TTL / expiry drain (`replayDeferredFollowIfSettled`).
- Simplifying anchor restore to a one-shot restore without the scroll listener fallback.
- Removing the follow-mode early return in `handleToolCardCollapseIntent` /
  `measureHeightChange`. During follow + streaming the RAF loop absorbs the
  (now single-frame) shrink by re-pinning next frame; injecting compensation +
  anchor lock there instead freezes the viewport on older content and causes
  the "occasionally not at the bottom" bug.
- Removing the `shouldSuspendAutoFollow` gate from event-driven follow
  scheduling. Outside follow mode it keeps deferred follows from firing while a
  collapse intent is still protecting the anchor.
- Removing the continuous RAF follow loop. Event-driven follow alone cannot
  keep up with dense token streams without visible jitter outside collapse
  windows.

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
