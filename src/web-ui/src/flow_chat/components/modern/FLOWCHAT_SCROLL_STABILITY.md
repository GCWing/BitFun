# FlowChat Scroll Stability

This document explains the scroll-stability mechanism used by `VirtualMessageList.tsx`.

## Rule Zero: Do Not Create Motion For This Mechanism To Chase

Every rule below is compensation for content that changes size on its own. The
cheapest way to keep the pane stable is to not generate the movement in the
first place. Five invariants hold across the message list, and breaking any of
them reintroduces the "the chat keeps refreshing itself" report:

1. **Keep a live action's top-level projection identity stable.** A rendered
   `ModelRound` remains one `model-round` virtual item, and an explore-only
   round remains one `explore-group` virtual item with a stable key. Within a
   `ModelRound`, an active collapsible tool is intentionally kept as a critical
   item; after it settles it may join the surrounding explore grouping. That
   inner grouping transition must not split the round into multiple virtual
   items or replace the item/round keys, which would unmount the card and look
   like a flash. Likewise, never hide the old location with `display: none` as
   a handoff mechanism.
2. **No mount-triggered animation on anything the list renders.** The list is
   virtualized: an item that scrolls out of view unmounts and remounts, so a
   `fadeIn` / `slideInUp` keyed off mount replays on every pass. Same for an
   animation keyed off `--streaming` → `--complete`: it replays when the
   typewriter drains. `getModelRoundItemClassName` deliberately has no `--enter`
   modifier, and `.user-message-item` deliberately has no enter animation.
3. **Keep wall-clock state out of projection and grouping.**
   `sessionToVirtualItems` and `buildModelRoundItemGroups` remain pure
   functions of session data. A timer must not reclassify a round, change a
   `VirtualItem` key, or create a recently-completed projection. The card layer
   does have a bounded completion-preview timer (documented below), but it only
   changes local expanded state after the card is already rendered; it does not
   restructure the virtual list.
4. **Do not compact a live tail in the same completion commit.** The execution
   and file-operation cards use a short completion-preview grace period while
   they remain the expanded tail. A newer item still collapses them immediately;
   if no newer item arrives, they compact after the grace period. Task,
   question, thinking, and explore-group components retain their own
   status/last-item policies and are not implicitly covered by this timer. When
   an automatic collapse starts, it **may animate** for
   `FLOWCHAT_COLLAPSE_DURATION_MS` (300ms) as long as
   `flowchat:tool-card-collapse-intent` stays active for that full window plus
   settle frames. Instant collapse is reserved for `prefers-reduced-motion` or
   an explicit `disableAnimation` opt-out. Height, opacity, and transform must
   share one duration (see `flowChatCollapseMotion.ts` /
   `SmoothHeightCollapse`). Do not hard-swap `BaseToolCard` ↔ `CompactToolCard`
   for expand/collapse — that remounts the body with no height transition.
5. **Keep the leading edge stable across collapse states.** A revealed body
   must not add `margin-inline-start`, `padding-inline-start`, or an equivalent
   left offset relative to its collapsed header. Expanded thinking, explore
   rows, tool details, image previews, and subagent projections all begin on
   their owning message/card edge. Vertical and trailing-edge spacing may
   remain, but a leading inset reads as a horizontal jump during collapse.

A sixth, related rule lives in `useTypewriter`: `replayOnMount` defaults to
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

A non-zero `collapse` reservation must have an explicit semantic owner: an
active or retained tool collapse, input-stack shrink, preserved-element range,
late-shrink clamp, or protected-range transfer from a pin. Ordinary Virtuoso
measurement convergence has no such owner. An idle measurement with no owner
must only rebase the measured height and must never create Footer space from a
negative `scrollHeight` delta alone.

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
- a floor prevents unrelated shrink reconciliation from dropping live scroll range
- collapse floors still drain from measured content growth or deliberate downward
  user navigation; pin floors drain only through the sticky-pin settlement path
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

The logical `isFollowingOutput` flag follows the same ownership rule: it is
only true while the coordinator owns `following-tail`. A `sticky-latest` pin
clears the flag and arms its turn for handoff. Once collapse protection and
unsettled pin growth have drained, the handoff re-enters tail follow when
either the pin reservation is empty or the natural content tail (excluding
Footer reservations) reaches the viewport bottom. The latter condition avoids
making real content grow through stale synthetic pin space before follow can
start. This prevents a stale React render from allowing follow effects to
overwrite a pinned header.
The armed turn identity is owned only by `useFlowChatFollowOutput`; the list
must not mirror it in a second ref because session resume and pin preparation
can otherwise update the two identities in different commits.

