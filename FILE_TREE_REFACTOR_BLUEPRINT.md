# BitFun File Tree Refactor Blueprint

## 1. Background

This document proposes a staged refactor plan for BitFun's file tree implementation, using VS Code Explorer as the reference architecture.

Current BitFun file tree characteristics:

- Frontend state is centered in `useFileSystem`
- UI uses two rendering paths:
  - recursive tree for small trees
  - `react-virtuoso` for large trees
- Tree data is plain nested `FileSystemNode[]`
- sync strategy combines watcher events and 1-second polling
- several capabilities exist in API but are not part of the main path
  - paginated children
  - `maxDepth`
  - backend search
  - remote hints

Current VS Code Explorer characteristics:

- explicit model layer: `ExplorerModel` / `ExplorerItem`
- explicit orchestration layer: `ExplorerService`
- explicit view layer: `ExplorerView`
- single tree foundation: `WorkbenchCompressibleAsyncDataTree`
- unified support for sorting, filtering, drag-and-drop, reveal, multi-root workspace, search, provider-based filesystem access

The target is not a line-by-line clone of VS Code. The target is to import the architectural strengths that matter for BitFun:

- stable model layer
- unified rendering path
- provider-oriented data access
- explicit synchronization pipeline
- smaller responsibility per module

### VS Code Reference Implementation Map

The following VS Code files are the primary source references for the architecture discussed in this blueprint.

- `src/vs/workbench/contrib/files/common/explorerModel.ts`
  - explorer data model
  - defines `ExplorerModel` and `ExplorerItem`
  - useful for node structure, resolved state, merging, and parent-child ownership

- `src/vs/workbench/contrib/files/browser/explorerService.ts`
  - explorer orchestration layer
  - useful for file event handling, refresh strategy, reveal logic, editable state, and bulk edit flow

- `src/vs/workbench/contrib/files/browser/views/explorerView.ts`
  - explorer view container
  - useful for tree creation, view state restore, focus behavior, auto reveal, and workbench integration

- `src/vs/workbench/contrib/files/browser/views/explorerViewer.ts`
  - explorer behavior layer around the tree
  - useful for data source, filtering, sorting, drag-and-drop, compression delegate, and find provider

- `src/vs/workbench/contrib/files/common/files.ts`
  - shared explorer contracts
  - useful for view IDs, context keys, sort order enums, and explorer configuration structure

- `src/vs/base/browser/ui/tree/asyncDataTree.ts`
  - async tree infrastructure
  - useful for understanding lazy loading, tree state, and async child resolution

- `src/vs/base/browser/ui/tree/compressedObjectTreeModel.ts`
  - compressed tree infrastructure
  - useful for compact folder behavior and compressed-node reasoning

- `src/vs/base/browser/ui/tree/abstractTree.ts`
  - generic tree behavior foundation
  - useful for navigation, find behavior, and tree update patterns

- `src/vs/workbench/contrib/files/common/explorerFileNestingTrie.ts`
  - file nesting support
  - useful if BitFun later wants richer folder/file grouping beyond current path compression

- `src/vs/workbench/contrib/files/browser/explorerFileContrib.ts`
  - explorer contribution registry
  - useful as a reference if BitFun later introduces explorer-specific pluggable behaviors

- `src/vs/workbench/contrib/files/browser/fileActions.ts`
  - explorer file actions
  - useful for command-level operation wiring

- `src/vs/workbench/contrib/files/browser/fileCommands.ts`
  - explorer/file command registrations
  - useful for separating tree behavior from command surfaces

- `src/vs/workbench/contrib/files/browser/views/explorerDecorationsProvider.ts`
  - explorer decorations
  - useful if BitFun later adds git/status/readonly decorations directly into the tree

Recommended reading order:

1. `src/vs/workbench/contrib/files/common/files.ts`
2. `src/vs/workbench/contrib/files/common/explorerModel.ts`
3. `src/vs/workbench/contrib/files/browser/explorerService.ts`
4. `src/vs/workbench/contrib/files/browser/views/explorerView.ts`
5. `src/vs/workbench/contrib/files/browser/views/explorerViewer.ts`
6. `src/vs/base/browser/ui/tree/asyncDataTree.ts`
7. `src/vs/base/browser/ui/tree/compressedObjectTreeModel.ts`

