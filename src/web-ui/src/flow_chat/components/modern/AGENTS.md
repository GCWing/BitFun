# FlowChat Scroll Instructions

This file applies to the modern FlowChat viewport implementation under this
directory. It is the rules. The reasoning, the measurements, and the failures
each rule was written against live in the documents below — read the one that
covers what you are about to change.

Also follow the repository and Web UI instructions in the parent guides.

## Required Reading

| Changing | Read |
|---|---|
| the tail spacer, the follow target, pinning, holding, the snap back, resizing, the footer, the reveal | `FLOWCHAT_SCROLL_STABILITY.md` |
| history paging, the prepend, the viewport anchor, history presentation | `FLOWCHAT_HISTORY_PAGING.md` |
| anything that writes `scrollTop`, one-shot navigation, the diagnostic trail | `FLOWCHAT_VIEWPORT_REGISTER.md` |
| the virtualizer, item measurement, item keys, anything a row renders | `FLOWCHAT_VIRTUALIZATION.md` |

`FLOWCHAT_SCROLL_STABILITY.md` also carries *Known Gaps* for all four — check it
before reporting a defect as new.

## Reservation and Follow

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
- Footer height represents only the current input-stack layout and real footer
  content such as history state and `RuntimeStatusSlot`. The tail spacer is a
  separate sibling and must not be folded into it.
- "At bottom" is measured against the end of real content, which sits above the
  tail spacer, so no alignment to the last item can express it.

## The Viewport Register

- Every deliberate viewport write goes through `useFlowChatViewportOwner`, named
  with the owner it belongs to, and holds that ownership for as long as it is
  moving — an animation included. Never assign `scrollTop` or call `scrollTo`
  on the FlowChat scroller directly.
- Adding an owner means adding it to `FLOWCHAT_VIEWPORT_OWNERS` in priority
  order and to its test, not adding a condition to anyone else's predicate.
- The virtualizer's own writes are registered through its `scrollToFn` option
  and attributed to whoever asked for the aim. Do not bypass it.
- A writer that declines to move the viewport says so through
  `flowChatViewportDiagnostics.ts`. The register records the writes; a write
  that never happened is invisible everywhere else, and "nothing happened" is
  the more common report. Anything reachable every frame goes through
  `traceViewportRepeating`, keyed by what distinguishes one run from another.
- There are no viewport writes outside the register. Adding one means wrapping
  it in `traceViewportPlacement` and having a reason it cannot be a register
  write; both writers that used to be outside are gone.
- One user action gets one painted placement. A focus request that first
  navigates to a Turn and then aims at an item inside it does both in the same
  task — two placements a few frames apart are two movements the reader sees,
  and the sampled drift lands on the first one, not the second.
- Centring a flow item is `VirtualMessageListRef.focusFlowItem`, not
  `element.scrollIntoView`. The virtualizer aligns *items*, so it cannot express
  "this tool call inside this Turn"; the computed offset is the contract's
  carve-out for a target that is not an item, and it still goes through the
  register.
- One-shot Turn/search/history navigation remains inside `VirtualMessageList`.

## History Paging and the Anchor

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
- Boundaries are evaluated on scroll **intent**, not only on `scroll` events. A
  reader already at the top produces no scroll event, so the one signal that
  they want more history is the gesture itself. Evaluate after ownership has
  been released, so the ask is theirs rather than our placement's.
- Ownership is read through `isFollowingOutputNow()`, never a render-time mirror
  of `isFollowingOutput`. A gesture releases it synchronously and asks in the
  same handler; the mirror still reports the ownership that gesture just ended.
- `exhausted` is latched per *window*, not per session. It answers "nothing
  before this start ordinal", so the latch clears whenever the window's ordinals
  change — not only when a page is `applied`.
- A history prepend must be compensated for in `VirtualMessageList`, by the
  height of the items that arrived above. Keying measurements on item identity
  covers the measurements; it does not move the scroll offset. The compensation
  cannot finish the job on its own — most of the movement is the arrived items
  measuring over the frames after it, and only the anchor's relationship
  survives that.
