# Workbench content tabs

New files, terminal views and contextual content prefer the right panel of an
already open session tab in the same device/workspace/filesystem scope. That tab
remains eligible while another main scene is active. Only sceneStore's open tabs
are destinations: cached history or a selected session whose tab was closed cannot
open a session implicitly. Without a matching open tab, open a main SceneBar resource.
Existing main views are reused in place, including views explicitly popped out.
File View and Panel View are no longer content scenes. The resource navigation
panel remains available beside conversations and editors.

## Ownership

| Owner | Contract |
| --- | --- |
| sceneStore | Top-level tab identity, activation, pinned order, navigation history and close completion |
| contentResourceStore | Resource identity, captured origin, presentation metadata, dirty/missing status and rename reconciliation |
| EditorDocument | Unsaved text, saved baseline, retained Monaco model and editor view state |
| ResourceFileContext | Origin-bound I/O for nested document renderers and embedded Markdown assets |
| ContentResourceView | Direct resource rendering and the save/discard/cancel close guard |
| FileTabManager / workbenchContentService | Normalize open requests and commit state before a view mounts |
| canvasTabTransfer | Shared menu/drag pop-out transaction, live source validation and document transfer |
| ContentCanvas and its hosts | Contextual Agent/Git/bottom content; each host still owns its own layout |

Default routing captures the device/filesystem origin before session activation,
waits for the owning session's navigation gate, selects its workspace snapshot and
explicitly expands the right panel. Later host mounts cannot replace that write.
Content navigation uses existing-tab activation, which revalidates the session
reference and origin after activation or settings exit. Closing, retiring or
replacing the destination cancels the request instead of recreating its tab.
Git opens and bottom-terminal opens keep their explicitly selected inline host.

No main resource tab contains a second ContentCanvas tab system. Inline pop-out,
from the menu or by dragging a right-panel tab onto SceneBar, transfers its document
reference into a main resource. SceneBar accepts drops on existing tabs or its empty
area and keeps pinned ordering. A tab-shaped destination hint appears when a
transferable session tab starts dragging, before the pointer reaches SceneBar.
Hover strengthens the insertion marker and changes the hint to a release action;
leaving the bar restores the availability hint until the drag ends. Cancellation,
a drop elsewhere, source removal or a device/workspace switch clears the hint.
Drag payloads contain
only live tab/group identity and device epoch; canceled, stale or unrelated drags
do not move content. A conflicting dirty inline view is retained until its draft
is resolved. Terminal runtime ownership remains
with the host; regular tab close detaches the view, while specialized explicit
termination policies keep using the existing terminal service.

## Identity and lifetime

- File identity includes device surface, remote connection and normalized path.
  Native percent sequences are preserved; only explicit file URIs are decoded.
  Windows drive/UNC identity is case-insensitive, remote paths retain POSIX case.
  Workspace-relative paths resolve against the captured workspace.
- Opening an existing file activates its tab and replaces navigation intent.
  Navigation does not overwrite its buffer or create another file tab.
- Editor models have document-owned identities. Renames update the displayed and
  saved path without replacing the document. File and directory rename events carry
  the origin captured before the filesystem request.
- Open file views retain their state during tab and device switches. Every document
  operation checks device identity and activation epoch. It cannot silently read
  or write through the newly selected device. Returning to a failed initial load
  allows the editor to retry on activation.
- Closing a dirty main document offers Save, Discard and Cancel. Failed/canceled saves
  preserve the tab; repeated close gestures share one pending operation. Resource
  removal releases its retained document model.
- Closing a workspace does not delete its opened documents. Their original file
  scope remains explicit. Device tab restoration is window-local, not a persisted
  session/workspace schema or a guarantee of recovering drafts after process exit.
- Main tabs support drag ordering, pinning, middle-click/Delete/Ctrl+W close and
  content-tab batch close. Batch close skips pinned content and stops on a canceled
  close. Paths, unsaved changes and missing files are exposed by the tab chrome.

## Boundaries and verification

This is a Web UI composition change over the existing filesystem and terminal
adapters. It changes no Rust runtime, remote wire format, session persistence or
headless job ownership. Resource/document tests exercise origin isolation, stale
work rejection, rename identity, navigation, close behavior and device restoration.
Those tests are not live Desktop, SSH, Peer Device, Remote Control or Detached
Dispatch integration evidence. Markdown rich undo across an actual inline-to-main
remount and project-wide language-service behavior are separate integration checks.

Use the focused commands in [AGENTS.md](AGENTS.md) and the editor's existing guide.
