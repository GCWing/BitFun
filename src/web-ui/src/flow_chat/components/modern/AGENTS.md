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
  `flowChatHistoryBoundary.ts` and reads only a visible item range. Deciding
  whether the ask is honoured stays in the container, which declines while
  follow-output owns the viewport and until the visible range has left that
  boundary since the last page.
- A *visible* item range is `getVisibleItemRange`, never the rendered rows. The
  rendered window carries overscan and reports both ends present for any
  transcript short enough to render whole.
- A history prepend must be compensated for in `VirtualMessageList`, by the
  height of the items that arrived above. Keying measurements on item identity
  covers the measurements; it does not move the scroll offset.
- The virtualizer must not adjust the scroll for its own re-measurements. It
  replays a delta against a scroll position it learns about a frame late, and
  every continuous writer here assigns `scrollTop` directly.
- Anything that moves the viewport deliberately must be registered in
  `isViewportOwnedElsewhere` for as long as it is moving, including an animation
  still in flight. The anchor judges by geometry and cannot tell our own
  movement from a displacement to undo.
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
- `flowChatHistoryBoundary.test.ts`
- `useFlowChatVirtualizer.test.ts`
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