- That compensation and the viewport anchor are displacements, not positions:
  `viewportOwner.shift`, never a write with an owner. A gesture must not be able
  to refuse either — paging up happens only while the reader is scrolling up, so
  anything a gesture can refuse here is refused every time.
- A scroll re-anchors the reader, except while the anchored Turn is missing from
  the rendered window. A correction is owed there and cannot be measured yet, so
  the anchor is carried through the scroll — credited with the reader's own
  travel — never replaced by whatever else happens to be rendered.
- The anchor's settle window is refreshed by what a frame *observed*, never by a
  counter left over from an earlier one. A frame that stood down for another
  owner looked at nothing, so it refreshes nothing — otherwise the loop runs at
  frame rate for as long as that owner rests on the viewport, which at the tail
  is indefinitely.
- The viewport anchor lives in `flowChatViewportAnchor.ts` and
  `useFlowChatViewportAnchor.ts` and must stay independent of the virtualizer:
  it may read the scroller and the Turns rendered inside it, and nothing else.
  Virtualizer-specific compensation stays in `VirtualMessageList`.
- "A new Turn" is `activeSession.dialogTurns.at(-1)`, never the end of the
  projection. Do not qualify that identity by whether the Turn is on screen —
  that belongs to the response, which defers until the Turn can be aligned.
- Detecting one means the ledger **grew**, not that the identity changed. A
  rollback truncates `dialogTurns` and moves that identity backwards onto a Turn
  that was always there; read as an arrival it pins the survivor to the top.
- An action that rewrites `dialogTurns` and wants the viewport moved announces
  it — `FLOWCHAT_MESSAGE_SUBMITTED_EVENT` for giving up a navigated history
  window, `FLOWCHAT_TURNS_ROLLED_BACK_EVENT` for settling on a new tail. The
  ledger cannot tell a Turn the reader sent from one that arrived from
  elsewhere, nor a rollback from a window re-cut, and there are two dozen
  writers of that array. Do not infer either from a count.

## Virtualization and Rendering

- `useFlowChatVirtualizer.ts` is the only module that may import a virtualization
  library. It speaks in scroller offsets and item positions; anything that would
  make a caller aware of which library is underneath belongs inside it.
- Prefer `scrollItemIntoView` over computing an offset. The virtualizer re-aims
  while items below the target measure, and an offset computed once cannot.
  Compute one only when the target is not an item.
- Anything reading an item position in the commit that changed the items calls
  `measureRenderedItems()` first. The library skips its inline measurement while
  the reader is scrolling, which is exactly when history arrives, so the cache
  holds reserved estimates until the ResizeObserver delivers a frame later.
- The virtualizer must not adjust the scroll for its own re-measurements. It
  replays a delta against a scroll position it learns about a frame late, and
  every continuous writer here assigns `scrollTop` directly.
- The virtualizer never follows output.
- No mount or enter animation inside `.virtual-item-wrapper`, no mount-triggered
  motion that changes transcript geometry, and nothing keyed on a state change a
  scroll can replay. A row mounts when it enters the rendered window, not when
  its content arrives, so the animation runs again on every page up and every
  scroll back. Cancel it at the wrapper rather than in the component — this has
  been patched locally four times and recurred each time.
- Tool cards reflow naturally and dispatch only `tool-card-toggle` after an
  expanded-state change so the virtualizer can remeasure.
- Stable virtual-item keys and projection identity must be preserved. Do not
  split one `ModelRound` into multiple virtual items or reclassify projection
  from a timer.

## Verification

`FLOWCHAT_VERIFICATION.md` is the single list — the automated checks, mapped to
the contract each one holds, and the manual ones grouped by scenario. Do not
keep a second copy here.

Do not perform UI interaction verification. Report the manual checks as pending
unless the user confirms them.

## Keeping These Documents True

Update the document that owns the area you changed, and this file only if a rule
changed. A rule belongs here when a reviewer could catch its violation by
reading a diff; everything else — the reasoning, the numbers, the failure it was
written against — belongs in the document.
