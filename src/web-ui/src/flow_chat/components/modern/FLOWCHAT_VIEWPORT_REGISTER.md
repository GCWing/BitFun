# FlowChat Viewport Register

Every deliberate write to the FlowChat scroller goes through one register, which
decides whether the writer may act. This document covers the register, what it
replaced, the two writers that stand outside it, and the trail it leaves.

## Who May Move the Viewport at All

Continuous movement belongs to `useFlowChatFollowOutput`; one-shot navigation
belongs to `VirtualMessageList`. The virtualizer never follows output. Card
renderers and tool cards must not write the outer FlowChat `scrollTop` — local
scroll surfaces inside a thinking, explore, terminal, or subagent card may
manage their own scroll position, but they must not dispatch an outer viewport
compensation request.

## The Register and Its Order

Every deliberate write goes through `useFlowChatViewportOwner`, which asks
`flowChatViewportOwnership.ts` whether the writer may act. The order is the
design:

| | Owner | Held while |
|---|---|---|
| 1 | `user-gesture` | 200ms from the last wheel, touch, key, or scrollbar press |
| 2 | `one-shot-navigation` | a Turn, search hit, or focus request is being reached |
| 3 | `snap-back` | the return from the reserved blank is animating |
| 4 | `follow-output` | the continuous writer owns the viewport |
| 5 | `layout-correction` | the scroller's box changed and a resting viewport is re-aligned |

The register decides **whether**, never **where** — targets stay with the writer
that owns them, and the anchor's correction stays idempotent. Ordering answers
what idempotency cannot: whether a movement was ours on purpose or something to
be undone. The anchor is idempotent and still destroyed the snap back, because
it restored a relationship we had deliberately changed.

**Repairing a displacement is not on this list**, and putting it there was a
mistake worth keeping recorded. The prepend compensation and the viewport anchor
both use `viewportOwner.shift` — see *A Displacement Is Not a Movement* in
`FLOWCHAT_HISTORY_PAGING.md`.

**Taking ownership and writing are one call.** Adding a writer without declaring
it means not using the helper, which is visible in review — where the previous
scheme needed the new writer added by hand to every other writer's private
predicate, and a single missed pair was a bug. The failure that made this
explicit: a snap back was missing from the hand-written predicate that preceded
the register, so it belonged to nobody while it animated. The snap travelled
0.7px, the anchor wrote it back, the write cancelled the animation, the
cancellation read as a gesture coming to rest, and the snap was issued again —
**958 times over 20 seconds, arriving nowhere**.

**Only an owner that releases may hold the viewport indefinitely**, and
follow-output is the only one — a claim with no expiry and no release is a
viewport nothing below it can ever write again. Every other writer states its
own window: a gesture goes quiet, an animation may never report completion, a
re-aim runs while measurements settle. Saying nothing means the write is
instantaneous and owns only itself, which is what a correction wants; the claim
still resolves against whoever holds the viewport, because that is the question
deciding whether the write happens at all.

The first thing this caught was its own: the virtualizer places the scroller
against its own state on mount, before any aim of ours exists. That 0px write
was attributed to a navigation and took an unbounded claim, so follow-output was
refused for the whole opening reveal and a long session opened at the head of
its loaded window instead of on the newest Turn. A library write nobody asked
for is now attributed to `layout-correction`, which is what it is.

**The opening reveal is not an owner.** It is a phase, and the thing moving the
viewport during it is follow-output, so a claim standing in for the reveal would
outrank follow-output and refuse it. One slot cannot hold a phase and a writer
at once, so the reveal remains an explicit condition beside the register.

Not everything collapsed into it, and the difference is worth keeping straight.
`smoothScrollFramesRef` is follow-output yielding to *its own* animation, which
is intra-owner; `settleFramesRef` is a frame budget; `pendingSnapBackTargetRef`
still answers "did our snap land here"; `boundaryArmedRef` is paging policy.
Only the parts that were really answering "is someone else moving the viewport"
are gone.

## What Counts as a Gesture

Ordinary `scroll` events do not transfer viewport ownership; only explicit
wheel, touch, or keyboard navigation exits follow-output. A gesture that comes
to rest inside the reserved blank hands it back — see *Snapping Back Out of the
Reserved Blank* in `FLOWCHAT_SCROLL_STABILITY.md`.

