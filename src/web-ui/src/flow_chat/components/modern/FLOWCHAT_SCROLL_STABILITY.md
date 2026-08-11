# FlowChat Tail Reservation Contract

FlowChat reserves a resident tail spacer below the transcript, and pairs it with
a follow target that does not move backwards for free. Together these give a
newly submitted Turn a top-aligned position and keep a tool-card collapse from
dragging earlier content down.

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

**After the reveal, the follow target is cooperative.** The gap tolerance
applies again, because from then on a shrinking content end means a card
collapsed, not that measurement is still catching up.

The reveal waits for a *semantic* signal — the last virtual item rendered with
its end inside the viewport, plus the viewport in position — not for geometry to
stop changing. Before the virtualizer renders anything, `scrollHeight` and the
end sit unchanged at their unmeasured values, which is indistinguishable from
having finished; a stability test reveals on frame 3 and shows the whole settle.

`tailHoldMaxGapPx` is a **streaming allowance**. Blank below the live output is
tolerable only because more output is about to fill it. Do not reuse it to
absorb anything else — applied to a foreign forward move it parks the content
end mid-viewport permanently, since nothing pulls the target back down.

## Keeping the Viewport on the Reader's Content

When history is prepended, the items arriving above the reader push their content
down by their own height while `scrollTop` stays the number it was.
`VirtualMessageList` adds that height back, in a layout effect, before anything
else observes the new transcript. The amount is read from the virtualizer's
placement of the item that *used to be first* — the height of exactly what
arrived, a delta and not a total.

This is the half of `firstItemIndex` that keying measurements on item identity
does not supply, and it was left out of the TanStack migration. Everything
downstream assumes it holds, and three separate failures were that one
assumption breaking:

- The virtualizer re-windows from its own scroll offset, which lags a frame, so
  it renders the head. The paging rule reads that as the reader having arrived
  at the head, and pages again — **five pages in 890ms on session open, and a
  single junction paging a transcript back to its first Turn**.
- The anchored Turn falls outside that window, so the anchor cannot find its
  element, drops the anchor, and corrects nothing. Measured: **655px of history
  arrived and `scrollTop` held at 23**, leaving the reader at the top of a block
  they never asked to see.
- With the reader left at the head, the paging boundary never re-arms and
  history becomes unreachable.

The arriving items are estimates until they measure, so this lands close rather
than exactly; the anchor — which can now find its Turn — takes it the rest of the
way. It is skipped while follow-output owns the viewport, which re-asserts its
own target every frame.

## The Viewport Anchor Owns Scroll Compensation

A virtualizer places items in the scroll range before it knows how tall they
are, so every late measurement rewrites the offset of everything below it.
Correcting for that is unavoidable. Doing it with `scrollTop` is not possible:
the same number means a different place after every measurement. The reading
position is therefore recorded as a **Turn and its offset from the viewport
top**, and restored as a relationship rather than replayed as a delta. That
makes the correction idempotent — when nothing moved it is zero.

**The anchor is the only compensator.** The virtualizer's own adjustment is
turned off (see *What Belongs to the Virtualizer*) because it replays a delta
against a scroll position it learns about a frame late. Restoring a relationship
has no base to go stale, which is the whole reason this is the one that stays.

This was not always true, and what happened when it was not is why the rule is
written down. react-virtuoso corrected by the change in *total* list height,
which assumes the change happened above the viewport — scrolling up into a
freshly paged block guarantees it did not, and one item measuring 38px -> 1003px
moved the viewport 965px, across a whole Turn. Worse, the correction was gated
on scroll direction, and its own prepend compensation set that direction to
`down`, disabling it for exactly the measurements that followed: `scrollHeight`
went 8393 -> 10073 with `scrollTop` held at 1133 and no correction at all,
sliding the transcript down by the full 1680px. Those corrections had to be
intercepted at `scrollBy` and answered by re-anchoring. **Do not reintroduce a
compensator whose amount is a total rather than a delta.**

**Capture is qualified by intent, not by geometry.** A scroll event cannot say
whether the user moved or the transcript moved under them, so the anchor is
re-taken only within `USER_DRIVEN_SCROLL_WINDOW_MS` of a wheel, touch, key, or
scrollbar press — the same distinction follow-output draws. Two rules were tried
and measured first, and both failed in ways worth recording:

- Capturing at the intent event itself records the position *before* the scroll
  it causes, which drags a scrolling viewport backwards.
- Gating on "the content height did not change" blocks almost every capture,
  because lazy measurement changes it on nearly every frame: 1075 blocked
  captures against 8 accepted ones, and a 1037px correction issued against the
  user's own gesture.

