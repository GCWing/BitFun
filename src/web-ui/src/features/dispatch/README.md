# Dispatch Web UI

The Web UI supports local execution plus detached SSH and same-account device
dispatch.

## Invariants

1. A dispatch target is selected while creating a session and is immutable after
   the first turn.
2. `local` uses the existing session, worktree, persistence, and dialog-turn
   paths unchanged.
3. A non-local session is an observer projection. The controller must not call
   `create_session`, `bind_session_worktree`, `start_dialog_turn`, restore, or
   local session persistence for it.
4. The target CLI owns the ordinary durable session and the append-only event
   log. The controller owns only the outbound observer index and a UI cache.
5. Status cursors advance only after every returned event has been applied.
   Agent envelope ids are deduplicated before replay. Terminal jobs keep
   polling until an empty page confirms that the event log is fully drained.
6. SSH CLI installation is always a separate, explicit confirmation. The UI
   displays the resolved version, URL, and SHA256 before starting it.
7. Account devices use encrypted request/response RPC and distinct
   `dispatch_target_*` commands. They never attach Peer Device Mode and an
   offline target never falls back to local execution.
8. Approval policy is explicit per job: `auto`, `reject-and-report`, or
   `remote`. `remote` projects pending requests into the normal permission
   panel; `auto` requires a one-shot, non-persisted confirmation immediately
   before submit.
9. MiniApp and quick-input hosts do not expose the dispatch picker.
10. Controller-side model settings never leak into an SSH dispatch. The submit
    omits `model` unless preflight recorded an explicit target model choice.
11. Deleting or archiving a projection writes a local job tombstone so outbound
    reconciliation cannot silently reopen it.
12. The observer ignores `SubagentSessionLinked`. Child observer ownership is not
    implemented, so creating an unmarked child projection would violate the
    observer-only persistence and cancellation boundary.
13. Workspace delivery is explicit. `existing` addresses a target directory;
    `snapshot-exact` transfers one verified source snapshot, including ignored
    and hidden regular files but excluding `.git`. It is never live or
    bidirectional synchronization.
14. Cursor pulls are multi-observer safe. Truncation and omitted events are
    visible completeness facts and must not be rendered as a full transcript.
15. The observer continues bounded polling while the window is hidden so
    remote permission and terminal system notifications can be delivered.
16. Controller-wide outbound progress never advances a renderer's own cursor;
    each observer replays and commits only the events it applied.
17. Listing jobs for an explicitly selected target adopts only outbound
    observer routing records. It never restores the target session into the
    controller's backend store or acquires local runtime ownership.