A scrollbar drag is the one exception, and the press is what makes it one: a
pointer held past the content box's trailing edge is on the bar, so the
scrolling it causes *is* intent. The press only arms it — `scrollbar-gutter:
stable` keeps the gutter reserved whether or not a bar is drawn there, so a
press that scrolls nothing changes nothing. Unqualified, a drag never released
the viewport: measured on WebView2, follow-output rewrote its target against the
thumb every frame for a 100px oscillation, and a drag that came to rest in the
reserved blank was skipped by the snap back because follow still nominally owned
it — with the frame loop long since asleep, so nothing corrected it.

## Reaching a Turn Is One Shot

One-shot Turn, search and history navigation lives in `VirtualMessageList`, and
holds `one-shot-navigation` for as long as it is still arriving.

**Alignment is asked for, not computed, wherever it fits.** A navigation
correcting its own scroll must re-issue through the virtualizer, never by
writing the scroller. The virtualizer keeps re-aiming at its last target for as
long as the measurements under it move, and only another scroll issued through
it replaces that. Writing the scroller directly is what left the tail Turn
top-aligning, being pulled to the content end, and being re-aimed at the top
again.

**The clamp branches on what is knowable, not on where the Turn is.** A rendered
Turn resolves its own offset, so the decision is made before anything moves and
the requested `behavior` survives. An unrendered one is known only to the
virtualizer, so it is placed with `behavior: 'auto'` and the landing read back;
an animated placement would not have arrived yet, so there would be nothing to
read. Both writes land in the same task, so the correction costs a second scroll
but not a second visible movement.

**Turn navigation never scrolls into the reserved blank to top-align a Turn.** A
Turn whose top lies past the content end is stopped at the content end instead,
which is where the tail rests. The blank belongs to follow-output — `pin-turn-top`
holds it for output that is arriving, and nothing arrives under a Turn the user
navigated to. There is no "is this the last Turn" test and no measurement of
what lies below it: a Turn with a viewport of content under it has its top above
the content end already, so the clamp does not bind and the final Turns of a
long transcript still top-align. Before the resident spacer the browser did this
for free by clamping at the end of the scroll range.

**Top-aligning a Turn aims at `FLOWCHAT_TURN_TOP_GAP_PX` above its user
message**, not at the message itself. The first Turn already sits below that gap
for free, because `.message-list-header` occupies it at the head of the scroll
content; every other Turn used to land flat on the top edge, and the two read as
different alignments. The header renders at the same constant so they cannot
drift. Both the one-shot scroll and the offset the follow loop re-asserts every
frame carry it — if only one did, they would fight.

**A gesture preempts a navigation still in flight.** The library's re-aim keeps
computing for up to 5s after the scroll that started it, and every write it
attempts during that window is refused once the reader takes over. The cost is
accepted: a distant navigation that the wheel brushes stops where it is and is
not corrected further.

## Why There Was No Coordinator Before, and What Changed

There was one — `FlowChatViewportCoordinator.ts`, removed alongside the
compensation engine, and it was a compensation engine itself: reservations,
pin and collapse compensation, element-anchor leases, a synthetic bottom range.
Nothing here does any of that.

The argument recorded against replacing it was that single-writer semantics were
unreachable, because the virtualizer writes `scrollTop` from inside the library —
its own re-aim, and its adjustment for a re-measured item — so a coordinator
could only serialise *our* writes while the conflicts in practice were with that
third writer. Two things retired that premise:

- The library's adjustment for a re-measured item is **off**
  (`shouldAdjustScrollPositionOnItemSizeChange`), because it replayed a delta
  against a scroll offset it learns about a frame late.
- **`scrollToFn` is a first-class virtualizer option.** Every write the library
  makes — `scrollToIndex`, `scrollToOffset`, and the re-aim that follows them —
  goes through a function we supply, so it is registered like any other.

What remains outside the register is the reader and the browser, and browser
scroll anchoring is off. A library write is attributed to whoever asked for the
aim, which is also what lets a gesture preempt a navigation still chasing its
Turn.

## Diagnosing the Viewport