**Restoring needs a window, not a callback.** A prepend settles over several
frames — a margin holds the position, the real heights land in padding, then the
margin is released — and *a margin change fires no ResizeObserver at all*, so no
single callback covers it. Every signal that the transcript moved therefore
opens `ANCHOR_SETTLE_FRAMES`, and a frame that had to correct refreshes it.
Measured before the window existed: four consecutive painted frames displaced by
896px.

The observer feeding this had to be repointed. `scrollerRef.firstElementChild`
is a viewport-sized box that stays at the scroller's height no matter how much
transcript there is — it never reported a content change at all, despite a
comment claiming it watched content. The item list is the element that grows,
and `border-box` is required because the virtualizer parks item space in
padding.

**The keeper does not know there is a virtualizer.** It lives in
`flowChatViewportAnchor.ts` (geometry and the DOM contract for the anchor
element) and `useFlowChatViewportAnchor.ts` (capture, restore, and the settle
window), and it talks to a scroller element and the Turns rendered inside it and
to nothing else. That is what let the virtualizer underneath it be replaced
without the keeper changing at all.

One consequence of the refresh rule is worth stating plainly: a frame that finds
the anchor already in place still counts as answered, so an open transcript that
no other writer owns holds one animation frame in flight indefinitely. The cost
is a `querySelectorAll` and two rect reads per frame. The window only winds down
once there is no anchor to keep.

**What the anchor cannot fix on its own** is a scroll range that was wrong to
begin with. Holding the reading position across a burst of measurement is worth
nothing if the burst blocks the main thread for 295ms. That is a property of how
unmeasured items are reserved, not of the anchor, and it is why the virtualizer
underneath it takes a per-item estimate.

## Reading History Is About the Transcript, Not the Intent

`viewportMode: 'history-reading'` does two things — it suppresses streaming
follow, and it pins the jump-to-latest bar open and routes it through a
presentation reset. Both are asking one question: **does the transcript on
screen still reach the newest Turn?**

A turn-navigation viewport intent used to answer that faithfully, because turn
navigation was the only thing that activated a history window. It is not any
more. A session whose loaded tail is shorter than the viewport pages older Turns
in the moment it opens, with nobody navigating, and the first paging step has to
set a turn intent — `isShowingHistoryPresentation` requires one, so without it
the paged-in Turns would not render at all. The viewport sitting on the newest
output was therefore reported as reading history: the bar was visible from the
moment the session opened, clicking it dropped the window and paged it straight
back in, and streaming output was not followed at all.

`flowChatLiveTailWindow.ts` answers it from the window's own ordinals instead.
These are ledger numbers, not measurements — the rule against inferring intent
from geometry is about ambiguous quantities like `scrollTop`, and does not
apply. The answer also keeps up on its own: a Turn arriving past the end of the
window flips it back with no help, where a flag recorded at activation time
would go stale and leave no way to the live tail.

`isReadingTurnViewport` keeps its old meaning for the auto-tail placement, which
asks a third question again — who owns the viewport. Merging those two is the
mistake this separates.

**A tail-anchored window must grow with the session.** It stops at the newest
Turn that existed when it was cut, and nothing moves its end afterwards, so an
appended Turn is simply not rendered. That is worse than it sounds: `latestTurnId`
is read off the rendered items, so follow-output never learns the Turn exists —
no pin, no follow, and nothing to scroll to. `resolveTailWindowGrowth` is
level-triggered for that reason. An edge — "it reached the tail last render and
does not now" — is consumed whether or not the extension succeeded, stranding
the window permanently on one failure; the current state stays `'extend'` until
the window is actually repaired. A window the user navigated to has a different
end, so the session growing says nothing about it and it is left alone. When the
store cannot extend far enough, the fallback drops back to the canonical tail:
that costs a visible re-page of the history above, which is why it is the
fallback, but it is the only branch that always shows the message just sent.

## Two Refusals Stand Between an Ask and a Page

`flowChatHistoryBoundary.ts` decides that a boundary is worth asking about.
Whether the ask is honoured is the container's, and it declines twice:

**While follow-output owns the viewport.** The position the ask was derived from
is then our own placement, not the reader's — as true of a history window being
opened as of the live tail, so the test is ownership and not presentation mode.
Ownership ends the moment the reader scrolls, which is exactly when the ask
starts meaning something.

**Until the visible range has left that boundary.** Prepend compensation puts
the viewport back on the reader's content, but the virtualizer places its rows
from a scroll offset it refreshes a frame later, so for one commit the visible
range is still read against the head. A direction is disarmed on dispatch and
armed again by the range leaving it; an ask that resolves to anything other than
`applied` re-arms immediately, because nothing was prepended and the range
sitting at the boundary is still the reader's own position.

