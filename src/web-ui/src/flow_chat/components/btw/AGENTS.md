# Embedded session panels

The transcript uses `BtwVirtualSessionList` and the shared FlowChat virtualizer.
Keep primary-session selection, paging, and input-footer policy out of this panel.
`useBtwSessionState` selects the child Session, parent title/workspace/connection
metadata, and only for `review-check`, the linked Task's derived outcome. Ordinary
panes must not read the parent's transcript. Keep snapshots stable for unrelated
store updates and parent streaming, and selection synchronous on ID/view changes.

Inactive tabs retain only the `BtwSessionPanel` wrapper and its lightweight view
state. The content, session subscriptions, observers, and scroll callbacks unmount
immediately, including when the auxiliary pane collapses. Keep background task
execution and permission mailboxes outside this lifetime. Preserve exploration
expansion and stable-item reading anchors; following readers return to the latest
output. A successfully restored review location must not reload stale persisted
UI state on each tab switch. Closing a tab releases its wrapper and view state.

For transcript windowing, scrolling, or review integration changes, run:

```bash
pnpm --dir src/web-ui run test:run src/flow_chat/components/btw/btwTailFollow.test.ts src/flow_chat/components/btw/useBtwSessionState.test.tsx src/flow_chat/components/btw/useBtwPanelViewport.test.tsx src/flow_chat/components/btw/BtwVirtualSessionList.test.tsx src/flow_chat/components/btw/BtwSessionPanel.review-action.test.tsx src/flow_chat/components/btw/BtwSessionPanelLayout.test.ts
```

Native WebView2 checks remain manual: long subagent transcripts, streaming tail
follow, upward scrolling, tool expansion, sidebar resizing, and cached-tab return.
Report memory measurements and remote scenarios as unverified until exercised.
