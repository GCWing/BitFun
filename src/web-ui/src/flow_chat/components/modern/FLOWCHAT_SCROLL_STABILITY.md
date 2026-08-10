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

## Opening a Session

A session mounts against an unsettled transcript: item heights are still
estimates, and an `isPartial` session pages older Turns in for hundreds of
milliseconds. The end of content can travel thousands of pixels after the first
alignment, so opening is its own phase with its own rules.

**While opening, the transcript is hidden and the follow target is
authoritative.** It tracks the content end exactly — no remembered offset, no
gap tolerance, and no accommodation of a foreign `scrollTop` write. Virtuoso
writes during this window too (it compensates a history prepend from the item
index before the prepended heights reach the DOM); fighting it is invisible
because nothing is painted, and accommodating it would be permanent once paging
stops.

**After the reveal, the follow target is cooperative.** The gap tolerance
applies again, because from then on a shrinking content end means a card
collapsed, not that measurement is still catching up.

The reveal waits for a *semantic* signal — the last virtual item rendered with
its end inside the viewport, plus the viewport in position — not for geometry to
stop changing. Before Virtuoso renders anything, `scrollHeight` and the content
end sit unchanged at their unmeasured values, which is indistinguishable from
having finished; a stability test reveals on frame 3 and shows the whole settle.

`tailHoldMaxGapPx` is a **streaming allowance**. Blank below the live output is
tolerable only because more output is about to fill it. Do not reuse it to
absorb anything else — applied to a foreign forward move it parks the content
end mid-viewport permanently, since nothing pulls the target back down.

## Current Behavior

- A newly submitted Turn scrolls to the viewport top and enters follow-output.
  Every other entry reason resumes at the end of real content.
- Session open enters follow-output as `session-open`, even with nothing
  streaming. The frame loop then runs on a `SETTLE_FRAMES` budget that refreshes
  whenever the target actually moves, so it tracks measurement and paging and
  then goes quiet. Without it nothing owns the viewport after the one-shot
  alignment, and the transcript strands wherever that early shot landed.
- `scrollToTurnEnd` deliberately does **not** exit follow-output. It is the
  session-open placement and wants the same position the settle is converging
  on; releasing ownership there hands the viewport back to nobody.
- The history prepend anchor is skipped while follow-output owns the viewport.
  Restoring a pre-prepend position is only meaningful when the user owns it.
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

## Virtuoso Footer Coupling

react-virtuoso adds the **entire** footer height when it scrolls to the *last*
index with `align: 'end'` (`dist/index.mjs`: `ft === wt && (St += O)`, where `O`
is `footerHeight`). FlowChat relies on that for the input-stack clearance on
session open, but the tail spacer lives in the same footer, so an uncorrected
end-alignment opens the session on a screen of reserved blank.

Every `align: 'end'` scroll therefore passes
`endAlignedTailOffsetPx(index, itemCount, tailSpacerPx)`, which cancels the
spacer's share and only the spacer's share. The affected call sites are
`initialTopMostItemIndex`, `scrollToTurnEnd`, and the `scrollToContentEnd`
fallback. `align: 'start'` and `align: 'center'` are unaffected — Virtuoso adds
footer height for the last index only.

If a future Virtuoso upgrade changes that rule, sessions will open one viewport
too high or too low. Re-check the offset math before bumping the dependency.

## Known Gaps

- Users can still scroll down into the reserved blank. Clamping or snapping that
  gesture is deliberately not implemented yet; it fights momentum scrolling and
  is pure polish.
- A collapse larger than `tailHoldMaxGapPx` still moves the viewport, by the
  excess only.
- On a very short transcript the scrollbar exposes a viewport of empty range.
- The opening reveal has a hard frame cap. A session that pages for longer than
  the cap is revealed mid-settle; raising the cap trades that against a longer
  blank on open.

## Why There Is No Viewport Coordinator

There was one — `FlowChatViewportCoordinator.ts`, removed alongside the
compensation engine. Single-writer semantics are not reachable here: Virtuoso
writes `scrollTop` from inside the library (`initialTopMostItemIndex`, the
`firstItemIndex` prepend compensation, `scrollToIndex`), so a coordinator can
only serialise *our* writes and the conflicts observed in practice were with
that third writer.

What works instead is a single *source of truth*. Read the live viewport, and
make each phase's rule idempotent with respect to a foreign write: authoritative
while hidden, cooperative once painted.

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
