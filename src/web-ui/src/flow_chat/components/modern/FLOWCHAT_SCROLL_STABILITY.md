# FlowChat Scroll Stability

This document explains the scroll-stability mechanism used by `VirtualMessageList.tsx`.

## Rule Zero: Do Not Create Motion For This Mechanism To Chase

Every rule below is compensation for content that changes size on its own. The
cheapest way to keep the pane stable is to not generate the movement in the
first place. Four invariants hold across the message list, and breaking any of
them reintroduces the "the chat keeps refreshing itself" report:

1. **Keep a live action's projection identity stable.** A collapsible tool
   belongs to its trailing `explore-group` from the first active render through
   completion. Do not render it as a standalone `model-round` while active and
   move it into a group when it settles; that swaps the Virtuoso key, unmounts
   the card, and looks like a flash. Likewise, never hide the old location with
   `display: none` as a handoff mechanism.
2. **No mount-triggered animation on anything the list renders.** The list is
   virtualized: an item that scrolls out of view unmounts and remounts, so a
   `fadeIn` / `slideInUp` keyed off mount replays on every pass. Same for an
   animation keyed off `--streaming` → `--complete`: it replays when the
   typewriter drains. `getModelRoundItemClassName` deliberately has no `--enter`
   modifier, and `.user-message-item` deliberately has no enter animation.
3. **No wall-clock input to projection or grouping.** `sessionToVirtualItems`
   and `buildModelRoundItemGroups` are pure functions of the session data. A
   time-dependent classification needs a timer to re-run it, and that timer
   restructures and remounts cards seconds after the data settled. There is no
   "transient window" for recently-completed tools any more.
4. **Do not compact the live tail merely because its status completed.** A
   terminal, process, file, task, question, or thinking card that was visible
   while running keeps a compact result preview until newer content supersedes
   it. When superseded, automatic expand/collapse lands in one frame; only user
   clicks animate. An automatic collapse that animates over 250–320 ms forces
   the compensation path below to track a moving target frame by frame — that
   tracking is the visible jitter. `ModelThinkingDisplay`,
   `FileOperationToolCard` (via `BaseToolCard disableExpandAnimation`) and
   `ExploreGroupRenderer` (via `SmoothHeightCollapse disableAnimation`) all
   animate only when the change came from a user click.

A fifth, related rule lives in `useTypewriter`: `replayOnMount` defaults to
false, so a still-streaming block that remounts continues from its current text
instead of resetting to an empty string and re-growing.

Read this before changing any of the following:

- footer height / footer rendering in `VirtualMessageList.tsx`
- scroll compensation state or refs
- semantic anchor lifetime and one-shot fallback restoration
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

Reservation state is ref-owned first and mirrored into React state. A Virtuoso
Footer remount must synchronously read the ref-owned value; otherwise one stale
React commit can remove exactly the reserved scroll range for a frame.

The Virtuoso Footer does not receive reservation pixels through React context.
Its stable DOM node is updated imperatively, and its ref callback restores the
current ref-owned height on mount. This keeps reservation updates from causing
an additional measurement-sensitive Virtuoso render.

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
Its floor grows when live DOM measurements require more range and drains only
from measured positive content growth.
The pinned item may hand off to tail-follow only after both the complete pin
reservation (`px`, not only `floorPx`) and collapse reservation reach zero.

## 2. Synchronous Footer DOM Apply

React state alone is not enough here.

`applyFooterCompensationNow()` writes footer height directly to the DOM and forces layout reads:

- `footer.style.height`
- `footer.style.minHeight`
- `footer.offsetHeight`
- `scroller.scrollHeight`

This is intentional. It ensures the browser uses the new footer height in the same turn, before we restore the anchor.

If you move compensation back to "React render only", the flash can return because the DOM may still be one frame behind when `scrollTop` is restored.

## 3. Semantic Anchor Coordinator

`FlowChatViewportCoordinator` owns the semantic viewport anchor. It tracks one
primary mode at a time: pinned item, following tail, or preserving an element.
Tool cards supply an anchor element but never calculate scroll offsets or
heights. The coordinator records the element's viewport-relative position and
restores it after the list remeasures. While an element anchor is active, the
coordinator also owns virtualizer compensation corrections, so independent
scroll writers cannot fight the pinned header.

