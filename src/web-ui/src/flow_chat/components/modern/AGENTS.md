# FlowChat Scroll Stability Instructions

This file applies to the modern FlowChat viewport implementation under this
directory.

## Required Reading

Before changing FlowChat rendering, projection, virtualization, scrolling,
tool-card collapse behavior, footer reservations, runtime-status slots, or
typewriter/reveal behavior, read:

- `FLOWCHAT_SCROLL_STABILITY.md`

Also follow the repository and Web UI instructions in the parent `AGENTS.md`
files.

The stability document describes the current contracts and the failure modes
that have caused visible flashes, header drops, stale tail whitespace, and
scroll ownership races. Treat it as part of the implementation contract, not
as background documentation.

## Stability Model

The central invariant is single ownership of viewport motion:

- `FlowChatViewportCoordinator` owns semantic viewport modes and anchors:
  `pinned-item`, `following-tail`, and `preserving-element`.
- `useFlowChatFollowOutput` owns the continuous streaming tail-follow RAF loop.
  It is the only continuous writer that advances the viewport toward the
  streaming tail and it must yield while an element anchor or collapse
  transaction owns the viewport.
- `VirtualMessageList` keeps Virtuoso's `followOutput={false}`. Do not enable
  Virtuoso's autonomous follow behavior or introduce another effect that writes
  the outer FlowChat viewport tail position. Independent writers are what
  produce the drop-then-restore flash. Local scroll surfaces inside a thinking,
  explore, terminal, or subagent card have their own narrowly scoped behavior.
- Tool cards must not calculate `scrollTop`, `scrollBy`, compensation pixels,
  or anchor offsets. They dispatch
  `flowchat:tool-card-collapse-intent` before a known height reduction and use
  `useToolCardHeightContract` for the state transition.
- Direct `scrollTop` / `scrollTo` writes are not categorically forbidden, but
  outer-viewport writes are restricted to the coordinator, the follow
  controller, and the narrowly scoped `VirtualMessageList` navigation,
  physical-bottom recovery, and reservation transactions. Every new write
  needs an explicit owner, a user-intent guard, and a reason why the existing
  coordinator or follow controller cannot perform it.
- Footer `collapse` and `pin` reservations provide physical range while the
  DOM or Virtuoso measurements settle. Apply footer compensation synchronously
  before restoring an anchor; do not replace this with React-state-only footer
  rendering.

Stable virtual-item keys and projection identity are equally important. Do not
split one `ModelRound` into multiple `model-round` virtual items, add
mount-triggered animations, or use a timer to reclassify projection/grouping.
Card-local completion preview timers are allowed only when they leave the
virtual projection unchanged and use the existing height contract when they
eventually collapse a card.

## Required Verification

Choose focused tests for the code you changed, then run the normal Web UI
checks:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run <focused-test-files>
```

Relevant stability tests include:

- `src/flow_chat/components/modern/FlowChatViewportCoordinator.test.ts`
- `src/flow_chat/components/modern/useFlowChatFollowOutput.test.tsx`
- `src/flow_chat/components/modern/VirtualMessageList.layout.test.ts`
- `src/flow_chat/components/modern/VirtualMessageList.session-boundary.test.tsx`
- `src/flow_chat/components/modern/flowChatCollapseMotion.test.ts`

For tool-card or collapse-contract changes, also run the nearest focused card
tests, for example the `FileOperationToolCard`, `ExecProcessToolCardView`,
`TaskToolDisplay`, `useToolCardHeightContract`, or `SmoothHeightCollapse` tests.
Do not claim the stability behavior is verified from type-check/lint alone.

## Manual Verification

For changes that affect viewport ownership, tail follow, pinning, collapse
height, session handoff, or scroll event handling, **require the user to perform
the following manual verification before considering the change complete**.
These interactions are too stateful and timing-sensitive for an Agent to verify
reliably; an Agent may run static checks and automated tests, but must not claim
these manual results without explicit user confirmation. Use a real streaming
conversation and check all of the following:

1. Start a new round and confirm the new user message is pinned to the intended
   top position without a visible jump.
2. Do not touch the viewport while output streams. When the output reaches the
   bottom, confirm it transitions naturally into follow-output mode instead of
   stopping short or waiting for the round to finish.
3. While output is streaming and following, scroll upward and downward by hand.
   Confirm the user's scroll intent immediately wins: auto-follow must not pull
   the viewport back or re-pin it unexpectedly.
4. Switch to another conversation during an active round, then return to the
   original conversation. Confirm the viewport resumes the correct follow mode
   when appropriate, without a black/empty tail, excessive synthetic footer
   space, or a delayed recovery that depends on another user scroll.
5. Exercise completed `Write`, `Edit`, `ExecCommand`, and terminal/tool-card
   collapses both at the tail and in the middle of the transcript. Confirm the
   header stays at the same viewport position while the body contracts.
6. Repeat the flow around thinking, explore groups, runtime-status/footer
   visibility, and at least one multi-tool round. Confirm there is no visible
   flash, drop-then-snap-back, permanent fall, accumulating tail whitespace, or
   one-frame loss of the pinned header.

When a manual check fails, enable the FlowChat diagnostics setting and inspect
the session `flowchat.log` before changing ownership rules. Keep diagnostic
payloads bounded and free of message content, tool arguments, and file data.

## Change Discipline

- Do not add a competing scroll writer, persistent scroll lock, or ad hoc
  `scrollBy`/`scrollTo` call from a card or renderer.
- Do not bypass `flowchat:tool-card-collapse-intent` for a known shrink.
- Do not clear footer reservations or semantic anchors merely to make a test
  pass; determine which viewport owner still needs them.
- Preserve user scroll intent, session/generation cancellation, and cleanup of
  pending RAF/timer work.
- Update `FLOWCHAT_SCROLL_STABILITY.md` whenever the ownership model,
  reservation contract, collapse lifecycle, or required verification changes.