Both were free under react-virtuoso: `firstItemIndex` moved the reported range
with the prepend, so the local start index jumped by the number of items added
and the rule stopped applying by itself.

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
also the only place where one FlowChat writer defers to another; it is not a
coordinator, and the reasons there is no coordinator are unchanged (see below).

## Current Behavior

- A newly submitted Turn scrolls to the viewport top and enters follow-output.
  Every other entry reason resumes at the end of real content, with one
  exception: a jump to latest while the **newest** Turn is still pinned returns
  to the pin. That mode only holds while the Turn's answer is shorter than one
  viewport, so everything it has produced is already on screen, and aiming at
  the content end would scroll *up* and shove the message the user just sent
  into the middle. It is also the landing place the snap back picks for the same
  viewport state — having the two disagree would be worse than either choice.
  The exemption therefore outlives the Turn: a short Turn stays pinned until a
  newer one replaces it.
- Top-aligning a Turn aims at `FLOWCHAT_TURN_TOP_GAP_PX` above its user message,
  not at the message itself. The first Turn already sits below that gap for
  free, because `.message-list-header` occupies it at the head of the scroll
  content; every other Turn used to land flat on the top edge, and the two read
  as different alignments. The header renders at the same constant so they
  cannot drift. Both the one-shot scroll and the offset the follow loop
  re-asserts every frame carry it — if only one did, they would fight.
- Turn navigation never scrolls into the reserved blank to top-align a Turn. A
  Turn whose top lies past the content end is stopped at the content end
  instead, which is where the tail rests. The blank belongs to follow-output —
  `pin-turn-top` holds it for output that is arriving, and nothing arrives under
  a Turn the user navigated to. There is no "is this the last Turn" test and no
  measurement of what lies below it: a Turn with a viewport of content under it
  has its top above the content end already, so the clamp does not bind and the
  final Turns of a long transcript still top-align. Before the resident spacer
  the browser did this for free by clamping at the end of the scroll range.
- The clamp branches on what is *knowable*, not on where the Turn is. A rendered
  Turn resolves its own offset, so the decision is made before anything moves
  and the requested `behavior` survives. An unrendered one is known only to the
  virtualizer, so it is placed with `behavior: 'auto'` and the landing read back;
  an animated placement would not have arrived yet, so there would be nothing to
  read. Both writes land in the same task, so the correction costs a second
  scroll but not a second visible movement.
- A navigation correcting its own scroll must re-issue through the virtualizer,
  never by writing the scroller. The virtualizer keeps re-aiming at its last
  target for as long as the measurements under it move, and only another scroll
  issued through it replaces that. Writing the scroller directly is what left
  the tail Turn top-aligning, being pulled to the content end, and being
  re-aimed at the top again.
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
- A scrollbar drag is the one exception, and the press is what makes it one: a
  pointer held past the content box's trailing edge is on the bar, so the
  scrolling it causes *is* intent. The press only arms it — `scrollbar-gutter:
  stable` keeps the gutter reserved whether or not a bar is drawn there, so a
  press that scrolls nothing changes nothing. Unqualified, a drag never released
  the viewport: measured on WebView2, follow-output rewrote its target against
  the thumb every frame for a 100px oscillation, and a drag that came to rest in
  the reserved blank was skipped by the snap back because follow still nominally
  owned it — with the frame loop long since asleep, so nothing corrected it.
- Ownership outliving the frame loop is deliberate — streaming has to be able to
  resume follow after the settle budget runs out — so the snap back asks whether
  follow is **correcting** the viewport, not whether it owns it. A live loop gets
  the viewport to itself, since it reaches its target in one frame and a snap
  back would only race it. An asleep one does not: a viewport left in the
  reserved blank under a sleeping loop is stranded, and nothing else was watching
  for it. This is the half of the scrollbar problem that is fixed everywhere,
  including where the drag itself cannot be recognised.
- The pinned Turn's offset is re-resolved from live layout every frame. Items
  above it are estimates until they are measured, so a cached absolute offset
  would drift.
- When streaming stops, `hold-tail` settles any remaining blank with one smooth
  scroll. A pinned Turn does not settle.
- Tool-card expansion and collapse use normal layout reflow and dispatch only
  `tool-card-toggle`. There is still no pre-collapse intent event and no
  per-card compensation.
