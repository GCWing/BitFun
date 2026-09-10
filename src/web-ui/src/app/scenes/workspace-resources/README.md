# Workspace resources

The workspace's folder entry opens one persistent navigation panel containing files
and terminals. Opening this panel leaves the active conversation or editor intact.
The existing `file-viewer` scene id and `file-viewer-nav` appearance identity remain
compatible with navigation, links and installed skins.

## Ownership

| Owner | Responsibility |
| --- | --- |
| `WorkspaceResourcePanel` | Workspace identity, composition, accessible section controls and action feedback |
| `workspaceResourceState` | Device/workspace-scoped layout preferences; no files, terminal processes or credentials |
| `FilesPanel` / file-explorer | File operations, tree state, search and navigation |
| Existing Shell hooks | Workspace terminal projections, saved configurations and explicit terminal actions |
| `TerminalService` / platform adapter | Terminal transport, event subscriptions and device-epoch boundaries |
| `terminal-core` | PTY lifetime and immutable initial working directory |

The panel reuses existing owners; it does not create an independent terminal manager
or filesystem cache. Search preferences are kept in the file-explorer owner under an
explicit resource key, while results are refreshed from the target.

## Interaction contracts

- File and terminal sections scroll independently and support collapse. The splitter
  supports pointer capture, arrow keys, Shift for larger increments, and Home/End.
  Empty terminals consume only a compact section.
- File and terminal content opens keep workspace navigation visible. Explicit
  navigation back to the main sidebar is respected; settings keep their own navigation.
- File-tree context menus can create a terminal in the selected directory. A terminal
  can reveal its current directory when it is inside the active workspace.
  Reveal preserves unrelated expanded folders, applies the target's path semantics,
  and addresses virtual rows by data index, including compressed directory chains.
- Opening a saved configuration does not run its startup command. Start, stop and
  remove are explicit actions. Removing a saved configuration does not stop its PTY.
- Regular terminal tabs carry `terminalCloseBehavior: detach`. Closing those tabs
  or the standalone terminal view does not stop the process. Specialized and legacy
  terminal tabs retain their existing lifetime policy.
- Exiting a shell leaves its output visible; only closing the view clears its selection.
- A running status means an active terminal session, not that its foreground program
  or development server is healthy.

## Workspace and device scope

Layout and terminal projections include the device surface and remote connection.
Asynchronous terminal work checks the originating device epoch and workspace before
committing UI state or opening a view. A panel unmount only detaches UI listeners.

Terminal ownership is projected from the immutable creation directory. Among nested
opened projects, the closest matching root owns the terminal. Current-directory events
change the displayed location without moving the terminal between projects. Saved
configurations remain attached to the workspace in which they were saved.

Remote paths use POSIX semantics even on a Windows controller. New SSH terminals pass
the owning connection explicitly and do not apply the controller's shell executable.
Host errors retain the last terminal projection and show an unavailable state; an
unsupported CLI peer terminal command remains visibly unsupported.

## Compatibility

`initialCwd` is optional response enrichment backed by the existing terminal-core
`initial_cwd` fact. Old payloads deserialize and round-trip unchanged. Older hosts
without this field remain usable: the frontend anchors their first observed directory
for the terminal-service lifetime. A fresh client cannot reconstruct an earlier
directory an old host never supplied.

Existing local saved-terminal keys and their version-1 records remain readable.
Unknown record fields survive edits. Unreadable configurations and newer record
versions remain intact, with read/write errors surfaced instead of reporting a save.
Other devices and SSH connections use isolated namespaces. Ambiguous historical
path-only profiles are left intact and are not automatically imported onto another
host.

The feature is a shared Web UI surface over existing adapters. Remote-workspace and
peer scoping have contract coverage; these tests do not establish live SSH/peer,
mobile Remote Control, or Detached Dispatch end-to-end behavior.

## Focused verification

```bash
pnpm --dir src/web-ui run test:run src/tools/terminal/services/terminalWorkspaceScope.test.ts src/tools/terminal/services/manualTerminalProfileService.test.ts src/tools/file-system/utils/fileTreeReveal.test.ts src/app/scenes/workspace-resources/workspaceResources.test.ts src/app/scenes/workspace-resources/terminalViewLifecycle.test.tsx
cargo test -p terminal-core --lib workspace_origin_contract_tests
pnpm run check:web
pnpm run i18n:audit
```

No mock-based visual validation or browser control is required for this change.