BitFun-to-VS Code rough mapping:

- BitFun `useFileSystem`
  - closest references:
    - `explorerService.ts`
    - part of `explorerModel.ts`
    - part of `explorerViewer.ts`

- BitFun `FileExplorer.tsx`
  - closest references:
    - `explorerView.ts`
    - part of `explorerViewer.ts`

- BitFun `FileTree.tsx` and `VirtualFileTree.tsx`
  - closest references:
    - `explorerView.ts`
    - `explorerViewer.ts`
    - `asyncDataTree.ts`

- BitFun `FileSystemService.ts`
  - closest references:
    - `explorerService.ts`
    - underlying `IFileService` usage pattern across explorer files

- BitFun path compression utilities
  - closest references:
    - `explorerViewer.ts`
    - `compressedObjectTreeModel.ts`
    - `explorerFileNestingTrie.ts`

## 2. Core Problems To Solve

### 2.1 State and rendering are too coupled

`useFileSystem` currently owns:

- root loading
- lazy loading
- expanded state
- cache invalidation
- watcher reaction
- polling
- partial tree mutation
- option-driven reload

This makes behavior hard to reason about and hard to test.

### 2.2 Two UI paths can drift

`FileTree` and `VirtualFileTree` do not share one behavior engine. This creates risk that:

- rename works in one path but not the other
- drag/drop behavior diverges
- keyboard/a11y parity drifts
- future features need to be implemented twice

### 2.3 Data model is too weak

`FileSystemNode` is a transport-shaped tree DTO, not a view model. It lacks explicit state for:

- resolved vs unresolved directory
- loading status
- error status
- placeholder / phantom nodes
- optimistic operations
- stable identity beyond path

### 2.4 Sync pipeline is fragmented

Watcher, cache invalidation, periodic polling, and editor change notifications are mixed into one hook. This makes correctness fragile for:

- rename
- remote workspace refresh
- expanded descendants
- silent refresh races

### 2.5 API contract is underused

Backend already exposes:

- `get_directory_children_paginated`
- remote-aware tree and directory reads
- structured watcher events

But the frontend main path still treats the backend mostly as a simple directory listing endpoint.

## 3. Refactor Goals

### Functional goals

- preserve current UX during migration
- keep lazy loading
- keep remote workspace support
- keep path compression
- keep file watcher integration
- preserve compatibility with current context menu actions

### Architectural goals

- separate model, orchestration, view, and transport
- reduce hook complexity
- make sync behavior deterministic
- provide one rendering path for all tree sizes
- allow future features without rewriting the foundation

### Quality goals

- unit-testable model logic
- unit-testable reconciliation logic
- fewer race conditions during refresh
- clear ownership per module

## 4. Target Architecture

## 4.1 Layer overview

Recommended stack:

1. Transport layer
2. Provider layer
3. Model layer
4. Controller/service layer
5. View adapter layer
6. React view layer

### 4.1.1 Transport layer

Responsibility:

- invoke Tauri commands
- normalize request and response shapes
- map backend errors

Keep:

- `WorkspaceAPI`

Adjust:

- stop exposing raw `any[]`
- return typed explorer DTOs

### 4.1.2 Provider layer

Introduce a filesystem provider abstraction used by the explorer only.

Suggested interface:

```ts
export interface ExplorerFileSystemProvider {
  getRoots(): Promise<ExplorerRootDescriptor[]>;
  getChildren(request: GetExplorerChildrenRequest): Promise<GetExplorerChildrenResult>;
  watch(callback: (event: ExplorerFsEvent) => void): () => void;
  stat(path: string): Promise<ExplorerNodeStat | null>;
  search(request: ExplorerSearchRequest): Promise<ExplorerSearchResult>;
}
```

Initial implementation:

- `TauriExplorerFileSystemProvider`

Future implementations:

- `RemoteExplorerFileSystemProvider`
- `MockExplorerFileSystemProvider`

### 4.1.3 Model layer

Introduce a stable in-memory explorer model.

Suggested types:

