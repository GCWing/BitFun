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

## Snapping Back Out of the Reserved Blank

The spacer is a full viewport the user can scroll into, and under slow streaming
it can take a long time for output to push it away. So a gesture that comes to
rest **below the follow target** returns to that target and hands the viewport
to follow, whether or not follow owned it before.

Three properties carry the whole design:

**The target is the follow target, never the content end.** A short new Turn is
pinned above the content end, so snapping to the content end would scroll *up*
and shove the message the user just sent into the middle of the viewport. A
held collapse gap is likewise a legitimate offset up to `tailHoldMaxGapPx` past
the content end; judged against the content end it would read as an overshoot
and fight the hold rule on every collapse. `memorylessFollowState` computes the
target from live geometry with no remembered offset, because the offset the hold
rule was protecting stopped being meaningful the moment the user took over.

**It acts on rest, never during the gesture.** `scrollend` where available, a
quiet period after the last scroll event where it is not. Correcting inside a
`scroll` handler fights momentum and the virtualizer's own writes; correcting
after the gesture ends fights nothing.

**Re-entering follow here does not violate "no intent from geometry".** The
region below the follow target is reserved blank — it carries no content, so a
gesture ending there can only mean "take me to the end". Scrolling up to read
history can never satisfy the condition. That asymmetry is the licence; do not
extend it to any position that has content in it.

The pin's *identity* therefore outlives a user takeover; only its *activity*
stops. Three things retire a pin: the crossover to `hold-tail`, a newer Turn,
and a session change. The crossover has to be one-way — a collapse can pull
content back under one viewport, and re-pinning there would jump the viewport
backwards. Since nothing re-pins a Turn whose identity was dropped, that is
automatic.

The snap completes on a second settle, and only when the viewport actually
arrived: a gesture that overrode the animation mid-flight belongs to the user
and keeps the viewport.

## Resizing Anchors the Viewport Bottom

A plain scroller preserves `scrollTop` across a resize, which anchors the **top**
edge — the bottom is where content gets revealed or swallowed. For a transcript
that is backwards, because the interesting end is the bottom.
`handleViewportResize` anchors there instead. Follow output already behaves this
way for a viewport it owns; this is the same rule for one it does not, so the
same drag stops producing two different results depending on whether the user
had scrolled.

The two halves are not equally capable, and the difference is the useful part:

- **A height change moves no content.** Preserving `scrollTop + clientHeight` is
  exact and needs no judgement about what the user was doing, so it is applied
  unconditionally. It also preserves the distance to the content end, which
  makes "was at the end, stays at the end" fall out for free rather than being a
  case. Growing the viewport is additionally a *restoration*: the browser used
  to clamp a bottom-anchored viewport at `scrollHeight - clientHeight`, and the
  resident spacer removed that clamp.
- **A width change reflows the transcript.** Where the line that was on the
  bottom edge went is a DOM question, and by the time the resize is observed the
  reflow has already happened, so it cannot be answered after the fact.
  Answering it would mean sampling an element anchor on the scroll path, which
  is a `getBoundingClientRect` per scroll event. Instead only the one position
  that can be recomputed from geometry is restored — the end of the transcript —
  which needs `wasAtTail`, the band check from *before* the resize.
  `VirtualMessageList` mirrors `isAtBottom` into a ref for that, and calls the
  handler ahead of recomputing it.

**One correction is not enough.** A width change reflows every item and a height
change makes Virtuoso render a different number of them; either way it
re-measures and re-estimates over the following passes, so the content end keeps
moving after the first callback. The correction therefore repeats over
`TAIL_REALIGN_RESIZE_CALLBACKS`, a window opened only by a change to the
scroller's own box. Streaming content growth arrives through the same observer
and must never inherit that window — it moves the content end away from a
resting viewport and can never strand it, so reacting to it would be all risk
and no benefit.

Two properties are shared with the gesture path, and one is not:

- **Instant, never animated.** A height change moves the viewport by exactly the
  height that was added or removed, so nothing appears to move at all; the rest
  is a correction the user is already watching happen under the cursor. An
  animation would add a scroll nobody asked for.
- **No transfer of ownership** — unlike the gesture path. A gesture ending in
  the blank says "take me to the end"; a layout change says nothing. The
  browser's clamp never changed who owned the viewport either.

Native scroll anchoring cannot help here: `overflow-anchor: none` is set
throughout the transcript, because it fights the virtualizer.

## The Frame Loop Yields to Its Own Animated Scrolls

`applyFollowTarget` assigns `scrollTop` outright, which cancels an in-flight
smooth scroll on the very next frame. Both `'smooth'` requests in
`useFlowChatFollowOutput` — the jump to latest and the post-streaming settle —
were therefore jumps in practice. `runContentEndScroll` now hands the loop a
frame budget to stay quiet for.

