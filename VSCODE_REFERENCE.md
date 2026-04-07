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