- "At bottom" is a band, not a point: from the end of real content down to
  whatever the follow rule owns. A pinned Turn and a held collapse gap are both
  inside it, so neither raises the jump-to-latest affordance; the reserved blank
  is outside it, so parking there does. No virtualizer-reported "at bottom" can
  express this: the end of the scroll range is the bottom of the reserved blank,
  not the end of content.
- The band is recomputed on scroll, on resize, **and when follow ownership
  changes** — its lower edge is the follow target, which can move while the
  viewport is perfectly still. A snap back completes at rest by construction,
  and a jump to latest that lands on a pin the viewport already sits on writes
  nothing at all. Driving the band from scroll events alone left the affordance
  visible over a viewport that was at the tail, and clicking it then had nothing
  to do — an inert button is worse than a missing one.

## What Belongs to the Virtualizer

FlowChat virtualizes with **TanStack Virtual**, behind `useFlowChatVirtualizer.ts`.
Nothing else imports it. The rest of FlowChat asks for offsets in scroller
coordinates and gets them back; there is no index space of the virtualizer's own
to convert at the edges, because measurements are cached against **item keys**,
so a history prepend leaves every measured item exactly where it was.

That is only half of what react-virtuoso's `firstItemIndex` did, and the other
half has to be supplied — see *Keeping the Viewport on the Reader's Content*.

The reason it is TanStack and not react-virtuoso is one line of its measurement
pass: `size = measured ?? estimateSize(i)`. A per-item estimate for everything
unmeasured. react-virtuoso reserves a single scalar (`lastSize`) for all of
them, and this transcript alternates 38px user messages with model rounds up to
5012px, so the scroll range was wrong by an order of magnitude until an item was
actually measured. `estimateVirtualMessageItemHeight` now feeds it directly.

**Items stay in normal flow inside a padded window**, not absolutely positioned.
Everything outside the window stands in as `padding-top` and `padding-bottom`
(`virtualWindowPaddingPx`). This matters for more than tidiness: when an item
inside the window changes height, the browser reflows the ones below it in the
same layout pass, so there is no frame where the scroll has been corrected but
the items have not moved yet.

**The virtualizer does not compensate for its own late measurements.**
`shouldAdjustScrollPositionOnItemSizeChange` is set to refuse, always. Its rule
is the right shape — this item's delta, only for an item above the viewport —
but it applies that delta to `scrollOffset`, the library's own copy of the
scroll position, refreshed only from scroll events. Every continuous writer here
assigns `scrollTop` directly and the matching scroll event lands a frame later,
so a measurement arriving in between is compensated from a position the viewport
has already left. Measured on session open: **nine corrections across two frames
walked the viewport from 7440 back to 3556**, and the follow loop wrote 7440
again on the next frame. The interception this replaces was written for
react-virtuoso and removed on the assumption that TanStack asked the right
question. It does — from a stale base.

**Alignment is asked for, not computed, wherever it fits.** `scrollItemIntoView`
goes through the virtualizer so that its re-aim keeps chasing the item while the
measurements under it move; an offset computed once is already stale by then.
The gap above a top-aligned Turn is the virtualizer's `scrollPaddingStart`, for
the same reason. Only two places compute an offset by hand, and both do it
because the target is not an item: the end of *real content*, which is above the
resident tail spacer, and the end of a Turn.

Two things that look like they belong here do not:

- **Positions in `virtualItems`.** That array is FlowChat's own projection, so
  an index into it means the same thing under any virtualizer. `scrollToIndex`,
  `scrollToSearchMatch`, and `data-virtual-index` all carry one and are left
  alone.
- **When to page.** `historyBoundariesForVisibleRange` decides that a boundary
  is worth asking about from a visible item range and nothing else. Its two
  thresholds are the ones that decide where a junction happens, which is why
  they are named and tested rather than inline.

**Visible is not rendered.** `getVisibleItemRange` intersects the rows with the
scroller box; the rendered window carries overscan, and a transcript short
enough to render whole reports the first *and* last item present wherever the
viewport stands. Feeding the rendered window to a rule that means "has the
reader arrived here" asks whether the item exists instead. Measured: a 21-item
transcript rendered rows 0..20 from index 0 no matter where the reader was, so
the head boundary read as reached forever. It has to be a callback rather than a
value, because a scroll moves the viewport across the window without changing
it.

react-virtuoso remains a dependency: the file tree (`VirtualFileTree.tsx`) still
uses it. Nothing under `flow_chat/` does.

## Known Gaps

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
  at once, but the work each one costs is unchanged.

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
compensation engine. Single-writer semantics are not reachable here: the
virtualizer writes `scrollTop` from inside the library — its own re-aim, and its
adjustment for a re-measured item above the viewport — so a coordinator can only
serialise *our* writes, and the conflicts observed in practice were with that
third writer. What made those conflicts survivable was not serialising them but
making our correction idempotent; see the viewport anchor.

