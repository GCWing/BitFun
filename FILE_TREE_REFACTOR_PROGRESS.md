# File Tree Refactor Progress

## Status

- Date: 2026-04-07
- Owner: Codex
- Strategy: staged migration with compatibility bridge

## Goals

- introduce a dedicated explorer architecture layer
- keep `FilesPanel` working during migration
- move state orchestration out of the React component tree
- unify future tree behavior behind one controller/model foundation

## Phase Tracking

### Phase 0: Discovery and boundaries

- [x] Identify current integration points
- [x] Write architecture blueprint
- [x] Add VS Code reference implementation map

### Phase 1: Explorer foundation

- [x] Create root progress tracker
- [x] Add new `tools/file-explorer` module
- [x] Add provider abstraction
- [x] Add model abstraction
- [x] Add controller abstraction
- [x] Add snapshot/projection bridge for current UI

### Phase 2: Compatibility bridge

- [x] Reimplement `useFileSystem` on top of the new controller
- [x] Keep `FilesPanel` API unchanged
- [x] Preserve lazy loading, watcher updates, and polling fallback

### Phase 3: UI unification

- [x] Move `FileTree` and `VirtualFileTree` toward one shared row behavior path
- [x] Remove duplicated expand/select logic in `FileExplorer`
- [x] Close rename parity gap in virtualized mode

### Phase 4: Backend and feature consolidation

- [x] Promote paginated children into the main path through controller-side page aggregation
- [x] Introduce explorer-oriented API contracts
- [x] Replace ad hoc filtering/search behavior with dedicated modules
- [x] Replace controller-side full pagination aggregation with UI-driven incremental directory paging

## Notes

- The first implementation pass focuses on architecture ownership, not final UI cleanup.
- Existing `tools/file-system` exports remain as compatibility surface during migration.
- Polling remains as a fallback in the compatibility phase to avoid regressions for remote workspaces.
- Current compatibility limitations remain:
  - UI still has separate recursive and virtualized tree containers
  - breadcrumb behavior still depends on the recursive tree path only
  - search behavior is modularized under `tools/file-explorer/search`, but the explorer controller itself still does not own search state

## Latest Progress

- `loadingPaths` is now wired into the tree projection so lazy-loading directories can render loading state in the UI
- `useFileSystem.updateOptions` now mutates effective explorer options through the controller bridge
- `useFileSystem.setFileTree` now replaces model state through the controller bridge instead of being a no-op
- root lazy load, directory expand, and directory refresh now go through the paginated children API in the controller
- desktop now exposes dedicated explorer commands:
  - `explorer_get_file_tree`
  - `explorer_get_children`
  - `explorer_get_children_paginated`
- legacy tree commands and new explorer commands now share one desktop-side implementation path
- `WorkspaceAPI` now exposes typed explorer DTOs and explorer-specific methods
- `TauriExplorerFileSystemProvider` now calls explorer-specific commands instead of the legacy generic tree commands
- workspace file search state machine now lives under `tools/file-explorer/search/useExplorerSearch.ts`
- legacy `src/web-ui/src/hooks/useFileSearch.ts` now acts as a compatibility bridge to the explorer search module
- tree text filtering and predicate filtering now live in `tools/file-explorer/search/treeFilter.ts` instead of being embedded inside `FileExplorer.tsx`
- `FilesPanel` and `GlobalSearch` now consume `useExplorerSearch` directly; the legacy hook remains only as a compatibility surface
- `useFileSystem.searchFiles` and `fileSystemService.searchFiles` are no longer empty compatibility stubs
- lazy directory paging is now incremental in the UI:
  - controller loads only the visible prefix of a directory instead of aggregating the full directory by default
  - recursive and virtualized tree paths render a `load more` row when a directory still has more children
  - `useFileSystem` now exposes `loadMoreFolder(...)`, and `FilesPanel` wires it through to `FileExplorer`
- verification completed for this step:
  - `npx tsc -p src/web-ui/tsconfig.json --noEmit`
  - `cargo check -p bitfun-desktop`