Viewport faults are intermittent, leave nothing in the DOM once they are over,
and read identically to two or three other causes: a Turn that lands and is
dragged away and a Turn that never landed are the same complaint. So the trail
is permanent, in `flowchat.log`, behind `app.logging.flow_chat_diagnostics` —
the same switch history paging uses — and tagged `viewport`.

`flowChatViewportDiagnostics.ts` records two different kinds of thing:

| | Where | What |
|---|---|---|
| **Writes** | the register, `viewportOwner.write` / `.claim` / `.release` | who moved the viewport, from where to where, and **who was refused** |
| **Decisions** | each writer | why it wanted to move, and why it did not |

The second half is the one that pays. A write that never happened leaves nothing
at the register to find, and "nothing happened" has been the report more often
than a wrong move has: a deferred new Turn, an anchor whose Turn left the
rendered window, a snap back declined because the settle belonged to the opening
reveal, a boundary that never re-armed. Each of those is now one line saying
which.

**A placement is recorded with what became of it.** `traceViewportPlacement`
samples the offset on the next frame and again once things have settled, and
reports the drift from the target. Read against the register's writes in the
same window, the drift says *who* took it away. This is how the two writers that
do not go through the register — the sticky Task indicator and cross-session
focus — are watched without being changed.

Everything here can fire on every frame, so repeated identical events collapse
into one entry per 500ms carrying the count and travel it stands for. The key
includes whatever makes one run a different run — the owner, the outcome, the
direction — so a *transition* always emits immediately. A thousand copies of a
steady state would only bury the transitions, which are the point.

**A duration is reported once, when it ends.** Coalescing is what makes the
per-frame traces readable, and it is also why they cannot answer "how long did
that last": only the first of a run is emitted, and the rest are a count.
`anchor.turnReturned` and `anchor.waitAbandoned` therefore report the whole of a
wait for the anchored Turn — milliseconds, settle frames, restore attempts, and
the reader travel carried through it — at the one moment the whole of it exists.
That wait is what decides whether a junction is seen, so it is the number to
read first. These are the only traces here that go through `traceViewport`
rather than the coalescer, because they fire once per wait and their payload is
the point rather than a sample of it.

**Read the state out before the state changes.** A `data` callback is evaluated
by `flowChatDiagnostics.trace`, synchronously, so a thunk over refs is normally
exactly right. It is not right when the caller resets those refs on the next
line, and the wait report does: it takes its numbers eagerly and closes over
them.

**Two numbers say whether a correction was a mistake.** `scrollRangePx` is
recorded on `prependCompensated` and on every `anchor.correct`, because a
correction and the reason for one read the same otherwise. Measured over five
junctions, the correction equalled the change in the scroll range every time
(−94/−93.4, +76/+76.5, +26/+30.7, +8/+7.6): the compensation had not over-shot,
the transcript above the reader had re-measured, and the anchor was following
it. A correction with the range *unchanged* would be the other diagnosis.

Nothing evaluates a payload while the switch is off.

### Reading the log

```text
pnpm run flowchat:log:analyze -- <path-to-flowchat.log> [--around <sequence>]
```

`scripts/diagnostics/analyze-flowchat-log.mjs` reports, in this order:

1. **Episodes** of viewport activity, worst *churn* first — travel per pixel of
   progress. A clean move is 1. The snap back that never arrived would be
   hundreds, and that number is the difference between "it moved wrongly" and
   "it fought".
2. **Placements that did not stick**, ranked by drift. A placement with no
   outcome sampled is listed separately rather than counted as clean.
3. **Refusals**, as owner × who outranked them.
4. **Declines**, as writer × reason.

Two things it is careful about, because both would otherwise flatter the
result: a coalesced entry is weighed by the run it stands for, not as one event;
and dropped entries are reported at the top, since every count below one is a
lower bound.

The paging side of the same fault is traced separately; see *Diagnosing History
Paging* in `FLOWCHAT_HISTORY_PAGING.md`.

## Related Files

- `flowChatViewportOwnership.ts`
- `useFlowChatViewportOwner.ts`
- `@/infrastructure/diagnostics/flowChatViewportDiagnostics.ts`
- `VirtualMessageList.tsx`
- `useFlowChatNavigation.ts`
- `scripts/diagnostics/analyze-flowchat-log.mjs`