Collapse anchors have three phases: active while CSS layout is changing,
retained-provisional while delayed virtualizer measurements may still arrive,
and settled-grace after the provisional estimate has been reconciled to current
DOM geometry. A short negative-layout quiet window ends the provisional phase.
The Footer is then reduced atomically to the minimum physical range needed by
the captured `scrollTop`; an anchor at `scrollTop === 0` needs no overflow range.
The semantic anchor remains retained through one final grace window, so a late
Virtuoso shrink can extend the range and restart settlement without exposing a
clamped frame. User navigation, a new pin, session reset, DOM disconnection, or
a quiet grace with no further correction releases it. Scroll
events enqueue semantic-element restores into the coordinator's single pending
animation frame. A transaction-owned non-user clamp may additionally extend its
physical range and restore the captured raw position synchronously before paint.
Active preservation blocks automatic tail takeover, while retained preservation
allows the tail controller to take ownership when its normal distance and intent
rules say that following should resume.

There is no persistent raw `scrollTop` lock or scroll-listener lock. An
unsignaled shrink may adjust an already owned protected range, but it may not
create a collapse reservation from idle geometry alone. Without a semantic
owner, the list accepts the new Virtuoso measurement and rebases its height and
scroll baselines. Subsequent layout changes are handled by the semantic anchor
(when present), the owned reservation transaction, or follow mode.

An element anchor also owns the minimum physical scroll range needed to restore
its offset. After writing `scrollTop`, the coordinator remeasures the actual DOM
offset. If a positive correction remains because the browser clamped at the
bottom, the range host synchronously extends the matching reservation, flushes
layout, and retries in the same frame. The post-write DOM measurement is the
source of truth because integer `scrollHeight` can overstate the browser's
subpixel scroll limit.

Physical-bottom synchronization must yield whenever the coordinator owns an
element anchor. It also yields while streaming `following-tail` owns the
viewport, because the single tail loop is the writer for content-growth motion.
Outside tail follow, physical-bottom synchronization is limited to a real
viewport `clientHeight` change. A message, round footer, or other content growth
changes `scrollHeight` only and must remain below the existing viewport instead
of moving the transcript upward to reach the new physical bottom.
A sticky pin intentionally sits at the physical bottom created by its
reservation; treating that geometry as tail-follow causes every content growth
measurement to push the pinned header upward before the coordinator can restore
it. Pinned, anchored, and non-streaming paths keep the normal physical-bottom
synchronization behavior.

Sticky pin floors are not reduced from a transient target rect. Positive
effective content growth first enters a short settlement ledger (currently
300 ms) instead of immediately removing physical bottom range. An unsignaled
negative height correction cancels matching unsettled growth; a known collapse
does not. Growth that reaches the complete remaining pin floor settles
immediately because the sticky viewport has reached its tail-follow handoff
boundary; if a collapse transaction is still active, that settlement resumes
as soon as the transaction finishes. Sub-threshold growth still waits for the
quiet window. Stable growth consumes the pin floor in one synchronous Footer
update. Live pin reconciliation may increase a floor immediately, but cannot
shrink it while Virtuoso item measurements are still moving. Stream end performs
one final pin measurement when the target is available, transfers all remaining
pin range into protected collapse space, and releases `pinned-item` ownership in
the same transaction. A temporarily virtualized target must not block this
release: the existing physical range is retained until a later explicit drain.
Pending pin retries and growth settlement are canceled at that boundary.

When a sticky target is temporarily virtualized, its provisional range must be
computed from `scrollHeight - currentPinPx`. Reusing physical `scrollHeight`
directly feeds the synthetic footer back into the next retry and grows the range
on every frame. Provisional pins remain at `floorPx: 0`; if the request expires
without capturing an element anchor, that range is removed atomically.

The pin-owned portion of the footer is capped at one viewport. A rendered
target can never require more than `clientHeight` of extra range to align its
top inside the viewport, and one viewport is also sufficient to materialize a
virtualized target. This cap applies to provisional and established pin ranges.
It does not apply to collapse compensation or the total footer: a large card or
several cumulative collapses can legitimately require more than one viewport to
preserve the current semantic anchor.