```ts
export type ExplorerNodeId = string;

export interface ExplorerNodeRecord {
  id: ExplorerNodeId;
  path: string;
  name: string;
  parentId: ExplorerNodeId | null;
  kind: 'file' | 'directory';
  depth: number;
  extension?: string;
  size?: number;
  mtime?: number;
  childrenIds: ExplorerNodeId[];
  childrenState: 'unresolved' | 'loading' | 'resolved' | 'error';
  errorMessage?: string;
  isRoot: boolean;
  isExcluded: boolean;
  isCompressed: boolean;
  nestedIntoId?: ExplorerNodeId;
}

export interface ExplorerModelState {
  roots: ExplorerNodeId[];
  nodes: Map<ExplorerNodeId, ExplorerNodeRecord>;
  expanded: Set<ExplorerNodeId>;
  selected: ExplorerNodeId | null;
  focused: ExplorerNodeId | null;
  loadingRoots: boolean;
  filterText: string;
  sortOrder: ExplorerSortOrder;
}
```

Model responsibilities:

- hold canonical tree state
- expose immutable snapshots/selectors
- merge loaded children
- apply filesystem events
- support optimistic rename/create/delete
- calculate visible nodes

This becomes BitFun's equivalent of VS Code `ExplorerModel` plus part of `ExplorerService`.

### 4.1.4 Controller/service layer

Introduce a dedicated controller that coordinates provider, model, cache, sync, and view intents.

Suggested class:

- `ExplorerController`

Responsibilities:

- initialize roots
- load children on expand
- refresh subtree
- reveal path
- handle watcher events
- handle periodic reconciliation
- expose command methods used by UI

Suggested public API:

```ts
interface ExplorerController {
  initialize(rootPath: string): Promise<void>;
  expand(nodeId: string): Promise<void>;
  collapse(nodeId: string): void;
  select(nodeId: string): void;
  revealPath(path: string): Promise<void>;
  refresh(nodeId?: string): Promise<void>;
  applyFsEvent(event: ExplorerFsEvent): Promise<void>;
  setFilter(text: string): void;
  setSort(order: ExplorerSortOrder): void;
  subscribe(listener: () => void): () => void;
  getSnapshot(): ExplorerViewSnapshot;
}
```

### 4.1.5 View adapter layer

Create a pure adapter that converts model snapshot into visible rows.

Suggested module:

- `ExplorerViewProjector`

Responsibilities:

- flatten visible tree
- apply compression
- apply filtering
- compute row metadata
- provide row identity for virtualization

This isolates tree projection from React rendering.

### 4.1.6 React view layer

Use one primary tree component for all sizes.

Recommended approach:

- keep virtualization always available
- use one row renderer
- do not maintain separate behavior logic for small and large trees

Suggested components:

- `ExplorerPane.tsx`
- `ExplorerTree.tsx`
- `ExplorerRow.tsx`
- `ExplorerToolbar.tsx`
- `ExplorerBreadcrumb.tsx`

If a non-virtual path is kept for simplicity, it must still consume the same row projector and row renderer.

## 4.2 Backend alignment

Backend should expose explorer-shaped APIs rather than generic filesystem tree DTOs.

Recommended Tauri commands:

- `explorer_get_roots`
- `explorer_get_children`
- `explorer_get_children_paginated`
- `explorer_stat`
- `explorer_search`
- `explorer_subscribe_metadata` if needed later

Recommended request shape:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerGetChildrenRequest {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub include_hidden: Option<bool>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub remote_connection_id: Option<String>,
}
```

This keeps command semantics aligned with the frontend model.

## 4.3 Provider-oriented remote support

Remote handling should stop leaking into view logic.

Target rule:

- explorer controller does not care whether a path is local or remote
- provider resolves that detail

Current backend remote hint support is useful and should remain, but it should sit behind provider contracts, not feature-specific branching in the React tree layer.

## 5. Proposed Module Layout

Recommended new frontend module layout:

```text
src/web-ui/src/tools/file-explorer/
  components/
    ExplorerPane.tsx
    ExplorerTree.tsx
    ExplorerRow.tsx
    ExplorerToolbar.tsx
    ExplorerBreadcrumb.tsx
  controller/
    ExplorerController.ts
    ExplorerSyncCoordinator.ts
  model/
    ExplorerModel.ts
    ExplorerModelTypes.ts
    ExplorerSelectors.ts
    ExplorerMutations.ts
  provider/
    ExplorerFileSystemProvider.ts
    TauriExplorerFileSystemProvider.ts
  projection/
    ExplorerViewProjector.ts
    ExplorerCompression.ts
    ExplorerFilter.ts
    ExplorerSort.ts
  hooks/
    useExplorerController.ts
    useExplorerSnapshot.ts
  services/
    ExplorerCache.ts
    ExplorerOperationQueue.ts
  types/
    explorer.ts
