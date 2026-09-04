# FlowChat Virtualization

What the virtualization library is allowed to decide, what stays ours, and the
one rule about rendering that only makes sense once a row's lifetime is shorter
than its content's.

## What Belongs to the Virtualizer

FlowChat virtualizes with **TanStack Virtual**, behind `useFlowChatVirtualizer.ts`.
Nothing else imports it. The rest of FlowChat asks for offsets in scroller
coordinates and gets them back; there is no index space of the virtualizer's own
to convert at the edges, because measurements are cached against **item keys**,
so a history prepend leaves every measured item exactly where it was.

That is only half of what react-virtuoso's `firstItemIndex` did, and the other
half has to be supplied — see *Keeping the Viewport on the Reader's Content* in
`FLOWCHAT_HISTORY_PAGING.md`.

The reason it is TanStack and not react-virtuoso is one line of its measurement
pass: `size = measured ?? estimateSize(i)`. A per-item estimate for everything
unmeasured. react-virtuoso reserves a single scalar (`lastSize`) for all of
them, and this transcript alternates 38px user messages with model rounds up to
5012px, so the scroll range was wrong by an order of magnitude until an item was
actually measured. `estimateVirtualMessageItemHeightWithContext` now feeds it
directly. The estimate is owned by the data shape in
`virtualItemHeightEstimators.ts`: text, thinking, user messages, model rounds,
Explore groups, and tool families each derive a bounded height from their
content, status, width, and expansion state. This code is pure and runs before
a row has a DOM node. Once mounted, DOM measurement remains authoritative and
replaces the estimate. Width and volatile Explore expansion changes invalidate
only the derived position pass; TanStack's key-based measured-size cache is
retained.

**Items stay in normal flow inside a padded window**, not absolutely positioned.
Everything outside the window stands in as `padding-top` and `padding-bottom`
(`virtualWindowPaddingPx`). This matters for more than tidiness: when an item
inside the window changes height, the browser reflows the ones below it in the
same layout pass, so there is no frame where the scroll has been corrected but
the items have not moved yet.

**The virtualizer does not use TanStack's own late-measurement adjustment.**
`shouldAdjustScrollPositionOnItemSizeChange` reads the real scroller position
and asks the viewport owner to apply the delta only when the whole item is above
the viewport. A partly visible row is left alone because its changed content is
inside what the reader is looking at. TanStack's adjustment is always refused:
it applies its delta to `scrollOffset`, the library's copy refreshed only from
scroll events. Every continuous writer here assigns `scrollTop` directly and
the matching scroll event lands a frame later, so that base can be stale. The
owner's displacement is applied before the new size enters the cache, while
the anchor remains responsible for restoring relationships across larger layout
transactions.

The measurement decision is recorded as the switch-gated, coalesced
`virtualizer.itemResize` probe: item identity, estimated and measured sizes,
the above-viewport decision, and the real scroll geometry before and after the
owner's displacement. It deliberately omits flow-item contents, which made the
temporary investigation probe too large for a lasting diagnostic trail.

**Measurement is forced before any position is read in the commit that changed
the items.** The library skips its inline resize while the reader is scrolling,
which is exactly when history arrives, so the cache holds reserved estimates
until the ResizeObserver delivers a frame later.
`virtualizer.measureRenderedItems()` does that reconciliation itself — the same
work, a frame earlier, free for any row whose height was already right. The
evidence and the numbers are in *A Displacement Is Not a Movement* in
`FLOWCHAT_HISTORY_PAGING.md`.

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
  is worth asking about, from where the reader stands and nothing else. Its
  thresholds are the ones that decide *where* a junction happens, which is why
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

## The Projection Is the Stable Thing

Stable virtual-item keys and projection identity are required. Do not split one
`ModelRound` into multiple virtual items, and do not reclassify projection from
a timer.

`getVirtualItemStableKey` keys on type, Turn and content id — never on an index.
That is what lets a prepend renumber every row without React unmounting any of
them, and it is what the measurement cache is keyed on underneath.

Tool cards reflow naturally and dispatch only `tool-card-toggle` after an
expanded-state change, so the virtualizer can remeasure. There is no
pre-collapse intent event and no per-card compensation.

## A Row's Mount Is Not an Arrival

**No mount or enter animation may live inside `.virtual-item-wrapper`**, no
mount-triggered motion may change transcript geometry, and nothing may be keyed
on a state change a scroll can replay.

Outside a virtualized list an element's insertion means its content arrived, and
a fade or a slide says so honestly. Here insertion means the row entered the
rendered window. Paging up mounts the Turns the page brought, the rows the
junction's own correction scrolls past, and every row the reader scrolls back
over afterwards — each one replaying whatever its stylesheet attached to mount.
`--streaming` to `--complete` is the same mistake in a different key: it fires
when the typewriter finishes, which is not when the reader is looking.

The one that shipped was `.markdown-renderer`, from the shared component
library: `animation: fadeIn var(--openbitfun-motion-duration-base) ease-out`,
350ms from `opacity: 0`. Once the junction displacement was down to tens of
pixels that fade was the entire remaining complaint — most of the screen
dimming and coming back on every page up. `VirtualItemRenderer.scss` cancels it
for anything inside a row and leaves the library alone, where a markdown block
really is mounted once.

The rule is stated here because four correct local fixes could not reach it.
`ModelRoundItem.scss` and `UserMessageItem.scss` each refuse an enter animation
of their own, in comments that name this reason. `FlowTextBlock`'s typewriter
refuses to replay on mount, because a streaming block that scrolled out and
back would restart from an empty string and re-grow. `FlowTextBlock.scss`
cancels this very fade — but only under `.streaming`, so the one block still
being written was exempt and the whole of history was not. Each author saw the
defect, guarded their own file, and had no way to guard a component in another
package.

## Related Files

- `useFlowChatVirtualizer.ts`
- `virtualMessageListLayout.ts`
- `VirtualItemRenderer.tsx` + `.scss`
- `VirtualMessageList.tsx`