Pending pin retries carry a synchronous generation plus the owning session and
turn. Canceling or replacing a request increments the generation before React
state is updated, so already-queued animation frames cannot restore a canceled
reservation. User navigation drops a provisional sticky range instead of
transferring it into protected collapse. Established pins keep the existing
protected-range handoff.

Arbitrary-turn navigation is a materialize-then-align transaction. Starting a
new request exits tail follow, but it does not remove the previous established
pin reservation before the target DOM exists. That reservation remains only as
physical scroll range; the active request prevents the old sticky target from
reconciling it. Once the requested user message is rendered, the shared pin
resolver applies the request's alignment policy. Exact requests replace the old
reservation, align the message to the 57px viewport offset, and start bounded
transient stabilization. Turn-rail requests use best-effort alignment: they
still align exactly when the natural range is sufficient, but when the target
cannot reach the 57px offset without synthetic tail space, they remove the
transient pin reservation, clamp to the natural maximum, and release
`pinned-item` ownership immediately. The natural boundary is an expected
content limit, not a pending transaction, so it must not retry until TTL expiry.
`sticky-latest` always uses the exact policy because streaming follow-output
depends on its protected pin range. An expired request releases semantic
ownership while preserving the current physical range, so failure cannot
silently clamp the pane to the bottom.

`rangeChanged` is a target-materialization signal, not a source of turn
identity. It retries the active generation against real DOM geometry. RAF
retries remain as a bounded fallback for browsers that coalesce range updates.
Transient navigation remains pending until the requested turn stays aligned for
two consecutive geometry samples. During that bounded transaction, Virtuoso's
materialization range expands to two viewport heights in both directions so
height-estimate reconciliation cannot immediately evict the target. If the
pinned DOM element still disconnects, the coordinator drops the stale element
anchor but retains logical `pinned-item` ownership while the active generation
rematerializes it. User intent, replacement, expiry, and explicit handoff still
release that ownership.
Virtuoso mounts on the first initial-history commit. A target prepared before
its ref is available becomes `initialTopMostItemIndex`; targets selected after
mount enter the normal immediate materialize-then-align transaction. The
left-side
`FlowChatTurnRail` is mounted outside the scroller and delegates navigation to
the same container-owned turn-pin request, so it does not need to rebind across
renderer handoffs or write the FlowChat viewport directly.

Mounting an already-streaming session is not a new-turn event. Session entry
resumes tail follow directly, while sticky pinning remains reserved for a new
turn that appears in the currently mounted session.

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

Runtime status is transient session UI state, not a `FlowItem`. The always-mounted
`RuntimeStatusSlot` occupies the first 24px of the existing Footer spacer and
switches only `visibility`; showing, hiding, and clearing it never change list
height or enter collapse reconciliation. Subagent projections use the same
fixed-height slot inside their local scroll surface.

If the list waits until `ResizeObserver` sees the shrink, the browser may already have clamped `scrollTop`.

### Completion-preview grace period

`useToolCardCompletionGracePeriod.ts` provides the bounded tail-preview window
used by `ExecProcessToolCardView`, `TerminalToolCard`, and
`FileOperationToolCard`. Its default is
`TOOL_CARD_COMPLETION_PREVIEW_GRACE_MS = 800`.

The timer starts only when a card that was expanded during execution is still
the last rendered item and has not been manually toggled. A newer item, user
interaction, unmount, or loss of tail ownership cancels the pending preview.
For ExecProcess/Terminal cards this covers terminal completion, cancellation,
errors, and rejections. For successful Write/Edit cards, the timer starts after
the typewriter reveal finishes so the completed content is not truncated. The
timer does not change `isLastItem`; an empty next round can still leave the
previous card as the rendered tail, but the grace period bounds that wait. The
timer expiry calls the existing height-contract collapse path, so footer
pre-compensation and semantic-anchor handling remain the same as for a
successor-driven collapse.

This is deliberately separate from the VirtualMessageList collapse-intent TTL
and settlement timers: the former controls when a card may compact, while the
latter protects the viewport while its height changes.

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

## C. Initial-History Snapshot Handoff