```

Compatibility bridge during migration:

```text
src/web-ui/src/tools/file-system/
  adapters/
    fileSystemNodeAdapter.ts
```

This allows existing call sites to migrate incrementally instead of forcing a big-bang rename.

## 6. Migration Strategy

Do not replace everything in one pass.

### Phase 0: Freeze current behavior

Goals:

- document current UX behavior
- list known gaps
- create comparison fixtures

Deliverables:

- tree behavior checklist
- sample datasets
- regression cases for:
  - expand/collapse
  - rename
  - drag/drop
  - refresh
  - remote root

### Phase 1: Introduce typed provider and model

Goals:

- keep current UI
- remove raw DTO usage from the view

Work:

- add `ExplorerFileSystemProvider`
- add `ExplorerModel`
- add DTO-to-model adapter
- move cache ownership out of `useFileSystem`

Exit criteria:

- current UI still renders from a model snapshot
- no direct `any[]` tree handling in components

### Phase 2: Move sync logic into controller

Goals:

- shrink `useFileSystem`
- centralize refresh rules

Work:

- add `ExplorerController`
- migrate:
  - watcher reaction
  - polling
  - subtree refresh
  - root reload
  - option-triggered reload

Exit criteria:

- React hook becomes a thin adapter over controller
- tree mutation logic is no longer embedded in hook effects

### Phase 3: Unify render path

Goals:

- eliminate behavioral split between `FileTree` and `VirtualFileTree`

Work:

- create single row renderer
- create single flatten/project path
- route both small and large trees through same visible row model
- remove duplicated expand/select logic

Exit criteria:

- rename, drag, keyboard, selection use one logic path
- virtualization can be enabled without feature regression

### Phase 4: Move feature policies into dedicated modules

Goals:

- match VS Code style separation

Work:

- add `ExplorerFilter`
- add `ExplorerSort`
- add `ExplorerCompression`
- add `ExplorerSearchCoordinator`

Exit criteria:

- component layer no longer owns filtering and sorting recursion
- search is backed by real provider logic

### Phase 5: Upgrade backend API shape

Goals:

- reduce frontend workarounds
- align API with explorer model

Work:

- add explorer-specific commands
- use paginated children on large directories
- make `maxDepth` explicit where useful
- return typed metadata for remote and local nodes

Exit criteria:

- explorer UI consumes explorer-specific transport contracts
- pagination is usable in main path

### Phase 6: Remove legacy module

Goals:

- delete old parallel architecture

Work:

- remove `useFileSystem` as primary engine
- remove duplicate tree components
- keep only compatibility shims that still serve other modules

## 7. Detailed Design Notes

## 7.1 Node identity

Use path as initial identity, but wrap it in explicit `id`.

Reason:

- makes later support for non-path-backed nodes easier
- supports phantom nodes during create/rename
- decouples internal state from transport

Rule:

- `id = normalized path` for stable nodes
- `id = phantom:<uuid>` for transient nodes

## 7.2 Child loading state

Do not infer loading from `children === undefined`.

Explicit state is required:

- `unresolved`
- `loading`
- `resolved`
- `error`

This is necessary for:

- precise spinners
- retry behavior
- optimistic updates
- paginated folders

## 7.3 Sync model

Recommended order of truth:

1. user operation result
2. filesystem watcher event
3. scheduled reconciliation refresh
4. periodic polling fallback

Controller should use an operation queue to avoid conflicting writes to the same subtree.

Suggested rule:

- coalesce refresh requests per directory
- ignore stale async responses using generation tokens
- keep subtree refresh scoped where possible
- escalate to root refresh only on ambiguity

## 7.4 Polling policy

Keep polling only as a fallback.

Recommended future policy:

- disable polling for local workspace when watcher confidence is high
- keep polling for remote workspace
- back off when tree is hidden
- back off when app is unfocused

Suggested config:

```ts
interface ExplorerSyncPolicy {
  enablePollingForLocal: boolean;
  enablePollingForRemote: boolean;
  localPollingMs: number;
  remotePollingMs: number;
  hiddenViewPollingMs: number;
}
```

## 7.5 Search

Current tree-local recursive search is acceptable as a temporary filter, but not as the final architecture.

Target split:

- quick client filter for already-loaded nodes
- provider-backed filename/content search for real workspace search

This should mirror the distinction VS Code keeps between tree filtering and search services.

## 7.6 Compression

Compression should not be a view-only trick.

Target:

- compression is computed in projection layer
- compressed rows preserve access to all underlying node ids
- rename, drag/drop, and selection operate on the concrete target node, not a lossy display string

## 7.7 Drag and drop

Move drag/drop rules out of `FileTreeNode`.

Create:

- `ExplorerDragController`

Responsibilities:

- source payload
- target resolution
- copy vs move rules
- root reorder rules if multi-root is added later
- compressed-row target disambiguation

## 7.8 Rename and optimistic items

Support transient model nodes:

- `pendingCreateFile`
- `pendingCreateFolder`
- `pendingRename`

This is the equivalent of VS Code `NewExplorerItem` behavior and is important to avoid view glitches during inline create/rename.

## 8. Suggested Backend Refactor

Current backend already has the right building blocks. The main need is packaging them behind explorer-focused contracts.

### 8.1 Keep these layers

- Tauri commands
- core filesystem service
- infrastructure file tree service
- watcher emitter

### 8.2 Improve command surface

Create dedicated explorer commands instead of overloading generic filesystem APIs.

### 8.3 Make pagination first-class

Large folders should be handled with page-aware responses:

```rust
pub struct ExplorerChildrenPage {
    pub children: Vec<ExplorerEntryDto>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}