What works instead is a single *source of truth*. Read the live viewport, and
make each phase's rule idempotent with respect to a foreign write: authoritative
while hidden, cooperative once painted.

## Viewport Ownership

The virtualizer never follows output. Continuous movement belongs to
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

## Verification

Run the smallest relevant automated checks:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run \
  src/flow_chat/components/modern/flowChatTailFollow.test.ts \
  src/flow_chat/components/modern/flowChatLiveTailWindow.test.ts \
  src/flow_chat/components/modern/flowChatHistoryBoundary.test.ts \
  src/flow_chat/components/modern/useFlowChatVirtualizer.test.ts \
  src/flow_chat/components/modern/flowChatViewportAnchor.test.ts \
  src/flow_chat/components/modern/useFlowChatViewportAnchor.test.tsx \
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
12. Open a session long enough to be `isPartial` — the loaded tail is shorter
    than the viewport, so it pages older Turns in on its own. No jump-to-latest
    bar should appear, and streaming output should be followed. Then send a
    message: it must appear immediately and pin to the viewport top, with the
    history above neither moving nor reloading.
13. With a short Turn pinned, scroll up, jump to latest, then scroll down into
    the blank and let go. After the snap back the jump-to-latest affordance must
    be gone — this is the one path where the viewport arrives at the tail
    without a scroll event to notice it.
14. Send a one-line message and let it pin. It must come to rest at the top with
    the same small gap above it as the very first Turn of the session, and the
    pin must hold steady rather than creeping down — a pinned offset past the
    end of the scroll range is clamped, and the follow loop will rewrite it
    every frame.
15. Drag the scrollbar to the very bottom. The screen must not be entirely
    blank: the last Turn and the input clearance stay visible above the
    reservation. Repeat with the composer expanded, which is where the reserve
    falls back to the hold-gap floor.
16. Drag the scrollbar, without touching the wheel first, down into the reserved
    blank and let go: it must snap back. Then drag it while output streams: the
    transcript must follow the thumb without the frame loop fighting it. A press
    on the thumb that moves nothing must leave the viewport alone.
17. Click the last Turn on the Turn Rail while it is short. It must land in one
    movement at the end of the transcript — no top-align followed by a slide
    back down. Do it from near the tail *and* from the top of a long session:
    those are the rendered and unrendered branches, and they take different
    paths. Then click a final Turn whose answer is longer than the viewport: it
    must still top-align.
18. Open a long `isPartial` session and scroll up slowly through several paging
    junctions. The Turn under the cursor must not move — not backwards, not
    forwards, and not for a single frame. A stall while a page is measured is a
    known gap and reads differently from a jump: the picture freezes and
    resumes in place, rather than showing different content and snapping back.
    Then scroll up fast through the same junctions, which is where the anchor
    and the user's gesture are most likely to disagree.
19. During that scroll, check that a paging junction does not leave the viewport
    stuck: keep scrolling past it, then wheel back down, and confirm the
    transcript still tracks the gesture in both directions.
20. Open a long session and check the scrollbar thumb: its size should be close
    to right on the first painted frame, and it should not jump as items
    measure. This is the per-item estimate doing its job, and it is the single
    most visible symptom if the estimate ever regresses.
21. Scroll to the very bottom and confirm the transcript ends where content
    ends, with the reserved blank below it reachable but not where the session
    opens.
22. Expand and collapse a tall tool card near the top of the viewport, and one
    below it, and confirm earlier content stays put in both cases.
23. Open a long `isPartial` session and leave it alone. Nothing may page in
    behind the reveal: the transcript opens on its loaded tail and stays there.
    Five pages arriving over 890ms is what this checks for, and the reveal only
    hides the first frame of it.
24. Scroll up to a junction. **One** page loads, the Turn under the cursor stays
    where it is, and paging stops until the head is reached again. Then keep
    going: every junction must behave the same way all the way to the first
    Turn, with no run of pages and no point where scrolling up stops doing
    anything.

## Related Files

- `flowChatTailFollow.ts`
- `flowChatLiveTailWindow.ts`
- `useFlowChatVirtualizer.ts`
- `flowChatHistoryBoundary.ts`
- `flowChatViewportAnchor.ts`
- `useFlowChatViewportAnchor.ts`
- `useFlowChatFollowOutput.ts`
- `VirtualMessageList.tsx`
- `ModernFlowChatContainer.tsx`
- `../../utils/flowChatScrollLayout.ts`
- `../../tool-cards/useToolCardHeightContract.ts`