Virtuoso is the only initial-history scroller and mounts on the first commit.
For sessions that still need the initial history render budget, a bounded recent
projection is rendered above it as a non-interactive snapshot. The snapshot:

- has no scroll container, spacers, pagination handlers, or viewport writer
- uses `pointer-events: none` and cannot consume wheel, touch, keyboard, or
  scrollbar intent
- keeps the previous pixels visible while Virtuoso measures its initial range
- releases immediately when the user starts scrolling so the real Virtuoso
  motion is never hidden behind a frozen frame
- retargets its release condition when Turn navigation begins during handoff
- disappears only after the requested Turn has visible text, the session
  changes, or the bounded handoff timeout expires

All Turn navigation, search materialization, boundary pagination, bottom state,
and follow-output transitions run through the mounted Virtuoso instance even
while the snapshot is visible. A catalog-backed partial session still keeps
only its restored tail as the default data presentation; this rendering change
does not imply full-history hydration.

## D. Arbitrary Turn Navigation Through Virtuoso

The left-side turn rail delegates to the container-owned top-aligned pin
transaction:

1. record generation, session, target turn, behavior, and pin mode
2. exit tail follow without removing established physical range
3. if the target is absent, issue an immediate `scrollToIndex(..., align:
   'start')`
4. retry from `rangeChanged` and bounded RAF work until the target user message
   exists
5. replace the prior pin reservation with the target's measured reservation
6. align the target to the shared 57px header offset and stabilize delayed
   Virtuoso measurements
7. cancel stale work on a newer request, user intent, session switch, jump to
   latest, or timeout

Every turn-rail marker, including the canonical latest Turn, uses this same
immediate transient top-pin transaction. Selecting the latest marker means
"show this Turn header"; it does not restore the tail presentation or resume
follow-output. Only the explicit jump-to-latest action restores the canonical
tail presentation and re-enters live-tail following. This separation keeps
turn navigation consistent and treats every rail selection as user reading
intent, including while the latest Turn is streaming.

Do not clear the previous pin/footer range in step 2. The target may be outside
the current Virtuoso range, and removing the footer first lets the browser clamp
the old position to the physical bottom before materialization succeeds.

The turn rail is an independent overlay surface. Its height is bounded to 60%
of the FlowChat content area; overflow scrolls only the rail, and keeping the
current marker visible may update only the rail list's `scrollTop`. Rail wheel,
keyboard, hover, and tooltip behavior must never become another writer for the
outer FlowChat viewport.

The existing visible-turn DOM measurement also collects every distinct turn
whose rendered items intersect the readable viewport. The first intersecting
turn remains the semantic current turn, while every intersecting turn marker
uses the same rail emphasis. Publish a new ordered `visibleTurnIds` snapshot
only when membership or order changes so ordinary scroll frames do not cause
redundant rail renders.

### Catalog-backed history loading

Catalog, loaded Turn cache, and active presentation are separate layers. Keep
these ownership rules intact:

- `Session.dialogTurns` remains the live restored tail unless an explicit
  full-history consumer calls `ensureSessionFullHistory`.
- Data residency, viewport intent, and follow-output ownership are independent.
  A cached history presentation may remain resident after the viewport returns
  to the live tail, but it must not keep the UI in history-reading mode,
  suppress live-tail anchoring, or imply that follow-output is active.
- For a small session whose cached presentation is contiguous from ordinal zero
  through the current total (`[0, totalTurnCount)`) and stays within the
  continuous projection budgets (24 Turns and 200 virtual items), explicit
  jump-to-latest changes only the viewport intent and follow-output ownership.
  The rendered projection and its stable virtual-item keys remain unchanged;
  `historyWindow` is disabled so boundary loading cannot start while following
  the tail. Canonical overlapping Turns are still overlaid by stable id, and a
  newly appended canonical Turn extends the projection at the end.
- Incomplete, discontinuous, or over-budget presentations retain the fallback
  behavior: explicit jump-to-latest clears the Store's `activeRange`, restores
  the canonical tail data source, and keeps the most recent component
  presentation only as a reactivation hint. The Store LRU remains authoritative:
  reactivation must find the complete range in `loadedRanges`, touch it as MRU,
  and otherwise fall back to the ordinary window-load transaction.
- Turn-rail navigation and sequential boundary loading use
  `load_session_turn_window`; neither path writes the FlowChat scroller.
