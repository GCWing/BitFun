# Tool Card Conventions

This document captures UI behavior conventions for Flow Chat tool cards.

## Preview-to-Result Transition

For tool cards that:

- render a preview while content or params are still arriving
- render a different result view after completion
- can affect list height near the bottom of the conversation

keep the preview visible until the tool actually reaches `status === 'completed'`.

Do not gate the preview only on streaming flags such as `isParamsStreaming`.
There is often a short intermediate window where streaming has ended but the tool
is still not completed. If the preview disappears during that window, the card
can temporarily collapse to header-only height and cause visible vertical drift
in `VirtualMessageList`.

Preferred pattern:

```tsx
if (status !== 'completed' && previewContent) {
  return <PreviewComponent content={previewContent} />;
}

if (status === 'completed' && finalContent) {
  return <ResultComponent content={finalContent} />;
}
```

Current examples:

- `FileOperationToolCard` `Write`
- `FileOperationToolCard` `Edit`

## Auto-Scroll Behavior For Previews

When the preview uses a nested scrolling code viewer, avoid forcing that nested
viewer to auto-scroll while params are streaming. Streaming code previews already
render the latest viewport-sized tail without overscan when nested auto-scroll is
disabled, and the outer conversation list owns the high-level follow behavior.
Writing `scrollTop` on every preview batch can force layout work inside the
WebView and make long code output less responsive.

Preferred pattern:

```tsx
<CodePreview
  content={previewContent}
  isStreaming={isParamsStreaming}
  autoScrollToBottom={false}
/>
```

## Known Height Changes

If a tool card performs a user-triggered or predictable collapse that can reduce
its height near the bottom of the conversation, dispatch
`flowchat:tool-card-collapse-intent` before the collapse happens so
`VirtualMessageList` can pre-compensate.

This applies to both:

- manual expand/collapse actions
- automatic status-driven collapses such as collapsing when a tool completes

After a height-changing expand/collapse actually happens, also dispatch
`tool-card-toggle` so `VirtualMessageList` can schedule follow-up measurement
and reconcile the final layout.

Tool cards should treat the list's bottom-spacing logic as an internal
implementation detail. Do not couple card behavior to specific reservation or
compensation fields inside `VirtualMessageList`; the stable contract for cards is
still the event pair above plus `useToolCardHeightContract`.

Preferred pattern:

```tsx
const cardHeight = cardRootRef.current?.getBoundingClientRect().height ?? null;

if (willCollapse) {
  window.dispatchEvent(new CustomEvent('flowchat:tool-card-collapse-intent', {
    detail: {
      toolId,
      toolName,
      cardHeight,
      reason: 'manual', // or 'auto'
    }
  }));
}

setIsExpanded(nextExpanded);
window.dispatchEvent(new CustomEvent('tool-card-toggle'));
```

Preferred implementation:

Use `useToolCardHeightContract` unless the component truly needs a custom
special-case implementation.

```tsx
const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
  toolId,
  toolName,
});

applyExpandedState(isExpanded, nextExpanded, setIsExpanded, {
  reason: 'manual', // or 'auto'
  detail: {
    filePath,
  },
});
```

If the collapsing region's effective height is better estimated by an inner
scroll container than the outer wrapper, pass `getCardHeight` to the helper.
Current examples include `ModelThinkingDisplay` and `ExploreGroupRenderer`.

Current examples:

- `useToolCardHeightContract`
- `FileOperationToolCard`
- `ModelThinkingDisplay`
- `TerminalToolCard`
- `ExploreGroupRenderer`

For details, read:

- `src/web-ui/src/flow_chat/components/modern/FLOWCHAT_SCROLL_STABILITY.md`
