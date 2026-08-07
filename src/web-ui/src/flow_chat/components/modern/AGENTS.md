# FlowChat Scroll Instructions

This file applies to the modern FlowChat viewport implementation under this
directory.

## Required Reading

Before changing rendering, virtualization, scrolling, tool-card collapse,
footer layout, runtime-status slots, or reveal behavior, read:

- `FLOWCHAT_SCROLL_STABILITY.md`

Also follow the repository and Web UI instructions in the parent guides.

## Current Contract

- FlowChat uses the natural browser scroll range.
- Do not add synthetic tail space, bottom reservations, sticky Turn modes,
  pre-collapse compensation, or persistent element-anchor guards.
- `useFlowChatFollowOutput` is the only continuous outer viewport writer.
- Keep Virtuoso `followOutput={false}`.
- One-shot Turn/search/history navigation remains inside `VirtualMessageList`.
- Tool cards reflow naturally and dispatch only `tool-card-toggle` after an
  expanded-state change so Virtuoso can remeasure.
- Footer height represents only the current input-stack layout and real footer
  content such as history state and `RuntimeStatusSlot`.
- Stable virtual-item keys and projection identity must be preserved.

## Verification

Choose focused tests, then run:

```text
pnpm run type-check:web
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run test:run <focused-test-files>
```

Relevant tests include:

- `useFlowChatFollowOutput.test.tsx`
- `VirtualMessageList.layout.test.ts`
- `VirtualMessageList.session-boundary.test.tsx`
- `ModernFlowChatContainer.history-state.test.tsx`
- `flowChatCollapseMotion.test.ts`

Do not perform UI interaction verification. Report the manual checks described
in `FLOWCHAT_SCROLL_STABILITY.md` as pending unless the user confirms them.

Update `FLOWCHAT_SCROLL_STABILITY.md` whenever viewport ownership, natural
navigation, footer layout, or required verification changes.
