# FlowChat Scroll Stability

FlowChat reserves a resident tail spacer below the transcript, and pairs it with
a follow target that does not move backwards for free. Together these give a
newly submitted Turn a top-aligned position and keep a tool-card collapse from
dragging earlier content down.

That is this document. Four siblings carry the rest.

## Which Document

| Changing | Read |
|---|---|
| the tail spacer, the follow target, pinning, holding, the snap back, resizing, the footer | this file |
| history paging, the prepend, the viewport anchor, history presentation | `FLOWCHAT_HISTORY_PAGING.md` |
| anything that writes `scrollTop`, one-shot navigation, the diagnostic trail | `FLOWCHAT_VIEWPORT_REGISTER.md` |
| the virtualizer, item measurement, item keys, anything a row renders | `FLOWCHAT_VIRTUALIZATION.md` |
| what to run before claiming it works | `FLOWCHAT_VERIFICATION.md` |

*Known Gaps* below is the whole list for all five — accepted defects are easier
to keep in one place than to hunt for.

## The Rule That Matters

**Static reservation is allowed. Reactive compensation is not.**

The tail spacer's height is a function of the viewport and the input-stack
inset. It must never be derived from a measured content height, a collapse
delta, an animation duration, or a streaming rate. The moment its height reacts
to content, it stops being a reservation and becomes the compensation engine
that was removed in "remove synthetic tail-space scrolling" — do not rebuild
that under a new name.

## How Much To Reserve

The spacer keeps two offsets inside the scroll range, and is the larger of what
they need. Both are bounds, not estimates: reserving more than the larger one is
pure blank at the end of the scroll range, and reserving less than either is a
clamp.

- **A pinned Turn.** Worst case its user message is the newest item with nothing
  answering it yet, so the message, the input inset and the spacer are all that
  lie below the message top. `clientHeight - bottomInsetPx -
  PINNED_TURN_MIN_ITEM_HEIGHT_PX` is exactly enough to put it on the top edge.
- **A held collapse gap.** `hold-tail` parks up to `tailHoldMaxGapPx` past the
  content end, and an offset the browser clamps is one the hold rule does not
  actually get to hold.

`PINNED_TURN_MIN_ITEM_HEIGHT_PX` must stay an **under**estimate of a
user-message item. Too low costs a few spare pixels of blank; too high puts the
pinned offset past the end of the scroll range, and the Turn is clamped back
down from the viewport top while the follow loop rewrites the clamped offset
every frame.

While the pin reserve is the binding bound, the spacer and the footer sum to a
constant: growing the composer moves the content end without moving the end of
the scroll range. Under the hold-gap floor the spacer stops tracking the inset
and the range grows with the composer, exactly as it did when the spacer was a
flat viewport.

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

`useFlowChatFollowOutput` is the only continuous outer viewport writer. Three
things about how it runs are load-bearing:

- **`scheduleFollowToLatest` re-asserts ownership after a layout change but does
  not force the content end.** A collapse resizes content too, and the hold rule
  is what keeps that from moving the viewport.
- **The pinned Turn's offset is re-resolved from live layout every frame.**
  Items above it are estimates until they are measured, so a cached absolute
  offset would drift.
- **When streaming stops, `hold-tail` settles any remaining blank with one
  smooth scroll.** A pinned Turn does not settle.

`tailHoldMaxGapPx` is a **streaming allowance**. Blank below the live output is
tolerable only because more output is about to fill it. Do not reuse it to
absorb anything else — applied to a foreign forward move it parks the content
end mid-viewport permanently, since nothing pulls the target back down.

## Opening a Session

A session mounts against an unsettled transcript: item heights are still
estimates, and an `isPartial` session pages older Turns in for hundreds of
milliseconds. The end of content can travel thousands of pixels after the first
alignment, so opening is its own phase with its own rules.

**While opening, the transcript is hidden and the follow target is
authoritative.** It tracks the content end exactly — no remembered offset, no
gap tolerance, and no accommodation of a foreign `scrollTop` write. The
virtualizer writes during this window too, as items measure and it corrects for
the ones above the viewport; fighting it is invisible because nothing is
painted, and accommodating it would be permanent once paging stops.

Nothing places the opening viewport by aligning to an item. The end of *real
content* is above the resident tail spacer, and no item knows where that is, so
the follow target writes the offset and the reveal waits for it.

