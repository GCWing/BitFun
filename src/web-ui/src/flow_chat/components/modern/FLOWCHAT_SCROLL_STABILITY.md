# FlowChat Natural Scroll Contract

FlowChat currently uses the browser's natural scroll range. It deliberately
does not create synthetic tail space to pin a Turn or protect a card header
during a height reduction.

## Current Behavior

- A newly submitted Turn scrolls to the natural tail and enters follow-output.
- `useFlowChatFollowOutput` is the only continuous writer while output streams.
- Ordinary `scroll` events do not transfer viewport ownership; only explicit
  wheel, touch, or keyboard navigation exits follow-output. This
  keeps layout growth and virtualizer remeasurement from disabling tail
  following while a card is mounting or expanding.
- Turn navigation uses Virtuoso `align: 'start'`, but the browser may clamp a
  target near the transcript tail to the natural maximum `scrollTop`.
- Tool-card expansion and collapse use normal layout reflow. The card header is
  not guaranteed to remain at the same viewport position.
- The Virtuoso Footer contains only real layout space for the current floating
  input stack, history status, and `RuntimeStatusSlot`.
- History prepend restoration may restore one captured item offset, but it may
  not extend the bottom scroll range or install a persistent anchor guard.

These limitations are intentional while a replacement viewport design is
developed. Do not reintroduce bottom reservations, sticky-latest modes,
pre-collapse compensation, or another synthetic range under a different name.

## Viewport Ownership

Keep `followOutput={false}` on Virtuoso. Continuous movement belongs to
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

The Footer height is:

```text
current input-stack height + viewport bottom inset + message clearance
```

It must not retain an earlier input height, include an estimated card shrink,
or grow to make a target start-aligned. Footer height is normal React layout
state; synchronous imperative `height`/`minHeight` compensation is forbidden.

## Tool-Card Contract

Tool cards update their expanded state normally and dispatch `tool-card-toggle`
after a real state change so Virtuoso can remeasure. There is no pre-collapse
intent event. `SmoothHeightCollapse` may continue to animate the local height.

## Verification

Run the smallest relevant automated checks:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run \
  src/flow_chat/components/modern/useFlowChatFollowOutput.test.tsx \
  src/flow_chat/components/modern/VirtualMessageList.session-boundary.test.tsx \
  src/flow_chat/components/modern/ModernFlowChatContainer.history-state.test.tsx \
  src/flow_chat/tool-cards/useToolCardHeightContract.test.tsx
```

Agents must not perform UI interaction verification. A human follow-up should
confirm:

1. A new Turn goes to the natural tail rather than being placed at the top.
2. Streaming follows the tail until the user scrolls.
3. Turn Rail and Usage Report navigation work and clamp naturally near the end.
4. Tool-card collapses reflow naturally without accumulating blank tail space.
5. Session switching and history paging do not restore stale footer height.

Temporary header movement during card collapse and the inability to top-align
the final Turns are expected under this contract.

## Related Files

- `VirtualMessageList.tsx`
- `useFlowChatFollowOutput.ts`
- `ModernFlowChatContainer.tsx`
- `../../utils/flowChatScrollLayout.ts`
- `../../tool-cards/useToolCardHeightContract.ts`