- Upward user intent at the restored-tail boundary loads the adjacent ordinal
  window without holding viewport ownership. Presentation activation then waits
  for a bounded 320 ms quiet window after the latest wheel, touch, keyboard, or
  scrollbar intent. New input resets that wait; session changes and newer
  presentation-owner generations cancel it. Only after the quiet window is
  acquired does the list capture the current element anchor and change to one
  contiguous history-window presentation. This keeps a multi-thousand-pixel
  prepend commit out of an active wheel gesture while still allowing the data
  request itself to prefetch in parallel. Never expose a later cached range
  across an unloaded gap.
- Derive the restored-tail boundary from the canonical `Session.dialogTurns`
  ordinal interval, never from the start of a merged `loadedRanges` entry.
  Cache residency may extend to the first Turn while the canonical tail still
  renders only recent Turns. Reaching ordinal zero is an exhausted boundary,
  not a not-ready or failed load.
- Appending below the current presentation does not require compensation.
  Prepending or trimming above it must retain the existing element-anchor
  transaction until the same user message returns to its captured viewport
  offset.
- A rejected or failed adjacent-window request must release only the element
  anchor lease created during its commit preparation, if any. A stale
  completion must never release a newer navigation or layout-preservation
  transaction.
- The non-tail loaded Turn cache uses a 48-Turn soft budget and a 64-Turn hard
  budget. Crossing the hard budget evicts least-recently-used ordinals back
  toward the soft budget. The live tail, active presentation, pending target,
  and in-flight request intervals are protected; merged cached ranges may be
  sliced, but the active presentation is never trimmed by cache eviction.
- Passive live-tail updates outside the presented history range remain hidden
  while the user reads history. When the history range overlaps canonical live
  Turns, stable Turn ids select the canonical objects instead of cached
  snapshots so streaming or recently completed content stays current without
  changing the viewport intent. An explicit `send-message` Turn-pin request
  first restores the tail presentation, then lets the existing sticky-latest
  pin materialize the newly submitted Turn.
- Cross-feature focus requests identify a Turn by stable `turnId` whenever one
  is available, with `turnIndex` reserved for the absolute one-based visible
  ordinal. They delegate to the same catalog/window materialization transaction
  as the Turn rail. Never pass that absolute ordinal to `scrollToTurn` on a
  partial tail or bounded history presentation; that method only understands
  the currently rendered local list.
- Search, edit, rollback, and compatibility fallback are explicit full-history
  consumers. Their shared ensure operation deduplicates an existing request and
  applies the completed projection only after the caller asks for it.
- A Host without `turnCatalog`, or without `load_session_turn_window`, retains
  the legacy full-restore fallback. This compatibility path must not cause a
  catalog-capable Host to resume unconditional background hydration.

## Why Transition Tracking Exists

User-initiated expand/collapse still uses animated layout properties such as:

- `grid-template-rows`
- `height`
- `max-height`

Automatic and manual collapses both animate through the shared motion contract
unless animation is explicitly disabled.

During those transitions, the DOM may report intermediate sizes for multiple frames.

The collapse intent carries a hard TTL (`expiresAtMs`, currently 1000 ms), but
that TTL only bounds collapse measurement and reservation settlement; it does
not expire the semantic element anchor. Automatic collapses are
finalized after `FLOWCHAT_COLLAPSE_DURATION_MS` plus a short settle-frame window;
manual or otherwise unsignaled intents use the TTL timer. The scroll handler keeps only a throttled-background
timer fallback for browsers that delay timers. While the intent is alive, the
grow branch of `measureHeightChange` protects the collapse reservation, but it
may still consume measured content growth from the sticky pin reservation.
Intent settlement follows the current semantic viewport owner. A sticky pinned
turn always reconciles provisional collapse space back into a freshly measured
pin reservation, even when the active transaction established a non-zero
collapse floor. That floor protects the pin only while layout is moving; it
must not cause the full-card estimate to survive into the next collapse. If the
pinned target is temporarily unavailable, settlement retries without dropping
the current range. A following tail instead enters retained-provisional quiet
settlement, while a collapsing header that owns `preserving-element` reduces
the footer atomically to the minimum range needed by its captured `scrollTop`.
A detached protected viewport uses the same geometric settlement against its
current `scrollTop`, without retaining provisional pixels above that range.
These owner-specific transactions prevent both clear-and-reacquire frames and
cumulative provisional whitespace. Any deferred follow is then replayed.

