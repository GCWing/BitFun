## Fix: Workspace section collapsed/expanded states look identical (#976)

### Problem

The workspace section header in the NavPanel uses `collapsible` mode without `onSceneOpen`, so the `ChevronRight` indicator was never rendered. Users could collapse/expand the section but had no visual cue showing the current state — collapsed and expanded headers looked identical.

Additionally, when a user manually collapsed the workspace section and then added a new workspace, the section stayed collapsed, making the new workspace invisible until the user manually expanded it.

### Root Cause

In `SectionHeader.tsx`, the chevron indicator was gated behind `onSceneOpen ?` (line 63), so collapsible-only sections (workspace, assistant sessions) never showed an indicator.

### Changes

1. **`SectionHeader.tsx`** — Render the chevron for both `onSceneOpen` and `collapsible` sections. Added `--collapsed` and `--expanded` CSS modifier classes that rotate the chevron based on `isOpen` state.

2. **`NavPanel.scss`** — Added `transform: rotate(0deg)` for collapsed state and `transform: rotate(90deg)` for expanded state, with the existing `transition: transform` providing smooth animation.

3. **`MainNav.tsx`** — Added a `useEffect` that auto-expands the workspace section when a new workspace is added (tracks `normalWorkspacesList.length`), so newly created workspaces are immediately visible even if the user had previously collapsed the section.

### Validation

- `tsc --noEmit` — no errors in changed files
- `vitest run NavPanelLayout.test.ts` — 4/4 tests pass
- Existing chevron hover behavior (`translateX(1px)` for scene-link sections) is preserved — the rotation classes only apply to `collapsible` sections without `onSceneOpen`

Fixes #976