There is no persistent scroll-position lock or scroll-listener lock. For an unsignaled
shrink with no semantic element anchor, `restoreScrollPositionOnce()` performs
one clamped `scrollTop` fallback using the pre-change position. It is a bounded
last resort, not a second controller: subsequent layout changes are handled by
the semantic anchor (when present), the reservation model, or follow mode.

An element anchor also owns the minimum physical scroll range needed to restore
its offset. After writing `scrollTop`, the coordinator remeasures the actual DOM
offset. If a positive correction remains because the browser clamped at the
bottom, the range host synchronously extends the matching reservation, flushes
layout, and retries in the same frame. The post-write DOM measurement is the
source of truth because integer `scrollHeight` can overstate the browser's
subpixel scroll limit.

Physical-bottom synchronization must yield whenever the coordinator owns an
element anchor. A sticky pin intentionally sits at the physical bottom created
by its reservation; treating that geometry as tail-follow causes every content
growth measurement to push the pinned header upward before the coordinator can
restore it.

Sticky pin floors are not reduced from a transient target rect. Positive
effective content growth first enters a short settlement ledger (currently
300 ms) instead of immediately removing physical bottom range. An unsignaled
negative height correction cancels matching unsettled growth; a known collapse
does not. Stable growth then consumes the pin floor in one synchronous Footer
update. Live pin reconciliation may increase a floor immediately, but cannot
shrink it while Virtuoso item measurements are still moving. Stream end
performs one final atomic collapse-to-pin transfer using the settled required
range.

## 4. Collapse Intent

Some collapses are predictable before layout actually shrinks.

`flowchat:tool-card-collapse-intent` is emitted before a known collapsible UI
shrinks. `VirtualMessageList` uses that event to:

- capture the card root as the semantic header anchor
- capture the pre-collapse anchor `scrollTop`
- capture the bottom distance before collapse
- estimate required compensation from current card height
- apply provisional compensation immediately

This pre-compensation is what avoids the flash.

If the list waits until `ResizeObserver` sees the shrink, the browser may already have clamped `scrollTop`.

## Runtime Flow

## A. Known Tool Card Collapse

When a helper-backed card or region is about to collapse:

1. it dispatches `flowchat:tool-card-collapse-intent` with its anchor element before the collapse state is applied
2. `VirtualMessageList` estimates the upcoming shrink using `cardHeight`
3. `VirtualMessageList` adds provisional footer compensation immediately
4. `VirtualMessageList` applies the provisional footer synchronously and records
   the semantic anchor's viewport offset
5. actual layout shrink happens
6. `ResizeObserver` / `MutationObserver` / transition listeners trigger `measureHeightChange()`
7. measured shrink reconciles the compensation to the real final value
8. the coordinator restores the anchor element's exact viewport-relative position

Common examples:

- `FileOperationToolCard`
- `ModelThinkingDisplay`
- `TerminalToolCard`
- `ExploreGroupRenderer`

## B. Unknown or Unsignaled Shrink

If a shrink happens without a collapse intent:

1. `measureHeightChange()` detects the negative height delta
2. compensation falls back to `shrinkAmount - distanceFromBottom`
3. `restoreScrollPositionOnce()` makes one clamped fallback restore using the
   previously known scroll position

This path is safer than doing nothing, but it is more likely to show visible movement than the pre-compensation path.

## Why Transition Tracking Exists

User-initiated expand/collapse still uses animated layout properties such as:

- `grid-template-rows`
- `height`
- `max-height`

(Automatic collapses no longer animate — see Rule Zero — so this path now only
covers deliberate user toggles.)

During those transitions, the DOM may report intermediate sizes for multiple frames.

The collapse intent carries a hard TTL (`expiresAtMs`, currently 1000 ms), but
its settlement is autonomous rather than scroll-driven. Automatic collapses are
finalized after a short settle-frame window; manual or otherwise unsignaled
intents use the TTL timer. The scroll handler keeps only a throttled-background
timer fallback for browsers that delay timers. While the intent is alive, the
grow branch of `measureHeightChange` protects the collapse reservation, but it
may still consume measured content growth from the sticky pin reservation.
Once the intent settles, residual collapse space is transferred to the settled
sticky pin in one state/DOM update and any deferred follow is replayed.

