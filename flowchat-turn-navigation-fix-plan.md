# FlowChat Turn Navigation Fix Plan

## Confirmed Root Cause

The failure is not caused by an explicit streaming-time navigation block.

Before a new turn starts, a restored historical session may use the static
initial-history list. Arbitrary turn navigation can then materialize a target
by changing `staticAnchorWindowTurnId` and rendering a bounded window around
the target.

Starting a new turn completes backend context restoration. The session changes
from `contextRestoreState: 'pending'` to `ready`, which disables
`useStaticInitialHistoryList` and replaces the static scroller with Virtuoso.

After this transition, a far arbitrary turn uses Virtuoso's
`scrollToIndex` fallback. In the reproduced failure:

- Header navigation returned `accepted: true`.
- Both during streaming and after streaming, pin status stayed `pending`.
- `isFollowingOutput` was `false` in both cases.
- The target turn remained unrendered.
- The viewport stayed at the physical bottom.

The visible drop to the bottom is a secondary effect. The navigation path
clears the new-turn sticky pin/footer reservation before the target has been
materialized. When materialization fails, the browser clamps the scroll
position to the physical bottom and there is no successful target alignment to
replace it.

There is also a separate scroller handoff bug: `ScrollAnchor` listens through a
stable ref object and does not rebind when `.current` changes from the static
scroller to the Virtuoso scroller.

## Recommended Design

### 1. Use one turn-navigation transaction

Header navigation, right-side anchor navigation, and search navigation should
share one transaction with:

- `targetTurnId`
- `targetVirtualIndex`
- `sessionId`
- `phase: materializing | aligning | settled | canceled`
- a generation for cancellation and stale-request rejection

The callers should request a transaction; the viewport implementation should
own the lifecycle.

### 2. Carry the pending target across the static-to-Virtuoso handoff

When Virtuoso mounts while a turn navigation is pending, use the pending target
as `initialTopMostItemIndex` with `align: 'start'` instead of defaulting to the
latest turn. After the target DOM exists, perform the exact pinned-top
alignment using the shared `57px` header offset.

This avoids depending on a post-mount reverse jump from the latest tail to a
far, unmeasured target.

### 3. Preserve the physical range until target alignment succeeds

The navigation sequence should be:

1. Capture the target and start the navigation transaction.
2. Exit follow mode.
3. Preserve the current physical range and existing footer reservation while
   the target is being materialized.
4. Materialize the target.
5. Align the target user message to the pinned top offset.
6. Confirm stable geometry for a few frames.
7. Only then release or transfer the pin reservation.

If the request is canceled, restore the appropriate physical range and clear
the transaction without silently moving to the bottom.

### 4. Make `rangeChanged` part of materialization completion

Do not rely only on blind per-frame retries. On `rangeChanged` or virtual-item
updates:

- check whether the target DOM node exists;
- retry materialization only while it is absent;
- enter `aligning` when it appears;
- settle after stable top alignment.

Keep a bounded timeout as a final guard, but make expiry a controlled cancel
and recovery path rather than a silent fall to the physical bottom.

### 5. Rebind `ScrollAnchor` to the actual scroller element

Pass the current scroller element as state instead of relying only on a stable
ref object:

```tsx
<ScrollAnchor scrollerElement={scrollerElement} />
```

The listener effect should depend on `scrollerElement`, so static-to-Virtuoso
replacement removes the old listener and attaches to the new node.

## Implementation Order

1. Add the shared navigation transaction and preserve-range semantics.
2. Feed the pending target into Virtuoso's initial positioning during the
   renderer handoff.
3. Resolve the target through `rangeChanged` and exact DOM alignment.
4. Fix `ScrollAnchor` scroller rebinding.
5. Remove temporary debug instrumentation after the user confirms the issue
   is fixed.

## Regression Coverage

Add focused tests for:

- arbitrary navigation in the static history window;
- navigation after a new turn switches static rendering to Virtuoso;
- navigation during streaming;
- navigation after streaming ends;
- an unrendered far target becoming rendered and then aligning at the top;
- preservation of the sticky pin/footer range until alignment succeeds;
- cancellation via jump-to-latest, user scroll, session switch, or a newer
  navigation request;
- `ScrollAnchor` rebinding after the scroller DOM node changes.

## Implementation Status

Implemented in the modern FlowChat viewport:

- pending navigation generations are invalidated without removing the active
  footer range; stale sticky reconciliation is suspended until the new target
  settles
- Virtuoso receives pending static/Virtuoso targets through
  `initialTopMostItemIndex`, and `rangeChanged` participates in materialization
- timeout and replacement paths release semantic ownership without silently
  falling to the physical bottom
- `ScrollAnchor` rebinds to the concrete scroller element
- regression coverage covers range preservation, static-to-Virtuoso handoff,
  and scroller rebinding

Automated verification passed:

- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run lint`
- 51 focused FlowChat tests passed

Interactive reproduction remains the final acceptance step. Temporary
interactive-debug instrumentation is intentionally retained until that
reproduction is confirmed fixed.