Session open enters follow-output as `session-open`, even with nothing
streaming. The frame loop then runs on a `SETTLE_FRAMES` budget that refreshes
whenever the target actually moves, so it tracks measurement and paging and then
goes quiet. Without it nothing owns the viewport after the one-shot alignment,
and the transcript strands wherever that early shot landed. `scrollToTurnEnd`
deliberately does **not** exit follow-output for the same reason: it is the
session-open placement and wants the same position the settle is converging on,
and releasing ownership there hands the viewport back to nobody.

**After the reveal, the follow target is cooperative.** The gap tolerance
applies again, because from then on a shrinking content end means a card
collapsed, not that measurement is still catching up.

The reveal waits for a *semantic* signal — the last virtual item rendered with
its end inside the viewport, plus the viewport in position — not for geometry to
stop changing. Before the virtualizer renders anything, `scrollHeight` and the
end sit unchanged at their unmeasured values, which is indistinguishable from
having finished; a stability test reveals on frame 3 and shows the whole settle.

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

**The snap asks whether follow is *correcting* the viewport, not whether it owns
it.** Ownership outlives the frame loop deliberately — streaming has to be able
to resume follow after the settle budget runs out — so the two questions differ.
A live loop gets the viewport to itself, since it reaches its target in one
frame and a snap back would only race it. An asleep one does not: a viewport
left in the reserved blank under a sleeping loop is stranded, and nothing else
was watching for it. This is the half of the scrollbar problem that is fixed
everywhere, including where the drag itself cannot be recognised.

**Where a jump to latest lands.** Every entry into follow-output resumes at the
end of real content, with one exception: a jump to latest while the **newest**
Turn is still pinned returns to the pin. That mode only holds while the Turn's
answer is shorter than one viewport, so everything it has produced is already on
screen, and aiming at the content end would scroll *up* and shove the message
the user just sent into the middle. It is also the landing place the snap back
picks for the same viewport state — having the two disagree would be worse than
either choice. The exemption therefore outlives the Turn: a short Turn stays
pinned until a newer one replaces it.

## The Follow Eases Its Write, Never Its Target

The follow target moves when the transcript reflows, and Markdown reflows a
line at a time. A loop that assigns the target outright therefore spends 24px
on one frame out of seven and nothing on the other six, which is what a reader
reports as the output jumping rather than scrolling. `flowChatTailEase.ts`
spends the same distance over all seven.

It buys latency, not speed. Under steady growth the eased offset settles where
its per-frame catch-up equals the growth, so the visible step converges on *the
content's growth per frame* whatever the fraction is; smoothing spreads a lumpy
motion evenly across frames it already had. What `TAIL_EASE_ALPHA` actually
sets is how far behind the tail the offset rides, and that lag is what has to be
given back when the stream stops.

**Only the write is eased.** `followStateRef` still holds the offset the rule
owns, so the settle budget, the at-tail band and the snap back all keep reading
a target rather than a position in transit. An ease that leaked into the target
would make every one of them chase the lag.

Four boundaries, and none of them is a matter of taste:

- **Past `TAIL_EASE_SNAP_ABOVE_PX` it jumps.** The first frame of an ease covers
  a quarter of the distance, so beyond four lines the ease's *opening step* is
  already bigger than the jump it set out to replace.
- **A target above the current offset is never eased.** That is content getting
  shorter — a card collapsing, a table reflowing — and easing down through it
  reads as the transcript being clawed backwards.
- **Not while the transcript is opening.** There the target is authoritative and
  nothing is painted, so an ease is travel nobody can see, holding open the one
  phase whose whole point is to end. The reveal is watching for the viewport to
  reach the content end.
- **An ease in flight keeps the loop alive.** The settle budget is refreshed by
  the *target* travelling, so without this a correction arriving on the last
  budgeted frame would be abandoned partway. It terminates on its own: the ease
  halves what is left every frame, and a write the register refuses moves
  nothing and so books no further frame.

A step of the list's own scroll offset changes no layout, but unlike the
thinking card's it is not free: the virtualizer re-windows from the scroll
events it produces, and every scroll event is also an anchor carry and a
visible-Turn pass. That price is `listCommits` in the `tailFollow` probe, which
is why both viewports are sampled over one window — see
`flowChatTailFollowDiagnostics.ts`.