## E. Follow-Output Mode (continuous tail)

When the viewport is in follow-output mode and the latest turn is still
streaming, the user's intent is "keep the tail visible". After the viewport
coordinator has entered `following-tail`, one RAF loop eases `scrollTop` toward
the effective bottom. Follow events only wake this loop; they do not launch
additional scroll writers. The target subtracts the current Footer
reservation, and large gaps snap directly to the target instead of leaving the
user visibly behind the output.

The loop is dormant while `pinned-item`, `preserving-element`, or a collapse
transaction owns the viewport. It never clears reservations or calls
`followTail()` from inside the animation frame. This keeps the semantic
handoff and Virtuoso compensation paths authoritative while allowing small
line-height growth to move over several frames. Explicit "jump to latest"
navigation keeps its native smooth scroll; the RAF loop waits for that motion
to settle before writing.

Collapses interact with follow mode in three mutually exclusive ways:

1. **Known collapse while follow + streaming is active:** the intent applies
   synchronous Footer pre-compensation before the card shrinks. The active
   intent allows shrink reconciliation even though tail follow is running.
    When the CSS window ends, the transaction becomes a retained-provisional
   collapse anchor instead of shrinking the Footer from a signed net-height
   estimate. Virtuoso
   can publish the matching item measurement after the CSS transition and after
   stream end; reducing synthetic range before that measurement clamps the
   viewport by exactly the removed pixels.
   The retained transaction records the latest safe follow position. After a
   negative-layout quiet window, it replaces both the provisional `px` and stale
   `floorPx` with the minimum geometrically required Footer range. The final
   release uses one timer plus a geometry generation: any effective height
   change invalidates that timer's snapshot, and the timer performs one more
   quiet check instead of every token allocating new timer work. If a later
   measurement clamps below that position, the scroll handler synchronously
   extends the range and restores it before paint. Real content growth and
   downward follow movement consume the range one-for-one. User intent, a new
   pin, session reset, or a final quiet grace releases the retained anchor.
   Stream end restarts the same settlement path;
   it does not preserve the provisional full-card estimate indefinitely.
2. **Unsignaled shrink while follow + streaming is active:** a strict physical
   clamp signature (the previous and current geometries are both at their
   physical bottoms, the range shrink matches the negative `scrollTop` delta,
   viewport height is stable, and there is no user intent) starts a
   `late-shrink` viewport transaction. The scroll handler extends the Footer and
   restores the pre-clamp position synchronously, covering virtualizer size
   commits that arrive after the originating collapse transaction was released.
   Other unsignaled shrinks remain owned by the tail loop; it follows only
   downward toward the new effective bottom on the next frame.
   A negative `scrollBy` issued by Virtuoso after a virtualized height
   reduction is also suppressed when the previous geometry was already at the
   physical bottom. That compensation would move the viewport away from the
   tail; the next follow frame owns the single tail correction instead.
3. **Not following (user reading older content):** the intent +
   pre-compensation + semantic-anchor path applies as described above, and
   `shouldSuspendAutoFollow` keeps event-driven follow scheduling
   deferred until the intent's TTL lapses.

The loop is cancelled as soon as follow exits (user upward scroll,
session change, or streaming end). Explicit "jump to latest" navigation pauses
the writer while its native smooth scroll completes, then resumes the same
single tail loop.

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

- `useToolCardHeightContract.ts` (used by most tool cards, including
  `ExecProcessToolCardView`, `FileOperationToolCard`, and `TerminalToolCard`)
- `ModelThinkingDisplay.tsx`
- `ExploreGroupRenderer.tsx`

Most tool cards now emit these events through `useToolCardHeightContract`.
The helper measures the visible `cardRootRef` and retains recent visible
measurements so state-driven collapses still report the pre-collapse height.
Never substitute an inner scroll container's `scrollHeight`; hidden overflow is
not layout height removed from the FlowChat list.

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
  atomically transfer the pin reservation to a protected collapse range in the
  same operation; an idle coordinator must never retain a live pin reservation.
- Stream end or cancellation must perform that same protected-range transfer
  before releasing `pinned-item`, even when the pinned DOM target is unavailable.
