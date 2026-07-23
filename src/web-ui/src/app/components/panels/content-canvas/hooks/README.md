# ContentCanvas Hooks

## usePanelTabCoordinator

Tab and panel state coordinator that manages synchronized state changes.

### Features

1. **Auto-collapse**: collapse the right panel when all tabs are closed
2. **Auto-expand**: expand the right panel when a tab opens
3. **Event listening**: listen to `expand-right-panel` to ensure all open paths expand the panel
4. **State sync**: keep tab and panel state consistent

### Usage

```tsx
usePanelTabCoordinator({
  autoCollapseOnEmpty: true,    // Auto-collapse when all tabs are closed
  autoExpandOnTabOpen: true,   // Auto-expand when a tab opens
});
```

### Design principles

1. **Single responsibility**: coordinate state only, do not replace store methods
2. **Reactive**: listen to tab count changes and respond automatically
3. **Debounced**: use `requestAnimationFrame` to avoid frequent updates
4. **Event-driven**: coordinate with other modules through events
