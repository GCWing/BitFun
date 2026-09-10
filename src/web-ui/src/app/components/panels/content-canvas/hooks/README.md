# ContentCanvas Hooks

## usePanelTabCoordinator

Installed by collapsible hosts (`AuxPane` and `BottomTerminalPane`), not by
`ContentCanvas`. Each host provides its own explicit expand/collapse actions
and expansion event. File Viewer, Git and Panel View own independent content
and never inherit the session's panel controls.

### Features

1. Reveal only for explicit content-open requests or the host's expansion event.
2. Preserve manual collapse on mount, content updates and workspace restoration.
3. Collapse on a nonempty-to-empty transition within the same scope; count all
   three editor groups. An explicitly opened empty pane stays open.
4. Layout actions set the intended state synchronously and idempotently, without
   delayed toggles that can outlive the host.

### Usage

```tsx
const { expandPanel, collapsePanel } = usePanelTabCoordinator({
  visibleTabCount,
  scopeKey: canvasWorkspaceKey,
  expandEventName: TAB_EVENTS.EXPAND_RIGHT_PANEL,
  onExpand: expandSessionAuxPane,
  onCollapse: collapseSessionAuxPane,
});

// Pass onReveal={expandPanel} and onCollapsePanel={collapsePanel} to ContentCanvas.
```

### Verification

```bash
pnpm --dir src/web-ui run test:run src/app/components/panels/content-canvas/hooks/canvasPanelOwnership.test.tsx
```

This exercises real stores, file-open events, lifecycle hooks and the layout
owner. It verifies state behavior, not visual appearance or remote transport.