- Scroll-handler anchor corrections must be coalesced through the coordinator's
  animation-frame restore queue; they must not write `scrollTop` synchronously.
- A retained collapse transaction may synchronously restore only a non-user
  downward clamp. It is transaction-scoped, advances with downward tail follow,
  and must release on user intent; it is not a general `scrollTop` lock.
- Following-tail collapse finalization must never reduce Footer range directly
  from a signed net-height estimate. It may reduce the retained estimate only
  after the negative-layout quiet window, using current DOM geometry while the
  anchor remains protected through the final grace.
- Unsignaled shrink reconciliation must not reduce a protected collapse floor;
  only measured growth, downward navigation, bottom arrival, or an explicit
  reservation reset may consume it.
- Unsignaled shrink reconciliation must not create a collapse reservation when
  no collapse transaction owns the range. Session-open and history-projection
  handoffs rebase their measurements instead of treating estimate convergence
  as content collapse.
- Pre-collapse intent must capture the anchor before the component shrinks.
- Compensation must not be consumed too early during active layout transitions.
- Session changes and empty-list resets must clear compensation and anchor state.

## Common Ways To Break This

- Adding a mount-triggered CSS animation to a virtualized list item, or animating
  an automatic collapse without keeping collapse-intent protection alive for the
  full `FLOWCHAT_COLLAPSE_DURATION_MS` window (see Rule Zero).
- Feeding `Date.now()` back into `sessionToVirtualItems` /
  `buildModelRoundItemGroups`, or splitting one `ModelRound` into several
  `model-round` virtual items — both swap stable Virtuoso keys for new ones and
  remount visible content.
- Replacing `applyFooterCompensationNow()` with state-only rendering.
- Measuring raw `scrollHeight` deltas without subtracting existing compensation.
- Removing `flowchat:tool-card-collapse-intent` from a helper-backed collapsible component.
- Finalizing an active collapse intent when a new one arrives mid-burst instead of
  coalescing TTL / provisional shrink (drops footer protection for a frame).
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

### Opt-in viewport diagnostics

Enable `app.logging.flow_chat_diagnostics` from the logging settings only while
reproducing a viewport stability issue. The frontend records bounded JSONL
batches to `flowchat.log` in the current session log directory. When disabled,
probe payloads are not evaluated and no timer, IPC request, or file is created.

The diagnostic schema groups events by hypothesis:

- `A`: user scroll intent, pin release, reservation transfer, and tail handoff
- `B`: semantic anchor capture, correction, release, or unexpected reacquisition
- `C`: content measurement, Footer compensation, and physical range changes
- `D`: Virtuoso scroll compensation and tail-follow ownership
- `E`: streaming tool-card collapse intent and anchor preservation

Do not add message content, tool arguments, file contents, or other sensitive
payloads to this channel. Keep all data producers lazy and guard hot-path probes
with `flowChatDiagnostics.isEnabled()` before allocating probe objects.

Use this checklist:

1. Verify a just-completed ExecCommand/Write tail keeps its preview during the
   short grace period, then compacts if no follow-on item arrives.
2. Verify manual collapse of a completed `Write` / `Edit` tool card.
3. Verify a newer item still causes immediate automatic compaction before the
   grace period expires.
4. Verify repeated expand/collapse near the bottom.
5. Verify thinking / explore / other collapsible sections still schedule measurements correctly.
6. Verify there is no visible "drop then snap back" flash.
7. Verify the final header position remains stable after collapse.

## Related Files

- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.tsx`
- `src/web-ui/src/flow_chat/components/modern/FlowChatViewportCoordinator.ts`
- `src/web-ui/src/flow_chat/components/modern/VirtualMessageList.scss`
- `src/web-ui/src/flow_chat/tool-cards/useToolCardHeightContract.ts`
- `src/web-ui/src/flow_chat/tool-cards/useToolCardCompletionGracePeriod.ts`
- `src/web-ui/src/flow_chat/tool-cards/ExecProcessToolCardView.tsx`
- `src/web-ui/src/flow_chat/tool-cards/FileOperationToolCard.tsx`
- `src/web-ui/src/flow_chat/tool-cards/ModelThinkingDisplay.tsx`
- `src/web-ui/src/flow_chat/tool-cards/TerminalToolCard.tsx`
- `src/web-ui/src/flow_chat/components/modern/ExploreGroupRenderer.tsx`
