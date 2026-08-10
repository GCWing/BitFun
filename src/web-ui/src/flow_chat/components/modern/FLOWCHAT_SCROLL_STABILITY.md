# FlowChat Tail Reservation Contract

FlowChat reserves a resident tail spacer of roughly one viewport below the
transcript, and pairs it with a follow target that does not move backwards for
free. Together these give a newly submitted Turn a top-aligned position and keep
a tool-card collapse from dragging earlier content down.

## The Rule That Matters

**Static reservation is allowed. Reactive compensation is not.**

The tail spacer's height is a function of the viewport and nothing else. It must
never be derived from a measured content height, a collapse delta, an animation
duration, or a streaming rate. The moment its height reacts to content, it stops
being a reservation and becomes the compensation engine that was removed in
"remove synthetic tail-space scrolling" — do not rebuild that under a new name.

## Why Both Halves Are Required

The spacer alone fixes nothing. It only removes the browser's forced `scrollTop`
clamp when content shrinks, which is *permission* to hold position. A follow
target that re-aligns the content end to the viewport bottom every frame will
still drag earlier content down by the collapse delta, spacer or not.

`flowChatTailFollow.ts` supplies the second half:

- `pin-turn-top` holds a freshly submitted Turn's user message at the viewport
  top while its answer is shorter than one viewport, then hands off at the
  crossover. The blank below a pinned Turn is the mode, not a defect.
- `hold-tail` keeps its previous offset when content shrinks, and gives ground
  only once the blank below the live output exceeds `tailHoldMaxGapPx`
  (a share of the viewport, not a measured delta).

Both are pure functions over geometry. They hold no timers and observe no
mutation.

## Current Behavior

- A newly submitted Turn scrolls to the viewport top and enters follow-output.
  Every other entry reason resumes at the end of real content.
- `useFlowChatFollowOutput` is the only continuous writer while output streams.
- `scheduleFollowToLatest` re-asserts ownership after a layout change but does
  **not** force the content end. A collapse resizes content too, and the hold
  rule is what keeps that from moving the viewport.
- Ordinary `scroll` events do not transfer viewport ownership; only explicit
  wheel, touch, or keyboard navigation exits follow-output.
- The pinned Turn's offset is re-resolved from live layout every frame. Virtuoso
  re-estimates unrendered item heights, so a cached absolute offset would drift.
- When streaming stops, `hold-tail` settles any remaining blank with one smooth
  scroll. A pinned Turn does not settle.
- Tool-card expansion and collapse use normal layout reflow and dispatch only
  `tool-card-toggle`. There is still no pre-collapse intent event and no
  per-card compensation.
- "At bottom" is measured against the end of real content, not the end of the
  spacer. Virtuoso's own `atBottomStateChange` is therefore unused.

## Known Gaps

- Users can still scroll down into the reserved blank. Clamping or snapping that
  gesture is deliberately not implemented yet; it fights momentum scrolling and
  is pure polish.
- A collapse larger than `tailHoldMaxGapPx` still moves the viewport, by the
  excess only.
- On a very short transcript the scrollbar exposes a viewport of empty range.

## Viewport Ownership

Keep `followOutput={false}` on Virtuoso. Continuous movement belongs to
`useFlowChatFollowOutput`; one-shot navigation belongs to
`VirtualMessageList`. Card renderers and tool cards must not write the outer
FlowChat `scrollTop`.

Local scroll surfaces inside a thinking, explore, terminal, or subagent card
may manage their own scroll position. They must not dispatch an outer viewport
compensation request.

Stable virtual-item keys and projection identity remain required. Do not split
one `ModelRound` into multiple virtual items, reclassify projection from a
timer, or add mount-triggered motion that changes transcript geometry.

## Footer Contract

The Virtuoso footer holds two independent pieces, and they must stay separate:

```text
message-list-footer     = current input-stack height + bottom inset + clearance
message-list-tail-spacer = tailSpacerPxForViewport(scroller.clientHeight)
```

The footer must not retain an earlier input height or include an estimated card
shrink. The spacer must not be folded into the footer calculation, or input
height would once again drive the scroll range.

## Verification

Run the smallest relevant automated checks:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run \
  src/flow_chat/components/modern/flowChatTailFollow.test.ts \
  src/flow_chat/components/modern/useFlowChatFollowOutput.test.tsx \
  src/flow_chat/components/modern/VirtualMessageList.session-boundary.test.tsx \
  src/flow_chat/components/modern/ModernFlowChatContainer.history-state.test.tsx \
  src/flow_chat/tool-cards/useToolCardHeightContract.test.tsx
```

Agents must not perform UI interaction verification. A human follow-up should
confirm:

1. A newly submitted Turn opens at the viewport top with room below it.
2. Streaming follows the tail until the user scrolls, and the pinned Turn hands
   off once its answer overflows the viewport.
3. An auto-collapsing TodoWrite or ExecCommand card leaves earlier content
   visually still.
4. Turn Rail and Usage Report navigation can now top-align the final Turns.
5. Session switching and history paging do not restore stale footer height.
6. Session open still lands at the end of the transcript, not inside the spacer.

## Related Files

- `flowChatTailFollow.ts`
- `useFlowChatFollowOutput.ts`
- `VirtualMessageList.tsx`
- `ModernFlowChatContainer.tsx`
- `../../utils/flowChatScrollLayout.ts`
- `../../tool-cards/useToolCardHeightContract.ts`
