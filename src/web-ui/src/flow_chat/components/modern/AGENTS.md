# FlowChat Scroll Instructions

This file applies to the modern FlowChat viewport implementation under this
directory.

## Required Reading

Before changing rendering, virtualization, scrolling, tool-card collapse,
footer layout, runtime-status slots, or reveal behavior, read:

- `FLOWCHAT_SCROLL_STABILITY.md`

Also follow the repository and Web UI instructions in the parent guides.

## Current Contract

- FlowChat reserves a resident tail spacer of about one viewport, sized from
  `scroller.clientHeight` and nothing else.
- Static reservation is allowed; reactive compensation is not. Do not derive any
  reserved height from a measured content height, a collapse delta, an animation
  duration, or a streaming rate.
- Do not add sticky Turn modes, pre-collapse compensation, or persistent
  element-anchor guards.
- The follow target lives in `flowChatTailFollow.ts` as pure functions over
  geometry. Keep it free of timers and mutation observers.
- `scheduleFollowToLatest` must not force the content end — the hold rule is
  what keeps a collapse from moving the viewport.
- `useFlowChatFollowOutput` is the only continuous outer viewport writer.
- The viewport anchor lives in `flowChatViewportAnchor.ts` and
  `useFlowChatViewportAnchor.ts` and must stay independent of the virtualizer:
  it may read the scroller and the Turns rendered inside it, and nothing else.
  Virtualizer-specific compensation stays in `VirtualMessageList`.
- `useFlowChatVirtualizer.ts` is the only module that may import a virtualization
  library. It speaks in scroller offsets and item positions; anything that would
  make a caller aware of which library is underneath belongs inside it.
- Prefer `scrollItemIntoView` over computing an offset. The virtualizer re-aims
  while items below the target measure, and an offset computed once cannot.
  Compute one only when the target is not an item.
- Deciding *that* a history boundary is worth asking about belongs to
  `flowChatHistoryBoundary.ts` and reads only a visible item range and the
  scroll distance to each end. Deciding whether the ask is honoured stays in the
  container, which declines while follow-output owns the viewport and until the
  visible range has left that boundary since the last page.
- The ask goes out a screenful before the boundary, so the junction lands off
  screen. Do not express that lead in items: one item here is anything from a
  38px user message to a 5012px model round.
- The arming latch re-arms from `historyBoundariesReached`, never from the ask.
  Sharing one predicate makes a boundary the reader can never be off, and the
  direction stays disarmed for the rest of the session.
- A *visible* item range is `getVisibleItemRange`, never the rendered rows. The
  rendered window carries overscan and reports both ends present for any
  transcript short enough to render whole.
- A history prepend must be compensated for in `VirtualMessageList`, by the
  height of the items that arrived above. Keying measurements on item identity
  covers the measurements; it does not move the scroll offset.
- Anything reading an item position in the commit that changed the items calls
  `measureRenderedItems()` first. The library skips its inline measurement while
  the reader is scrolling, which is exactly when history arrives, so the cache
  holds reserved estimates until the ResizeObserver delivers a frame later.
- That compensation and the viewport anchor are displacements, not positions:
  `viewportOwner.shift`, never a write with an owner. A gesture must not be able
  to refuse either — paging up happens only while the reader is scrolling up, so
  anything a gesture can refuse here is refused every time.
- The compensation cannot finish the job on its own. It is a pixel delta in the
  commit that prepends, and most of the movement is the arrived items measuring
  over the frames after it; only the anchor's relationship survives that.
- A scroll re-anchors the reader, except while the anchored Turn is missing from
  the rendered window. A correction is owed there and cannot be measured yet, so
  the anchor is carried through the scroll — credited with the reader's own
  travel — never replaced by whatever else happens to be rendered.
- The virtualizer must not adjust the scroll for its own re-measurements. It
  replays a delta against a scroll position it learns about a frame late, and
  every continuous writer here assigns `scrollTop` directly.
- Every deliberate viewport write goes through `useFlowChatViewportOwner`, named
  with the owner it belongs to, and holds that ownership for as long as it is
  moving — an animation included. Never assign `scrollTop` or call `scrollTo`
  on the FlowChat scroller directly.
- Adding an owner means adding it to `FLOWCHAT_VIEWPORT_OWNERS` in priority
  order and to its test, not adding a condition to anyone else's predicate.
- A writer that declines to move the viewport says so through
  `flowChatViewportDiagnostics.ts`. The register records the writes; a write
  that never happened is invisible everywhere else, and "nothing happened" is
  the more common report. Anything reachable every frame goes through
  `traceViewportRepeating`, keyed by what distinguishes one run from another.
- A viewport write that does not go through the register is wrapped in
  `traceViewportPlacement`, which samples what became of it. There are two, both
  outside this directory, and neither may grow into three without evidence.
- The virtualizer's own writes are registered through its `scrollToFn` option
  and attributed to whoever asked for the aim. Do not bypass it.
- "A new Turn" is `activeSession.dialogTurns.at(-1)`, never the end of the
  projection. Do not qualify that identity by whether the Turn is on screen —
  that belongs to the response, which defers until the Turn can be aligned.
- Giving up a navigated history window is the composer's call, announced through
  `FLOWCHAT_MESSAGE_SUBMITTED_EVENT`. The ledger cannot tell a Turn the reader
  sent from one that arrived from elsewhere, and only the first may move them.
- The virtualizer never follows output. "At bottom" is measured against the end
  of real content, which sits above the tail spacer, so no alignment to the last
  item can express it.
- One-shot Turn/search/history navigation remains inside `VirtualMessageList`.
- Tool cards reflow naturally and dispatch only `tool-card-toggle` after an
  expanded-state change so the virtualizer can remeasure.
- Footer height represents only the current input-stack layout and real footer
  content such as history state and `RuntimeStatusSlot`. The tail spacer is a
  separate sibling and must not be folded into it.
- Stable virtual-item keys and projection identity must be preserved.

## Verification

Choose focused tests, then run:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run <focused-test-files>
```

Relevant tests include:

- `flowChatTailFollow.test.ts`
- `flowChatViewportOwnership.test.ts`
- `../../../infrastructure/diagnostics/flowChatViewportDiagnostics.test.ts`
- `flowChatHistoryBoundary.test.ts`
- `useFlowChatVirtualizer.test.ts`
- `useFlowChatVirtualizer.measurement.test.tsx`
- `flowChatViewportAnchor.test.ts`
- `useFlowChatViewportAnchor.test.tsx`
- `useFlowChatFollowOutput.test.tsx`
- `VirtualMessageList.layout.test.ts`
- `VirtualMessageList.session-boundary.test.tsx`
- `ModernFlowChatContainer.history-state.test.tsx`
- `flowChatCollapseMotion.test.ts`

Do not perform UI interaction verification. Report the manual checks described
in `FLOWCHAT_SCROLL_STABILITY.md` as pending unless the user confirms them.

Update `FLOWCHAT_SCROLL_STABILITY.md` whenever viewport ownership, natural
navigation, footer layout, or required verification changes.