```

### 8.4 Add explorer metadata where useful

Optional future metadata:

- readonly
- symlink
- hidden
- git status summary
- remote origin metadata

Only add fields that are actually consumed by the explorer.

## 9. Testing Plan

## 9.1 Unit tests

Add unit tests for:

- model merge logic
- path normalization
- compression projection
- filter and sort behavior
- watcher event reconciliation
- rename create delete optimistic transitions

## 9.2 Integration tests

Add integration tests for:

- lazy expand loads one directory only
- rename updates the visible row and selection
- watcher-driven refresh updates only affected subtree
- remote workspace path refresh does not force full root reload
- paginated folder merges preserve scroll state

## 9.3 Manual verification checklist

- open large workspace
- expand deeply nested tree
- rename in small tree mode
- rename in virtualized mode
- create new file under compressed path
- delete expanded folder child
- receive external rename from filesystem
- switch local and remote workspaces

## 10. Rollout Plan

Recommended rollout:

1. merge new model and provider behind feature flag
2. migrate one panel/page to new explorer engine
3. shadow-run controller logging against legacy tree
4. switch default for local workspaces
5. switch default for remote workspaces
6. remove legacy path after one stabilization cycle

Suggested feature flag:

- `features.explorerV2`

## 11. Acceptance Criteria

The refactor is complete when all of the following are true:

- explorer behavior is driven by one controller and one model
- renderer behavior is consistent for small and large trees
- watcher and polling logic are outside the React component layer
- search is no longer a stub in the main explorer architecture
- pagination is supported in the main path for large directories
- remote and local access share one provider contract
- rename/create/delete support optimistic transient nodes
- tests cover model reconciliation and subtree refresh logic

## 12. Recommended Implementation Order

Recommended practical order for this repository:

1. add `file-explorer/model`
2. add `file-explorer/provider`
3. add `ExplorerController`
4. adapt current `useFileSystem` to consume controller
5. unify row rendering between normal and virtualized tree
6. move filter/sort/compression into projection layer
7. switch context menu and rename flows to node-id based actions
8. upgrade backend contracts for pagination and explorer-specific DTOs
9. remove legacy duplicated tree path

## 13. Non-Goals

The following are not required for the first refactor milestone:

- full clone of VS Code workbench APIs
- multi-root workspace UI
- full Git decoration integration
- extension-driven explorer contributions
- OS-native drag/drop parity with VS Code

These can be added later once the explorer foundation is stable.

## 14. Short Recommendation

The highest-value change is:

- introduce `ExplorerModel` + `ExplorerController`

The highest-risk area is:

- keeping two render paths alive too long

The most important migration rule is:

- no big-bang rewrite; move responsibilities one layer at a time