This is a budget rather than a flag on purpose: a missing completion signal
costs a few idle frames, where a stuck flag would stall follow entirely. It is
also the only place where one FlowChat writer defers to another; it is not a
coordinator, and the reasons there is no coordinator are unchanged (see below).

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
  wheel, touch, or keyboard navigation exits follow-output. A gesture that comes
  to rest inside the reserved blank hands it back.
- The pinned Turn's offset is re-resolved from live layout every frame. Virtuoso
  re-estimates unrendered item heights, so a cached absolute offset would drift.
- When streaming stops, `hold-tail` settles any remaining blank with one smooth
  scroll. A pinned Turn does not settle.
- Tool-card expansion and collapse use normal layout reflow and dispatch only
  `tool-card-toggle`. There is still no pre-collapse intent event and no
  per-card compensation.
- "At bottom" is a band, not a point: from the end of real content down to
  whatever the follow rule owns. A pinned Turn and a held collapse gap are both
  inside it, so neither raises the jump-to-latest affordance; the reserved blank
  is outside it, so parking there does. Virtuoso's own `atBottomStateChange`
  remains unused.

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

- Dragging the scrollbar does not exit follow-output. It produces only `scroll`
  events, which are deliberately not treated as intent, so a drag during
  streaming is a tug of war with the frame loop.
- A collapse larger than `tailHoldMaxGapPx` still moves the viewport, by the
  excess only.
- An animated scroll aims at the target it was issued for. Jumping to latest
  while output is arriving therefore ends with one catch-up step covering
  whatever content grew during the animation.
- A width change anchors the viewport bottom only for a viewport that was at the
  end of the transcript. Everywhere else the reflow moves content out from under
  the bottom edge and nothing puts it back, because the anchor would have to be
  captured before the reflow. Closing this means sampling an element anchor on
  the scroll path.
- On a very short transcript the scrollbar exposes a viewport of empty range.
  The snap back makes this more visible, not less: the range is draggable and
  bounces back.
- The opening reveal has a hard frame cap. A session that pages for longer than
  the cap is revealed mid-settle; raising the cap trades that against a longer
  blank on open.

## Diagnosing History Paging

Older Turns are paged in when the viewport reaches the head of the loaded
window. Every way that handshake can fail is **silent and identical in the UI**:
the boundary status returns to `idle` and no indicator is shown, so "declined to
load" is indistinguishable from "there is no more history". The failure is also
intermittent, so it is traced permanently rather than reproduced on demand.

`historySessionDiagnostics` keeps a per-session ring buffer shared with the
hydration timeline, and the two log channels carry different things:

| | `flowchat.log` | `webview.log` |
|---|---|---|
| carries | the full paging step stream | the refusal alarm + its trail |
| enabled by | `app.logging.flow_chat_diagnostics` | always on |
| written via | `flowChatDiagnostics.trace` | `log.warn` |

The in-memory trail is kept regardless of the flag, so
`warnHistoryPagingRefusedWithPendingTurns` is **self-sufficient** — the recent
events travel with the warning and no one has to reproduce the fault with
diagnostics turned on first. It warns once per session, so scrolling against a
dead boundary cannot flood the log. Turn the flag on only when the trail's
30-event cap is not enough.

Two detectors raise it:

- `exhausted` returned for `beyond-known-total`. That result **latches the
  direction off for the rest of the session** and only `applied` clears it, so
  reaching it on an unknown or contradictory total is how history goes
  permanently missing rather than merely late.
- A `before` request blocked by that latch while the session is still
  `isPartial`. This fires at the moment the user scrolls up and nothing happens.

When the report is "scrolling up shows no history, but the Turn Rail can still
load those Turns", search the log for `declined to page older Turns`. Turn Rail
navigation goes through `loadSessionTurnWindow` directly and bypasses the
boundary latch entirely, which is why it keeps working. The accompanying
`FlowChat history paging trail` warning carries the preceding events, including
`anchor_capture_failed` — `captureHistoryPrependAnchor` returning `false`
cancels a window that was already fetched.

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
7. Scrolling down into the reserved blank and letting go returns to the end of
   real content, and streaming resumes following from there.
8. Doing the same right after submitting a short Turn returns that Turn to the
   viewport top, not to the content end.
9. Pressing End scrolls to the bottom of the scroll range and then comes back —
   that key is the cheapest way to land deep in the spacer.
10. Jump to latest is now animated rather than an instant jump.
11. With the viewport resting at the end but *not* following — scroll away and
    back, and check the jump-to-latest affordance is hidden — resizing the
    window keeps content against the bottom in every direction: taller reveals
    more history above, shorter does not cut the last lines off, and narrower
    does not push them off screen as the text rewraps. Repeat while reading
    history: nothing should move.

## Related Files

- `flowChatTailFollow.ts`
- `useFlowChatFollowOutput.ts`
- `VirtualMessageList.tsx`
- `ModernFlowChatContainer.tsx`
- `../../utils/flowChatScrollLayout.ts`
- `../../tool-cards/useToolCardHeightContract.ts`