"At bottom" is a band, not a point: from the end of real content down to
whatever the follow rule owns. A pinned Turn and a held collapse gap are both
inside it, so neither raises the jump-to-latest affordance; the reserved blank
is outside it, so parking there does. No virtualizer-reported "at bottom" can
express this: the end of the scroll range is the bottom of the reserved blank,
not the end of content.

The band is recomputed on scroll, on resize, **and when follow ownership
changes** — its lower edge is the follow target, which can move while the
viewport is perfectly still. A snap back completes at rest by construction,
and a jump to latest that lands on a pin the viewport already sits on writes
nothing at all. Driving the band from scroll events alone left the affordance
visible over a viewport that was at the tail, and clicking it then had nothing
to do — an inert button is worse than a missing one.

**A follow the frame loop is still correcting is inside the band by
definition.** The eased write rides behind the offset it owns, so a burst of
two or three lines would otherwise drop the viewport out of the band for a few
frames and flash the affordance over a transcript that is following the newest
output. Ownership cannot express this: it outlives the loop deliberately, and a
viewport stranded in the reserved blank under a sleeping loop is the case the
snap back exists for. A gesture stops the loop before it can hide anything —
that is what makes reading the loop safe here and reading ownership not.

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
change makes the virtualizer render a different number of them; either way it
re-measures over the following passes, so the content end keeps moving after the
first callback. The correction therefore repeats over
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
also *intra-owner* and deliberately outside the register: `smoothScrollFramesRef`
is follow-output yielding to its own animation, and the register arbitrates
between writers rather than inside one. See `FLOWCHAT_VIEWPORT_REGISTER.md`.

## Footer Contract

The footer below the items holds two independent pieces, and they must stay
separate:

```text
message-list-footer      = current input-stack height + bottom inset + clearance
message-list-tail-spacer = tailSpacerPxForViewport(clientHeight, footer)
```

The footer must not retain an earlier input height or include an estimated card
shrink. The spacer reads the footer to size itself, but the two must not be
folded into one number: the footer is content the transcript clears, the spacer
is range past the end of content, and only the footer is inside the content end.

Footer height represents only the current input-stack layout and real footer
content such as history state and `RuntimeStatusSlot`.

## Known Gaps

- The eased follow raises the scroll-event rate from one a line to one a frame
  while output streams, and every one of those is an anchor carry, a
  visible-Turn pass and a boundary evaluation. The heavy one is bounded — the
  visible-Turn pass is coalesced to a frame and writes the store only on a
  change — but the virtualizer re-windows from those events, so the cost is
  real and lands in `listCommits`. Measure with the `tailFollow` probe before
  changing `TAIL_EASE_ALPHA`.
- Easing is bounded by the frames the display gives it. The step it converges
  on is the content's growth *per frame*, so the same stream smooths less at
  60Hz than at 200Hz, and a fast enough stream is a line a frame on any display
  — at which point the ease is spending its lag and buying nothing.
- A scrollbar drag is recognised from the gutter the bar occupies, so it is
  invisible where the platform draws overlay scrollbars that take no layout
  width — WebKit-backed builds, where `scrollbar-gutter: stable` reserves
  nothing either. There the drag still fights the frame loop while output
  streams; it no longer strands the viewport, because the snap back now asks
  whether follow is correcting rather than whether it owns. Closing the rest
  means either a signal that does not depend on the bar having a box, or a
  scrollbar of our own — which would also stop the thumb from reaching the
  reserved blank at all, and take the empty-range gap below with it.
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
- **Estimates are still estimates.** A page of history is now reserved per item
  rather than at one scalar, so the range it takes up is close instead of wrong
  by an order of magnitude — but `estimateVirtualMessageItemHeight` cannot know
  how a model round wraps. Corrections shrink; they do not reach zero. And the
  cost of rendering a heavy item is a separate axis: less measurement is forced
  at once, but the work each one costs is unchanged. See
  `FLOWCHAT_VIRTUALIZATION.md`.
- A junction still costs one frame at the first page of a session, measured at
  93px: the commit paints before the settle frame that would correct it. Slower
  frames swallow both and show nothing. Closing it means a tighter estimator,
  not a further correction — the correction already equals the change in the
  scroll range every time it runs.

## Related Files

- `flowChatTailFollow.ts`
- `useFlowChatFollowOutput.ts`
- `../../utils/flowChatScrollLayout.ts`
- `../../tool-cards/useToolCardHeightContract.ts`
- `VirtualMessageList.tsx`
- `ModernFlowChatContainer.tsx`
