# FlowChat Viewport Verification

The automated checks, and the manual ones agents may not run.

## Automated

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run <the files below that your change touches>
```

Pick by what you changed rather than running the whole column — but this is the
whole column, and it is the only list. Two divergent copies of it used to exist,
each missing what the other had.

| Test | Contract it holds |
|---|---|
| `flowChatTailFollow.test.ts` | the follow target, `pin-turn-top`, `hold-tail` |
| `flowChatCollapseMotion.test.ts` | collapse does not move earlier content |
| `useFlowChatFollowOutput.test.tsx` | the frame loop, snap back, resize realign |
| `../../tool-cards/useToolCardHeightContract.test.tsx` | tool cards reflow rather than compensate |
| `flowChatHistoryBoundary.test.ts` | the screenful lead, and the latch's own predicate |
| `flowChatLiveTailWindow.test.ts` | "does the transcript still reach the newest Turn" |
| `flowChatViewportAnchor.test.ts` | anchor geometry and the DOM contract |
| `useFlowChatViewportAnchor.test.tsx` | capture, restore, carry, the settle window |
| `VirtualMessageList.session-boundary.test.tsx` | prepend compensation and the ask |
| `ModernFlowChatContainer.history-state.test.tsx` | history presentation and the submission event |
| `flowChatViewportOwnership.test.ts` | the priority order, preemption, expiry |
| `../../../infrastructure/diagnostics/flowChatViewportDiagnostics.test.ts` | coalescing, placement sampling, the switch |
| `useFlowChatVirtualizer.test.ts` | the offsets-and-positions boundary |
| `useFlowChatVirtualizer.measurement.test.tsx` | `measureRenderedItems` against a real virtualizer |
| `VirtualMessageList.layout.test.ts` | the item-height estimate and the spacer |

## Manual

**Agents must not perform UI interaction verification.** Report these as pending
unless a human confirms them. They are grouped so that adding a check to one
group does not renumber the others.

### Opening a session

1. Session open lands at the end of the transcript, not inside the spacer.
2. Open a long `isPartial` session and leave it alone. Nothing may page in
   behind the reveal: the transcript opens on its loaded tail and stays there.
   Five pages arriving over 890ms is what this checks for, and the reveal only
   hides the first frame of it.
3. Open a long session and check the scrollbar thumb: its size should be close
   to right on the first painted frame, and it should not jump as items
   measure. This is the per-item estimate doing its job, and it is the single
   most visible symptom if the estimate ever regresses.
4. Session switching and history paging do not restore stale footer height.

### Submitting and pinning

1. A newly submitted Turn opens at the viewport top with room below it.
2. Send a one-line message and let it pin. It must come to rest at the top with
   the same small gap above it as the very first Turn of the session, and the
   pin must hold steady rather than creeping down — a pinned offset past the
   end of the scroll range is clamped, and the follow loop will rewrite it
   every frame.
3. Scroll down into the blank and let go right after submitting a short Turn:
   it returns that Turn to the viewport top, not to the content end.
4. Send a message from the live tail, and again while parked deep in history.
   Both must end with the new Turn at the viewport top; the second also has to
   leave the history window to get there.
5. Send a message, let it pin, then roll it back from its own message actions.
   The transcript must come to rest with the surviving last Turn at the
   *bottom* — not with it pinned to the top, which is what reading the
   truncation as an arrival used to do.
6. Roll a Turn back from further up a transcript, having scrolled to reach it.
   The surviving last Turn must end at the bottom here too — scrolling to reach
   the button hands the viewport to the reader, and the answer has to run
   anyway. Leaving it to the anchor is what showed Turns 2..6 of an 8-Turn
   session with the new last Turn's answer below the fold.
7. Edit a message and rerun it. There must be one movement, not two — the
   truncation is silent and the rerun's Turn pins as usual.

### Streaming and follow

1. Streaming follows the tail until the user scrolls, and the pinned Turn hands
   off once its answer overflows the viewport.
2. With output streaming, scroll up and hold still. Follow must not write while
   the gesture is recent, and must resume once it goes quiet.
3. Jump to latest is animated rather than an instant jump.
4. While reading a history window, let output arrive from somewhere else. The
   viewport must not move — this is the case the submission event exists to
   stay out of.

### Collapse

1. An auto-collapsing TodoWrite or ExecCommand card leaves earlier content
   visually still.
2. Expand and collapse a tall tool card near the top of the viewport, and one
   below it, and confirm earlier content stays put in both cases.

### The reserved blank and the snap back

1. Scrolling down into the reserved blank and letting go returns to the end of
   real content, and streaming resumes following from there.
2. Pressing End scrolls to the bottom of the scroll range and then comes back —
   that key is the cheapest way to land deep in the spacer.
3. Scroll to the very bottom and confirm the transcript ends where content
   ends, with the reserved blank below it reachable but not where the session
   opens.
4. Wheel down into the reserved blank and stop. The transcript must return to
   the content end in one smooth movement, not shudder in place.
5. With a short Turn pinned, scroll up, jump to latest, then scroll down into
   the blank and let go. After the snap back the jump-to-latest affordance must
   be gone — this is the one path where the viewport arrives at the tail
   without a scroll event to notice it.

### The scrollbar

1. Drag the scrollbar to the very bottom. The screen must not be entirely
   blank: the last Turn and the input clearance stay visible above the
   reservation. Repeat with the composer expanded, which is where the reserve
   falls back to the hold-gap floor.
2. Drag the scrollbar, without touching the wheel first, down into the reserved
   blank and let go: it must snap back. Then drag it while output streams: the
   transcript must follow the thumb without the frame loop fighting it. A press
   on the thumb that moves nothing must leave the viewport alone.

### Resizing

1. With the viewport resting at the end but *not* following — scroll away and
   back, and check the jump-to-latest affordance is hidden — resizing the
   window keeps content against the bottom in every direction: taller reveals
   more history above, shorter does not cut the last lines off, and narrower
   does not push them off screen as the text rewraps. Repeat while reading
   history: nothing should move.

### History paging

1. Open a session long enough to be `isPartial` — the loaded tail is shorter
   than the viewport, so it pages older Turns in on its own. No jump-to-latest
   bar should appear, and streaming output should be followed. Then send a
   message: it must appear immediately and pin to the viewport top, with the
   history above neither moving nor reloading.
2. Scroll up to a junction. **One** page loads, the Turn under the cursor stays
   where it is, and paging stops until the head is reached again. Then keep
   going: every junction must behave the same way all the way to the first
   Turn, with no run of pages and no point where scrolling up stops doing
   anything.
3. Open a long `isPartial` session and scroll up slowly through several paging
   junctions. The Turn under the cursor must not move — not backwards, not
   forwards, and not for a single frame. A stall while a page is measured is a
   known gap and reads differently from a jump: the picture freezes and
   resumes in place, rather than showing different content and snapping back.
   Then scroll up fast through the same junctions, which is where the anchor
   and the user's gesture are most likely to disagree.
4. During that scroll, check that a paging junction does not leave the viewport
   stuck: keep scrolling past it, then wheel back down, and confirm the
   transcript still tracks the gesture in both directions. Nothing on screen may
   fade or slide as rows come back — a row entering the rendered window is not
   its content arriving.

### Turn navigation

1. Turn Rail and Usage Report navigation can top-align the final Turns.
2. Click the last Turn on the Turn Rail while it is short. It must land in one
   movement at the end of the transcript — no top-align followed by a slide
   back down. Do it from near the tail *and* from the top of a long session:
   those are the rendered and unrendered branches, and they take different
   paths. Then click a final Turn whose answer is longer than the viewport: it
   must still top-align.
3. Navigate to a Turn from the Turn Rail — a near one, a far one, and one close
   enough to the end that the window loaded for it reaches the newest Turn. All
   three must come to rest on that Turn at the viewport top and stay there.
4. Navigate to a far Turn and start scrolling with the wheel before it comes to
   rest. The gesture wins immediately and nothing pulls the viewport back to
   the Turn afterwards, including several seconds later.