## C. Follow-Output Mode (continuous tail)

When the viewport is in follow-output mode and the latest turn is still
streaming, the user's intent is "keep the tail visible". The continuous
RAF loop re-pins `scrollTop` toward the bottom every frame.

Collapses interact with follow mode in three mutually exclusive ways:

1. **Known collapse while follow + streaming is active:** the intent applies
   synchronous Footer pre-compensation before the card shrinks. The active
   intent allows shrink reconciliation even though tail follow is running.
   When the short protection window ends, the collapse reservation remains
   consumable by real streaming growth instead of being removed immediately;
   stream end performs the final exact reconciliation. This keeps the card
   header stable without giving up tail-follow ownership.
   If React/Virtuoso has already clamped `scrollTop` before a data-driven auto
   intent reaches the list's layout handler, the handler extends the collapse
   reservation as needed and restores the last stable follow position before
   paint. Manual collapses and non-follow viewports do not use this fallback.
2. **Unsignaled shrink while follow + streaming is active:** there is no
   semantic collapse transaction to preserve, so the RAF loop re-pins to the
   new bottom on the next frame.
   A negative `scrollBy` issued by Virtuoso after a virtualized height
   reduction is also suppressed when the previous geometry was already at the
   physical bottom. That compensation would move the viewport away from the
   tail; the next follow frame owns the single tail correction instead.
3. **Not following (user reading older content):** the intent +
   pre-compensation + semantic-anchor path applies as described above, and
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
- include the card root as `anchorElement`; its top edge represents the stable header position
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
- A stalled positive anchor correction must extend physical bottom range and
  retry before paint.
- Resize and height observers must not synchronize to the physical bottom while
  a semantic element anchor owns the viewport.
- Sticky pin floors must shrink from measured content growth, not a transient
  target-element position.
- A user gesture that exits pinned mode must release the semantic anchor and
  clear or atomically transfer the pin reservation in the same operation; an
  idle coordinator must never retain a live pin reservation.
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
- Removing the intent TTL, settle-frame finalizer, or the throttled scroll
  fallback that covers delayed background timers.
- Reintroducing a persistent scroll-listener lock or allowing multiple competing
  scroll writers. Semantic anchors and the bounded fallback must remain separate.
- Passing reservation pixels through Virtuoso context or React-owned Footer
  styles. The stable Footer DOM and ref-owned reservation are the hot path.
- Restoring the blanket follow-mode early return in
  `handleToolCardCollapseIntent` or applying it to an active known intent in
  `measureHeightChange`. Known streaming collapses require synchronous range
  reservation; only unsignaled shrinks are delegated entirely to the RAF loop.
- Removing the `shouldSuspendAutoFollow` gate from event-driven follow
  scheduling. Outside follow mode it keeps deferred follows from firing while a
  collapse intent is still protecting the anchor.
- Removing the continuous RAF follow loop. Event-driven follow alone cannot
  keep up with dense token streams without visible jitter outside collapse
  windows.

## If You Need To Change This Logic

Use this checklist:

1. Verify the live tail stays expanded when a conversation ends with an action.
2. Verify manual collapse of a completed `Write` / `Edit` tool card.
3. Verify automatic compaction only after newer content supersedes the action.
4. Verify repeated expand/collapse near the bottom.
5. Verify thinking / explore / other collapsible sections still schedule measurements correctly.
6. Verify there is no visible "drop then snap back" flash.
7. Verify the final header position remains stable after collapse.

## Related Files

- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.tsx`
- `src/web-ui/src/flow_chat/components/modern/FlowChatViewportCoordinator.ts`
- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.scss`
- `src/web-ui/src/flow_chat/tool-cards/useToolCardHeightContract.ts`
- `src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx`
- `src/web-ui/src/flow_chat/tool-cards/ModelThinkingDisplay.tsx`
- `src/web-ui/src/flow_chat/tool-cards/TerminalToolCard.tsx`
- `src/web-ui/src/flow_chat/components/modern/ExploreGroupRenderer.tsx`
